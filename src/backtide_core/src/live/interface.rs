//! PyO3 interface for live feeds and paper-trading sessions.

use crate::backtest::models::{Order, State};
use crate::data::models::{Bar, Instrument, InstrumentType, Interval, Provider};
use crate::data::providers::{Binance, Coinbase, DataProvider, Kraken};
use crate::indicators::interface::_indicator_deterministic_name;
use crate::indicators::utils::compute_indicators;
use crate::live::engine::PaperBroker;
use crate::live::models::{
    MarketUpdate, PaperTradingConfig, PaperTradingSnapshot, PaperTradingUpdate,
};
use crate::live::providers::{
    support_message, ExchangeMarketDataStream, LiveStreamError, MarketDataStream,
};
use crate::strategies::interface::BuiltinStrategy;
use crate::strategies::utils::IndicatorView;
use crate::utils::python::{dict_to_dataframe, to_python};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Reusable, cancellable exchange market-data collector.
///
/// The feed retains a healthy WebSocket across bounded `collect` calls and
/// retries disconnects with exponential backoff. Call `cancel` safely from
/// another Python thread; cancellation latency is at most 250 ms and closes
/// the retained socket. A later call requires `reset`.
///
/// Parameters
/// ----------
/// provider : str | Provider
///     Exchange WebSocket provider.
///
/// symbols : list[str]
///     Provider symbols to subscribe to.
///
/// interval : str | Interval, default="1m"
///     Candle interval. Coinbase supports `"5m"` only.
///
/// include_partial : bool, default=True
///     Include updates for candles that have not closed yet.
///
/// reconnect_attempts : int, default=5
///     Maximum connection attempts for a collection batch.
///
/// backoff_seconds : float, default=0.25
///     Initial reconnect delay in seconds.
///
/// See Also
/// --------
/// - backtide.live:collect_market_updates
/// - backtide.live:PaperTradingSession
///
/// Examples
/// --------
/// ```pycon
/// from backtide.live import LiveMarketFeed
///
/// feed = LiveMarketFeed("kraken", ["BTC-USD"], interval="1m")
/// feed.cancel()
/// print(feed.is_cancelled())
/// ```
#[pyclass(module = "backtide.live")]
pub struct LiveMarketFeed {
    provider: Provider,
    symbols: Vec<String>,
    interval: Interval,
    include_partial: bool,
    reconnect_attempts: usize,
    backoff: Duration,
    canceled: Arc<AtomicBool>,
    collecting: AtomicBool,
    stream: Mutex<Option<Box<dyn MarketDataStream>>>,
}

#[pymethods]
impl LiveMarketFeed {
    #[new]
    #[pyo3(signature = (
        provider,
        symbols,
        interval: "str | Interval"=Interval::OneMinute,
        include_partial=true,
        reconnect_attempts=5,
        backoff_seconds=0.25,
    ))]
    #[pyo3(
        text_signature = "(provider, symbols, interval='1m', include_partial=True, reconnect_attempts=5, backoff_seconds=0.25)"
    )]
    fn new(
        provider: Provider,
        symbols: Vec<String>,
        interval: Interval,
        include_partial: bool,
        reconnect_attempts: usize,
        backoff_seconds: f64,
    ) -> PyResult<Self> {
        support_message(provider, interval).map_err(PyValueError::new_err)?;
        if symbols.is_empty() || symbols.iter().any(|symbol| symbol.trim().is_empty()) {
            return Err(PyValueError::new_err("at least one non-empty symbol is required"));
        }
        if reconnect_attempts == 0 {
            return Err(PyValueError::new_err("reconnect_attempts must be positive"));
        }
        if !backoff_seconds.is_finite() || backoff_seconds <= 0.0 {
            return Err(PyValueError::new_err("backoff_seconds must be finite and positive"));
        }

        Ok(Self {
            provider,
            symbols,
            interval,
            include_partial,
            reconnect_attempts,
            backoff: Duration::from_secs_f64(backoff_seconds),
            canceled: Arc::new(AtomicBool::new(false)),
            collecting: AtomicBool::new(false),
            stream: Mutex::new(None),
        })
    }

    /// Collect up to `max_events`, retrying transient disconnects.
    ///
    /// Parameters
    /// ----------
    /// max_events : int, default=1
    ///     Maximum number of updates to return.
    ///
    /// timeout_seconds : float, default=30
    ///     Maximum collection time in seconds.
    ///
    /// Returns
    /// -------
    /// list[[MarketUpdate]]
    ///     Updates received before the event limit or timeout.
    ///
    /// Examples
    /// --------
    /// ```pycon
    /// from backtide.live import LiveMarketFeed
    ///
    /// feed = LiveMarketFeed("binance", ["BTC-USDT"])
    /// updates = feed.collect(max_events=10, timeout_seconds=5)  # norun
    /// ```
    #[pyo3(signature = (max_events=1, timeout_seconds=30.0))]
    fn collect(
        &self,
        py: Python<'_>,
        max_events: usize,
        timeout_seconds: f64,
    ) -> PyResult<Vec<MarketUpdate>> {
        validate_collection(max_events, timeout_seconds)?;
        if self
            .collecting
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(PyRuntimeError::new_err(
                "another collect call is already in progress for this feed",
            ));
        }
        let provider = self.provider;
        let symbols = self.symbols.clone();
        let interval = self.interval;
        let include_partial = self.include_partial;
        let reconnect_attempts = self.reconnect_attempts;
        let backoff = self.backoff;
        let canceled = Arc::clone(&self.canceled);
        let result = (|| {
            let initial_stream = self
                .stream
                .lock()
                .map_err(|_| PyRuntimeError::new_err("live feed stream lock is poisoned"))?
                .take();
            let runtime = live_runtime()?;
            let (updates, retained_stream) = py
                .detach(|| {
                    runtime.block_on(collect_updates_reusable(
                        provider,
                        symbols,
                        interval,
                        max_events,
                        Duration::from_secs_f64(timeout_seconds),
                        include_partial,
                        reconnect_attempts,
                        backoff,
                        canceled,
                        initial_stream,
                    ))
                })
                .map_err(stream_error_to_python)?;
            let mut stream_slot = self
                .stream
                .lock()
                .map_err(|_| PyRuntimeError::new_err("live feed stream lock is poisoned"))?;
            if !self.canceled.load(Ordering::Acquire) {
                *stream_slot = retained_stream;
            }
            Ok(updates)
        })();
        self.collecting.store(false, Ordering::Release);
        result
    }

    /// Request cancellation of an in-progress `collect` call.
    ///
    /// Examples
    /// --------
    /// ```pycon
    /// from backtide.live import LiveMarketFeed
    ///
    /// feed = LiveMarketFeed("kraken", ["BTC-USD"])
    /// feed.cancel()
    /// print(feed.is_cancelled())
    /// ```
    fn cancel(&self) -> PyResult<()> {
        self.canceled.store(true, Ordering::Release);
        self.stream
            .lock()
            .map_err(|_| PyRuntimeError::new_err("live feed stream lock is poisoned"))?
            .take();
        Ok(())
    }

    /// Clear cancellation before intentionally reusing this feed.
    ///
    /// Examples
    /// --------
    /// ```pycon
    /// from backtide.live import LiveMarketFeed
    ///
    /// feed = LiveMarketFeed("kraken", ["BTC-USD"])
    /// feed.cancel()
    /// feed.reset()
    /// print(feed.is_cancelled())
    /// ```
    fn reset(&self) -> PyResult<()> {
        if self.collecting.load(Ordering::Acquire) {
            return Err(PyRuntimeError::new_err(
                "cannot reset cancellation while collect is still in progress",
            ));
        }
        self.canceled.store(false, Ordering::Release);
        Ok(())
    }

    /// Whether cancellation has been requested.
    ///
    /// Returns
    /// -------
    /// bool
    ///     `True` after `cancel` and `False` after construction or `reset`.
    ///
    /// Examples
    /// --------
    /// ```pycon
    /// from backtide.live import LiveMarketFeed
    ///
    /// feed = LiveMarketFeed("kraken", ["BTC-USD"])
    /// print(feed.is_cancelled())
    /// ```
    fn is_cancelled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
}

/// A stateful paper-trading account with optional strategy evaluation.
///
/// Parameters
/// ----------
/// config : [PaperTradingConfig] | None, default=None
///     Execution, fee, and risk settings. Uses defaults when omitted.
///
/// strategy : BaseStrategy | None, default=None
///     Existing built-in or custom strategy. Its `evaluate` method runs after
///     resting orders are matched on each processable candle. Explicit orders
///     can also be passed to `on_bar`.
///
/// See Also
/// --------
/// - backtide.live:MarketUpdate
/// - backtide.live:PaperTradingConfig
///
/// Examples
/// --------
/// ```pycon
/// from backtide.live import MarketUpdate, PaperTradingSession
///
/// session = PaperTradingSession()
/// update = session.on_bar(
///     MarketUpdate(
///         "BTC-USD", "1m", 1_700_000_000, 1_700_000_060,
///         100.0, 102.0, 99.0, 101.0, volume=5.0,
///     )
/// )
/// print(update.snapshot.equity)
/// ```
#[pyclass(module = "backtide.live")]
pub struct PaperTradingSession {
    broker: PaperBroker,
    config: PaperTradingConfig,
    strategy: Option<Py<PyAny>>,
    indicator_objects: Vec<(String, Py<PyAny>)>,
    histories: HashMap<String, VecDeque<Bar>>,
}

#[pymethods]
impl PaperTradingSession {
    #[new]
    #[pyo3(signature = (config=None, strategy=None))]
    fn new(
        py: Python<'_>,
        config: Option<PaperTradingConfig>,
        strategy: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        let config = config.unwrap_or_default();
        let broker = PaperBroker::new(config.clone()).map_err(PyValueError::new_err)?;
        let indicator_objects = collect_required_indicators(py, strategy.as_ref())?;

        Ok(Self {
            broker,
            config,
            strategy,
            indicator_objects,
            histories: HashMap::new(),
        })
    }

    /// Process a live or replayed candle.
    ///
    /// Parameters
    /// ----------
    /// market : [MarketUpdate]
    ///     Provider-normalized candle update.
    ///
    /// orders : list[[Order]] | None, default=None
    ///     Explicit orders to submit after resting orders are matched. Orders
    ///     returned by the configured strategy are appended automatically.
    ///
    /// Returns
    /// -------
    /// [PaperTradingUpdate]
    ///     Fills plus a complete mark-to-market account snapshot.
    ///
    /// Examples
    /// --------
    /// ```pycon
    /// from backtide.backtest import Order
    /// from backtide.live import MarketUpdate, PaperTradingSession
    ///
    /// session = PaperTradingSession()
    /// market = MarketUpdate(
    ///     "BTC-USD", "1m", 1_700_000_000, 1_700_000_060,
    ///     100.0, 102.0, 99.0, 101.0,
    /// )
    /// update = session.on_bar(market, [Order("BTC-USD", 1.0)])
    /// print(update.processed)
    /// ```
    #[pyo3(signature = (market, orders=None))]
    fn on_bar(
        &mut self,
        py: Python<'_>,
        market: MarketUpdate,
        orders: Option<Vec<Order>>,
    ) -> PyResult<PaperTradingUpdate> {
        let explicit_orders = orders.unwrap_or_default();
        self.record_history(&market);
        let (mut fills, processed) = self.broker.begin_update(&market);

        let strategy_orders = if processed && self.strategy.is_some() {
            self.evaluate_strategy(py, &market)?
        } else {
            Vec::new()
        };

        let orders_submitted = explicit_orders.len() + strategy_orders.len();
        if processed {
            self.broker.submit_orders(explicit_orders, &market, &mut fills, false);
            self.broker.submit_orders(strategy_orders, &market, &mut fills, true);
        }

        Ok(PaperTradingUpdate {
            market,
            fills,
            snapshot: self.broker.snapshot(),
            orders_submitted,
            processed,
        })
    }

    /// Return the current account state without processing a candle.
    ///
    /// Returns
    /// -------
    /// [PaperTradingSnapshot]
    ///     Current cash, positions, prices, and profit-and-loss values.
    ///
    /// Examples
    /// --------
    /// ```pycon
    /// from backtide.live import PaperTradingSession
    ///
    /// snapshot = PaperTradingSession().snapshot()
    /// print(snapshot.equity)
    /// ```
    fn snapshot(&self) -> PaperTradingSnapshot {
        self.broker.snapshot()
    }

    fn __repr__(&self) -> String {
        let snapshot = self.broker.snapshot();
        format!(
            "PaperTradingSession(equity={}, positions={}, open_orders={}, processed_bars={})",
            snapshot.equity,
            snapshot.portfolio.positions.len(),
            snapshot.portfolio.orders.len(),
            snapshot.processed_bars,
        )
    }
}

impl PaperTradingSession {
    fn record_history(&mut self, market: &MarketUpdate) {
        if !market.is_valid_bar() {
            return;
        }
        let bar = market.bar();
        let history = self.histories.entry(market.symbol.clone()).or_default();
        if let Some(last) = history.back_mut() {
            if last.open_ts == bar.open_ts {
                *last = bar;
                return;
            }
            if bar.open_ts < last.open_ts {
                return;
            }
        }

        history.push_back(bar);
        while history.len() > self.config.max_history {
            history.pop_front();
        }
    }

    fn evaluate_strategy(&mut self, py: Python<'_>, market: &MarketUpdate) -> PyResult<Vec<Order>> {
        let Some(strategy) = self.strategy.as_ref().map(|strategy| strategy.clone_ref(py)) else {
            return Ok(Vec::new());
        };
        let snapshot = self.broker.snapshot();
        let state = State {
            timestamp: market.close_ts as i64,
            bar_index: snapshot.processed_bars.saturating_sub(1),
            total_bars: snapshot.processed_bars,
            is_warmup: false,
        };
        let portfolio = snapshot.portfolio;

        if let Some(builtin) = BuiltinStrategy::try_from_py(py, &strategy) {
            let indicators = self.compute_latest_indicators()?;
            let bars: Vec<(&str, &[Bar])> = self
                .histories
                .iter_mut()
                .map(|(symbol, bars)| (symbol.as_str(), &*bars.make_contiguous()))
                .collect();
            let instrument_types: HashMap<&str, InstrumentType> =
                bars.iter().map(|(symbol, _)| (*symbol, InstrumentType::Crypto)).collect();
            let view = IndicatorView::new(&indicators, 0);
            return Ok(builtin.evaluate(&bars, &portfolio, &state, &view, &instrument_types));
        }

        let data = PyDict::new(py);
        for (symbol, bars) in &mut self.histories {
            data.set_item(symbol, bars_to_data(py, bars.make_contiguous())?)?;
        }

        let indicators = self.compute_indicators_for_python(py)?;
        strategy.bind(py).call_method1("evaluate", (data, portfolio, state, indicators))?.extract()
    }

    fn compute_latest_indicators(
        &self,
    ) -> PyResult<HashMap<String, HashMap<String, Vec<Vec<f64>>>>> {
        let computed = self
            .compute_indicators()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        Ok(computed
            .into_iter()
            .map(|(name, symbols)| {
                let symbols = symbols
                    .into_iter()
                    .map(|(symbol, series)| {
                        let latest = series
                            .into_iter()
                            .map(|values| vec![values.last().copied().unwrap_or(f64::NAN)])
                            .collect();
                        (symbol, latest)
                    })
                    .collect();
                (name, symbols)
            })
            .collect())
    }

    fn compute_indicators_for_python<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let output = PyDict::new(py);
        for (name, symbols) in
            self.compute_indicators().map_err(|error| PyRuntimeError::new_err(error.to_string()))?
        {
            let per_symbol = PyDict::new(py);
            for (symbol, series) in symbols {
                per_symbol.set_item(symbol, to_python(py, &series)?)?;
            }
            output.set_item(name, per_symbol)?;
        }
        Ok(output)
    }

    fn compute_indicators(
        &self,
    ) -> crate::errors::EngineResult<HashMap<String, HashMap<String, Vec<Vec<f64>>>>> {
        if self.indicator_objects.is_empty() {
            return Ok(HashMap::new());
        }
        let aligned: HashMap<String, Vec<Option<Bar>>> = self
            .histories
            .iter()
            .map(|(symbol, bars)| {
                (symbol.clone(), bars.iter().copied().map(Some).collect::<Vec<_>>())
            })
            .collect();
        compute_indicators(&self.indicator_objects, &aligned, None)
    }
}

/// Collect a finite batch from an exchange WebSocket.
///
/// A timeout returns the updates collected so far.
///
/// !!! warning
///     Yahoo Finance is intentionally rejected because it does not provide an
///     official live market-data WebSocket. Choose Binance, Coinbase, or Kraken.
///
/// Parameters
/// ----------
/// provider : str | [Provider]
///     Public WebSocket source. Use `"binance"`, `"coinbase"`, or `"kraken"`.
///     `"yahoo"` is accepted by historical-data APIs but rejected here.
///
/// symbols : list[str]
///     One or more canonical market symbols to subscribe to, such as
///     `"BTC-USDT"` for Binance or `"BTC-USD"` for Coinbase and Kraken.
///
/// interval : str | [Interval], default="1m"
///     Duration represented by each candle. Accepted strings are `"1m"`,
///     `"5m"`, `"15m"`, `"30m"`, `"1h"`, `"4h"`, `"1d"`, and `"1w"` where
///     supported by the provider. Coinbase live collection supports `"5m"` only.
///
/// max_events : int, default=1
///     Maximum number of updates to return across all subscribed symbols.
///
/// timeout_seconds : float, default=30
///     Maximum number of seconds to wait for the batch. When it expires, the
///     function returns any updates already received, including an empty list.
///
/// include_partial : bool, default=True
///     Whether to include in-progress candle revisions. Set to `False` to
///     receive only candles the provider has marked as final.
///
/// Returns
/// -------
/// list[[MarketUpdate]]
///     Updates received before the event limit or timeout.
///
/// See Also
/// --------
/// - backtide.live:LiveMarketFeed
/// - backtide.live:PaperTradingSession
///
/// Examples
/// --------
/// ```pycon
/// from backtide.live import collect_market_updates
///
/// updates = collect_market_updates(  # norun
///     "binance",
///     ["BTC-USDT"],
///     interval="1m",
///     max_events=10,
///     timeout_seconds=5,
/// )
/// ```
#[pyfunction]
#[pyo3(signature = (
    provider,
    symbols,
    interval: "str | Interval"=Interval::OneMinute,
    max_events=1,
    timeout_seconds=30.0,
    include_partial=true,
))]
#[pyo3(
    text_signature = "(provider, symbols, interval='1m', max_events=1, timeout_seconds=30.0, include_partial=True)"
)]
pub fn collect_market_updates(
    py: Python<'_>,
    provider: Provider,
    symbols: Vec<String>,
    interval: Interval,
    max_events: usize,
    timeout_seconds: f64,
    include_partial: bool,
) -> PyResult<Vec<MarketUpdate>> {
    validate_collection(max_events, timeout_seconds)?;

    let runtime = live_runtime()?;
    py.detach(|| {
        runtime
            .block_on(collect_updates(
                provider,
                symbols,
                interval,
                max_events,
                Duration::from_secs_f64(timeout_seconds),
                include_partial,
                3,
                Duration::from_millis(250),
                Arc::new(AtomicBool::new(false)),
            ))
            .map_err(stream_error_to_python)
    })
}

/// List the spot instruments available from a live WebSocket provider.
///
/// Parameters
/// ----------
/// provider : str | [Provider]
///     Live provider whose complete spot catalog should be returned. Yahoo
///     Finance is rejected because it has no supported live WebSocket.
///
/// limit : int, default=10000
///     Maximum number of instruments to return.
///
/// Returns
/// -------
/// list[[Instrument]]
///     Canonical symbols and metadata reported by the selected provider.
///
/// Examples
/// --------
/// ```pycon
/// from backtide.live import list_live_instruments
///
/// instruments = list_live_instruments("kraken", limit=100)
/// print(instruments[0].symbol)
/// ```
#[pyfunction]
#[pyo3(signature = (provider, limit=10_000))]
pub fn list_live_instruments(
    py: Python<'_>,
    provider: Provider,
    limit: usize,
) -> PyResult<Vec<Instrument>> {
    if provider == Provider::Yahoo {
        return Err(PyValueError::new_err(
            "Yahoo Finance does not expose an official market-data WebSocket",
        ));
    }
    if limit == 0 {
        return Err(PyValueError::new_err("limit must be positive"));
    }

    let runtime = live_runtime()?;
    let providers = live_catalog_providers(runtime)?;
    let catalog = providers
        .get(&provider)
        .cloned()
        .ok_or_else(|| PyValueError::new_err(format!("unsupported live provider: {provider}")))?;
    py.detach(|| {
        runtime
            .block_on(catalog.list_instruments(InstrumentType::Crypto, None, limit.min(10_000)))
            .map_err(Into::into)
    })
}

async fn collect_updates(
    provider: Provider,
    symbols: Vec<String>,
    interval: Interval,
    max_events: usize,
    timeout: Duration,
    include_partial: bool,
    reconnect_attempts: usize,
    initial_backoff: Duration,
    canceled: Arc<AtomicBool>,
) -> Result<Vec<MarketUpdate>, LiveStreamError> {
    collect_updates_reusable(
        provider,
        symbols,
        interval,
        max_events,
        timeout,
        include_partial,
        reconnect_attempts,
        initial_backoff,
        canceled,
        None,
    )
    .await
    .map(|(updates, _)| updates)
}

async fn collect_updates_reusable(
    provider: Provider,
    symbols: Vec<String>,
    interval: Interval,
    max_events: usize,
    timeout: Duration,
    include_partial: bool,
    reconnect_attempts: usize,
    initial_backoff: Duration,
    canceled: Arc<AtomicBool>,
    mut retained_stream: Option<Box<dyn MarketDataStream>>,
) -> Result<(Vec<MarketUpdate>, Option<Box<dyn MarketDataStream>>), LiveStreamError> {
    let deadline = Instant::now() + timeout;
    let mut updates = Vec::with_capacity(max_events);
    let mut attempt = 0_usize;
    let mut last_error: Option<LiveStreamError> = None;

    while updates.len() < max_events && !canceled.load(Ordering::Acquire) {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        let mut stream = if let Some(stream) = retained_stream.take() {
            stream
        } else {
            match cancellable_connect(provider, &symbols, interval, deadline, &canceled).await {
                Ok(Some(stream)) => stream,
                Ok(None) => break,
                Err(error) => {
                    last_error = Some(error);
                    attempt += 1;
                    if attempt >= reconnect_attempts {
                        break;
                    }
                    cancellable_backoff(
                        exponential_backoff(initial_backoff, attempt - 1),
                        &canceled,
                        remaining,
                    )
                    .await;
                    continue;
                },
            }
        };

        loop {
            if updates.len() >= max_events || canceled.load(Ordering::Acquire) {
                break;
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            let poll = remaining.min(Duration::from_millis(250));
            match tokio::time::timeout(poll, stream.next_update()).await {
                Err(_) => continue,
                Ok(Ok(Some(update))) => {
                    attempt = 0;
                    if include_partial || update.is_final {
                        updates.push(update);
                    }
                },
                Ok(Ok(None)) => {
                    last_error = Some(LiveStreamError::InvalidMessage(
                        "provider closed the WebSocket connection".to_owned(),
                    ));
                    break;
                },
                Ok(Err(error)) => {
                    last_error = Some(error);
                    break;
                },
            }
        }

        if updates.len() >= max_events {
            retained_stream = Some(stream);
            break;
        }
        if canceled.load(Ordering::Acquire) {
            break;
        }
        if deadline.checked_duration_since(Instant::now()).is_none() {
            retained_stream = Some(stream);
            break;
        }
        attempt += 1;
        if attempt >= reconnect_attempts {
            break;
        }
        if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            cancellable_backoff(
                exponential_backoff(initial_backoff, attempt - 1),
                &canceled,
                remaining,
            )
            .await;
        }
    }

    if updates.is_empty() && !canceled.load(Ordering::Acquire) {
        if let Some(error) = last_error {
            return Err(error);
        }
    }
    Ok((updates, retained_stream))
}

async fn cancellable_connect(
    provider: Provider,
    symbols: &[String],
    interval: Interval,
    deadline: Instant,
    canceled: &AtomicBool,
) -> Result<Option<Box<dyn MarketDataStream>>, LiveStreamError> {
    let connect = ExchangeMarketDataStream::connect(provider, symbols, interval);
    tokio::pin!(connect);

    loop {
        if canceled.load(Ordering::Acquire) {
            return Ok(None);
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Ok(None);
        };
        tokio::select! {
            result = &mut connect => {
                return result.map(|stream| Some(Box::new(stream) as Box<dyn MarketDataStream>));
            },
            _ = tokio::time::sleep(remaining.min(Duration::from_millis(250))) => {},
        }
    }
}

fn validate_collection(max_events: usize, timeout_seconds: f64) -> PyResult<()> {
    if max_events == 0 {
        return Err(PyValueError::new_err("max_events must be positive"));
    }
    if !timeout_seconds.is_finite() || timeout_seconds <= 0.0 {
        return Err(PyValueError::new_err("timeout_seconds must be finite and positive"));
    }
    Ok(())
}

fn live_runtime() -> PyResult<&'static tokio::runtime::Runtime> {
    // Do not use `Engine::get().rt` here: initializing the process engine also
    // opens DuckDB and authenticates a Yahoo REST session. A user consuming a
    // public exchange WebSocket should not pay for storage or unrelated
    // network initialization. This lightweight runtime is still reused for
    // the lifetime of the process.
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let runtime = tokio::runtime::Runtime::new().map_err(|error| {
        PyRuntimeError::new_err(format!("failed to create Tokio runtime: {error}"))
    })?;
    let _ = RUNTIME.set(runtime);
    RUNTIME.get().ok_or_else(|| PyRuntimeError::new_err("failed to initialize live runtime"))
}

fn live_catalog_providers(
    runtime: &'static tokio::runtime::Runtime,
) -> PyResult<&'static HashMap<Provider, Arc<dyn DataProvider>>> {
    static PROVIDERS: OnceLock<HashMap<Provider, Arc<dyn DataProvider>>> = OnceLock::new();
    static INITIALIZING: Mutex<()> = Mutex::new(());
    if let Some(providers) = PROVIDERS.get() {
        return Ok(providers);
    }

    let _guard = INITIALIZING
        .lock()
        .map_err(|_| PyRuntimeError::new_err("live catalog initialization lock is poisoned"))?;
    if let Some(providers) = PROVIDERS.get() {
        return Ok(providers);
    }

    let providers = runtime.block_on(async {
        let mut providers: HashMap<Provider, Arc<dyn DataProvider>> = HashMap::new();
        providers.insert(Provider::Binance, Arc::new(Binance::new().await?));
        providers.insert(Provider::Coinbase, Arc::new(Coinbase::new().await?));
        providers.insert(Provider::Kraken, Arc::new(Kraken::new().await?));
        Ok::<_, crate::data::errors::DataError>(providers)
    })?;
    let _ = PROVIDERS.set(providers);
    PROVIDERS.get().ok_or_else(|| {
        PyRuntimeError::new_err("failed to initialize live instrument catalog providers")
    })
}

fn stream_error_to_python(error: LiveStreamError) -> PyErr {
    match error {
        LiveStreamError::Unsupported(message) => PyValueError::new_err(message),
        other => PyRuntimeError::new_err(other.to_string()),
    }
}

fn exponential_backoff(initial: Duration, exponent: usize) -> Duration {
    initial.saturating_mul(2_u32.saturating_pow(exponent.min(8) as u32)).min(Duration::from_secs(5))
}

async fn cancellable_backoff(duration: Duration, canceled: &AtomicBool, remaining: Duration) {
    let deadline = Instant::now() + duration.min(remaining);
    while !canceled.load(Ordering::Acquire) {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        tokio::time::sleep(left.min(Duration::from_millis(50))).await;
    }
}

fn collect_required_indicators(
    py: Python<'_>,
    strategy: Option<&Py<PyAny>>,
) -> PyResult<Vec<(String, Py<PyAny>)>> {
    let Some(strategy) = strategy else {
        return Ok(Vec::new());
    };
    let strategy = strategy.bind(py);
    if !strategy.hasattr("required_indicators")? {
        return Ok(Vec::new());
    }

    let required: Vec<Py<PyAny>> = strategy.call_method0("required_indicators")?.extract()?;
    let mut seen = HashSet::new();
    let mut indicators = Vec::with_capacity(required.len());
    for indicator in required {
        let name = _indicator_deterministic_name(indicator.bind(py).as_any())?;
        if seen.insert(name.clone()) {
            indicators.push((name, indicator));
        }
    }
    Ok(indicators)
}

fn bars_to_data<'py>(py: Python<'py>, bars: &[Bar]) -> PyResult<Bound<'py, PyAny>> {
    let data = PyDict::new(py);
    data.set_item("open", PyList::new(py, bars.iter().map(|bar| bar.open))?)?;
    data.set_item("high", PyList::new(py, bars.iter().map(|bar| bar.high))?)?;
    data.set_item("low", PyList::new(py, bars.iter().map(|bar| bar.low))?)?;
    data.set_item("close", PyList::new(py, bars.iter().map(|bar| bar.close))?)?;
    data.set_item("volume", PyList::new(py, bars.iter().map(|bar| bar.volume))?)?;
    dict_to_dataframe(py, &data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::models::OrderStatus;
    use crate::data::models::Currency;
    use crate::live::providers::MockMarketDataStream;
    use crate::strategies::interface::BuyAndHold;

    fn mock_update(timestamp: u64) -> MarketUpdate {
        MarketUpdate {
            provider: "mock".to_owned(),
            symbol: "BTC-USDT".to_owned(),
            interval: "1m".to_owned(),
            open_ts: timestamp,
            close_ts: timestamp + 60,
            open: 100.0,
            high: 100.0,
            low: 100.0,
            close: 100.0,
            volume: 1.0,
            n_trades: Some(1),
            is_final: true,
            received_ts: timestamp as i64 + 60,
        }
    }

    #[test]
    fn live_feed_cancellation_can_be_reset_without_connecting() {
        let feed = LiveMarketFeed::new(
            Provider::Binance,
            vec!["BTC-USDT".to_owned()],
            Interval::OneMinute,
            true,
            3,
            0.01,
        )
        .unwrap();

        assert!(!feed.is_cancelled());
        feed.cancel().unwrap();
        assert!(feed.is_cancelled());
        feed.reset().unwrap();
        assert!(!feed.is_cancelled());
    }

    #[test]
    fn live_instrument_catalog_validates_before_network_access() {
        Python::attach(|py| {
            let yahoo = list_live_instruments(py, Provider::Yahoo, 100).unwrap_err();
            assert!(yahoo.to_string().contains("does not expose"));

            let empty = list_live_instruments(py, Provider::Kraken, 0).unwrap_err();
            assert!(empty.to_string().contains("limit must be positive"));
        });
    }

    #[tokio::test]
    async fn reusable_collection_retains_stream_between_bounded_batches() {
        let stream = MockMarketDataStream::new(vec![mock_update(1_000), mock_update(1_060)]);
        let canceled = Arc::new(AtomicBool::new(false));
        let (first, retained) = collect_updates_reusable(
            Provider::Binance,
            vec!["BTC-USDT".to_owned()],
            Interval::OneMinute,
            1,
            Duration::from_secs(1),
            true,
            1,
            Duration::ZERO,
            Arc::clone(&canceled),
            Some(Box::new(stream)),
        )
        .await
        .unwrap();

        let (second, retained) = collect_updates_reusable(
            Provider::Binance,
            vec!["BTC-USDT".to_owned()],
            Interval::OneMinute,
            1,
            Duration::from_secs(1),
            true,
            1,
            Duration::ZERO,
            canceled,
            retained,
        )
        .await
        .unwrap();

        assert_eq!(first[0].open_ts, 1_000);
        assert_eq!(second[0].open_ts, 1_060);
        assert!(retained.is_some());
    }

    #[test]
    fn reconnect_backoff_is_exponential_and_capped() {
        let initial = Duration::from_millis(100);
        assert_eq!(exponential_backoff(initial, 0), Duration::from_millis(100));
        assert_eq!(exponential_backoff(initial, 3), Duration::from_millis(800));
        assert_eq!(exponential_backoff(initial, 20), Duration::from_secs(5));
    }

    #[test]
    fn built_in_strategy_can_drive_paper_session() {
        Python::attach(|py| {
            let strategy = Py::new(py, BuyAndHold::new(Some("BTC-USD".to_owned())))?.into_any();
            let mut session = PaperTradingSession::new(py, None, Some(strategy))?;
            let market = MarketUpdate {
                provider: "mock".to_owned(),
                symbol: "BTC-USD".to_owned(),
                interval: "1m".to_owned(),
                open_ts: 1_000,
                close_ts: 1_060,
                open: 100.0,
                high: 100.0,
                low: 100.0,
                close: 100.0,
                volume: 1.0,
                n_trades: Some(1),
                is_final: true,
                received_ts: 1_060,
            };

            let result = session.on_bar(py, market, None)?;
            assert_eq!(result.orders_submitted, 1);
            assert_eq!(result.fills.len(), 1);
            assert!(result.snapshot.portfolio.positions.contains_key("BTC-USD"));
            PyResult::Ok(())
        })
        .unwrap();
    }

    #[test]
    fn buy_and_hold_reserves_cash_for_paper_fees() {
        Python::attach(|py| {
            let strategy = Py::new(py, BuyAndHold::new(Some("BTC-USD".to_owned())))?.into_any();
            let config = PaperTradingConfig {
                initial_cash: 100.0,
                commission_pct: 0.1,
                commission_fixed: 0.5,
                slippage: 0.1,
                ..PaperTradingConfig::default()
            };
            let mut session = PaperTradingSession::new(py, Some(config), Some(strategy))?;
            let market = MarketUpdate {
                provider: "mock".to_owned(),
                symbol: "BTC-USD".to_owned(),
                interval: "1m".to_owned(),
                open_ts: 1_000,
                close_ts: 1_060,
                open: 100.0,
                high: 100.0,
                low: 100.0,
                close: 100.0,
                volume: 1.0,
                n_trades: Some(1),
                is_final: true,
                received_ts: 1_060,
            };

            let first = session.on_bar(py, market.clone(), None)?;
            assert_eq!(first.orders_submitted, 1);
            assert_eq!(first.fills[0].status, OrderStatus::Filled);
            assert!(first.fills[0].order.quantity < 1.0);
            assert!(first.fills[0].reason.contains("quantity reduced"));
            assert_eq!(first.snapshot.portfolio.cash[&Currency::USD], 0.0);

            let mut next = market;
            next.open_ts = 1_060;
            next.close_ts = 1_120;
            next.received_ts = 1_120;
            let second = session.on_bar(py, next, None)?;
            assert_eq!(second.orders_submitted, 0);
            assert!(second.fills.is_empty());
            PyResult::Ok(())
        })
        .unwrap();
    }

    #[test]
    fn history_replaces_partial_and_ignores_stale_bars() {
        Python::attach(|py| {
            let mut session = PaperTradingSession::new(py, None, None)?;
            let mut current = MarketUpdate {
                provider: "mock".to_owned(),
                symbol: "BTC-USD".to_owned(),
                interval: "1m".to_owned(),
                open_ts: 1_000,
                close_ts: 1_060,
                open: 100.0,
                high: 100.0,
                low: 100.0,
                close: 100.0,
                volume: 1.0,
                n_trades: Some(1),
                is_final: false,
                received_ts: 1_030,
            };
            session.on_bar(py, current.clone(), None)?;
            current.close = 101.0;
            current.high = 101.0;
            current.is_final = true;
            session.on_bar(py, current, None)?;

            let stale = MarketUpdate {
                provider: "mock".to_owned(),
                symbol: "BTC-USD".to_owned(),
                interval: "1m".to_owned(),
                open_ts: 900,
                close_ts: 960,
                open: 90.0,
                high: 90.0,
                low: 90.0,
                close: 90.0,
                volume: 1.0,
                n_trades: Some(1),
                is_final: true,
                received_ts: 1_070,
            };
            let result = session.on_bar(py, stale, None)?;

            assert!(!result.processed);
            assert_eq!(session.histories["BTC-USD"].len(), 1);
            assert_eq!(session.histories["BTC-USD"].back().unwrap().close, 101.0);
            PyResult::Ok(())
        })
        .unwrap();
    }
}
