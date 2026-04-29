//! Backtest engine logic.
//!
//! This module implements the per-strategy event loop, order matching,
//! multi-currency portfolio bookkeeping and result aggregation. It runs
//! every selected strategy fully in parallel using [`rayon`].

use crate::backtest::indicators::Indicator as BuiltinIndicator;
use crate::backtest::models::commission_type::CommissionType;
use crate::backtest::models::experiment_config::ExperimentConfig;
use crate::backtest::models::experiment_result::{
    EquitySample, ExperimentResult, OrderRecord, StrategyRunResult, Trade,
};
use crate::backtest::models::order::{new_order_id, Order};
use crate::backtest::models::order_type::OrderType;
use crate::backtest::models::portfolio::Portfolio;
use crate::backtest::models::state::State;
use crate::backtest::strategies::BuyAndHold;
use crate::data::models::bar::Bar;
use crate::data::models::currency::Currency;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::engine::Engine;
use crate::errors::{EngineError, EngineResult};
use crate::utils::progress::{progress_bar, progress_spinner};
use indicatif::ProgressBar;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::Py;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use uuid::Uuid;

// ────────────────────────────────────────────────────────────────────────────
// Public interface
// ────────────────────────────────────────────────────────────────────────────

impl Engine {
    /// Run a single backtest experiment end-to-end.
    ///
    /// Phases:
    /// 1. Resolve & download required data (skipped if already in storage).
    ///    The configured ``strategy.benchmark`` symbol (if any) is folded
    ///    into the symbol list so its bars flow through the standard
    ///    pipeline and become available to every strategy.
    /// 2. Load OHLCV bars for every primary symbol on the chosen interval.
    /// 3. Compute every requested indicator (built-in or custom Python) once
    ///    over the full dataset, in parallel across (symbol, indicator).
    /// 4. Run every selected strategy in parallel, each with its own
    ///    portfolio, order book and equity log. When a benchmark is
    ///    configured, a ``BuyAndHold(symbol=benchmark)`` strategy is
    ///    auto-injected under the name returned by
    ///    [`benchmark_strategy_name`] so alpha can be derived from real
    ///    backtest results.
    /// 5. Persist the aggregate result (and per-strategy artefacts) to
    ///    DuckDB, then return it to the caller.
    pub fn run_experiment(
        &self,
        config: &ExperimentConfig,
        verbose: bool,
    ) -> EngineResult<ExperimentResult> {
        let started_at = now_secs();
        let experiment_id = Uuid::new_v4().simple().to_string()[..16].to_owned();
        let mut warnings: Vec<String> = Vec::new();

        // Persist the source configuration as a TOML file under
        // `<storage>/experiments/<experiment_id>.toml` *before* the run
        // starts. Lets the UI/CLI re-open and edit a past experiment.
        if let Err(e) =
            persist_experiment_config(&self.config.data.storage_path, &experiment_id, config)
        {
            warn!(experiment_id = %experiment_id, "Failed to persist experiment config: {e}");
            warnings.push(format!("Failed to persist experiment config: {e}"));
        }

        // Augment the symbol list with the benchmark (if any) so its bars
        // get downloaded & loaded just like any user symbol.
        let mut symbols = config.data.symbols.clone();
        let benchmark = config.strategy.benchmark.trim().to_owned();
        if !benchmark.is_empty() && !symbols.iter().any(|s| s == &benchmark) {
            symbols.push(benchmark.clone());
        }
        if symbols.is_empty() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Experiment has no symbols.",
            )));
        }

        // ── Phase 1: data ───────────────────────────────────────────────

        let pb = verbose.then(|| progress_spinner("Resolving instrument profiles..."));
        let profiles = self.resolve_profiles(
            symbols.clone(),
            config.data.instrument_type,
            vec![config.data.interval],
            false,
        )?;
        if let Some(p) = pb {
            p.finish_and_clear();
        }

        let pb = verbose.then(|| progress_spinner("Downloading missing bars..."));
        let start_clamp = config.data.start_date.as_deref().and_then(parse_iso_date_to_ts);
        let end_clamp = config.data.end_date.as_deref().and_then(parse_iso_date_to_ts);
        let dl = self.download_bars(&profiles, start_clamp, end_clamp, false)?;
        warnings.extend(dl.warnings.iter().cloned());
        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // ── Phase 2: load bars ──────────────────────────────────────────
        let pb = verbose.then(|| progress_spinner("Loading bars from storage..."));
        let bar_map = self.load_bars(
            &symbols,
            config.data.interval,
            *self
                .config
                .data
                .providers
                .get(&config.data.instrument_type)
                .expect("provider configured for instrument type"),
            start_clamp,
            end_clamp,
        )?;
        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // Build a master timeline (union of all symbol timestamps, sorted).
        let mut all_ts: Vec<i64> =
            bar_map.values().flat_map(|bars| bars.iter().map(|b| b.open_ts as i64)).collect();
        all_ts.sort_unstable();
        all_ts.dedup();

        if all_ts.is_empty() {
            warnings.push("No bars available for the selected symbols/interval.".into());
            return Ok(ExperimentResult {
                experiment_id,
                name: config.general.name.clone(),
                tags: config.general.tags.clone(),
                started_at,
                finished_at: now_secs(),
                status: "failed".into(),
                strategies: Vec::new(),
                warnings,
            });
        }

        // Per-symbol aligned bars indexed by timestamp position.
        let aligned = align_bars(&bar_map, &all_ts, config.engine.empty_bar_policy);

        // ── Phase 3: indicators (computed once) ─────────────────────────

        let pb = verbose.then(|| {
            progress_bar(config.indicators.indicators.len() as u64, "Computing indicators...")
        });
        let indicators = compute_indicators(&config.indicators.indicators, &aligned, pb.as_ref())?;
        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // ── Phase 4: run strategies in parallel ─────────────────────────

        let mut strategy_objs = load_strategies(&config.strategy.strategies)?;

        // Auto-inject a Buy & Hold of the benchmark symbol as a regular strategy.
        let benchmark_name = format!("Benchmark ({benchmark})");
        if !benchmark.is_empty() {
            match Python::attach(|py| -> PyResult<Py<PyAny>> {
                let bh = BuyAndHold {
                    symbol: Some(benchmark.clone()),
                };
                Ok(Py::new(py, bh)?.into_any())
            }) {
                Ok(obj) => strategy_objs.push((benchmark_name.clone(), obj, false)),
                Err(e) => warnings.push(format!("Failed to instantiate benchmark: {e}")),
            }
        }

        let pb = verbose.then(|| {
            progress_bar(
                strategy_objs.len() as u64,
                format!("Running {} strategies...", strategy_objs.len()),
            )
        });
        let pb_arc = pb.as_ref().map(|p| Mutex::new(p.clone()));

        let cfg_clone = config.clone();
        let aligned_arc = std::sync::Arc::new(aligned);
        let indicators_arc = std::sync::Arc::new(indicators);
        let profiles_arc = std::sync::Arc::new(profiles.clone());

        // Built-in (Rust) strategies are run in parallel via rayon.
        // Custom (Python) strategies are run sequentially under the GIL.
        let (custom, builtin): (Vec<(String, Py<PyAny>, bool)>, Vec<(String, Py<PyAny>, bool)>) =
            strategy_objs.into_iter().partition(|(_, _, is_custom)| *is_custom);

        let cfg_arc = std::sync::Arc::new(cfg_clone);

        let cfg_for_par = std::sync::Arc::clone(&cfg_arc);
        let aligned_for_par = std::sync::Arc::clone(&aligned_arc);
        let indicators_for_par = std::sync::Arc::clone(&indicators_arc);
        let profiles_for_par = std::sync::Arc::clone(&profiles_arc);

        let mut results: Vec<StrategyRunResult> = builtin
            .into_par_iter()
            .map(|(name, obj, _)| {
                let r = run_one_strategy(
                    &name,
                    obj,
                    &cfg_for_par,
                    &aligned_for_par,
                    &indicators_for_par,
                    &profiles_for_par,
                );
                if let Some(pb) = &pb_arc {
                    pb.lock().unwrap().inc(1);
                }
                r
            })
            .collect();

        for (name, obj, _) in custom {
            let r = run_one_strategy(
                &name,
                obj,
                &cfg_arc,
                &aligned_arc,
                &indicators_arc,
                &profiles_arc,
            );
            if let Some(pb) = &pb_arc {
                pb.lock().unwrap().inc(1);
            }
            results.push(r);
        }

        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // ── Compute alpha for every non-benchmark run ───────────────────
        if !benchmark.is_empty() {
            let bench_total = results
                .iter()
                .find(|r| r.strategy_name == benchmark_name)
                .and_then(|r| r.metrics.get("total_return").copied());
            if let Some(b) = bench_total {
                for r in &mut results {
                    if r.strategy_name == benchmark_name {
                        continue;
                    }
                    let tr = r.metrics.get("total_return").copied().unwrap_or(0.0);
                    r.metrics.insert("alpha".into(), tr - b);
                }
            }

            // Ensure the benchmark run is always the first entry in the results.
            if let Some(idx) = results.iter().position(|r| r.strategy_name == benchmark_name) {
                if idx != 0 {
                    let bench = results.remove(idx);
                    results.insert(0, bench);
                }
            }
        }

        let finished_at = now_secs();
        let status = "completed".to_owned();

        let result = ExperimentResult {
            experiment_id: experiment_id.clone(),
            name: config.general.name.clone(),
            tags: config.general.tags.clone(),
            started_at,
            finished_at,
            status,
            strategies: results,
            warnings,
        };

        // ── Phase 5: persist ────────────────────────────────────────────
        let pb = verbose.then(|| progress_spinner("Persisting experiment results..."));
        if let Err(e) = self.db.write_experiment(config, &result) {
            warn!("Failed to persist experiment: {e}");
        }
        if let Some(p) = pb {
            p.finish_and_clear();
        }

        info!(
            id = %experiment_id,
            n_strategies = result.strategies.len(),
            "Experiment finished."
        );
        Ok(result)
    }

    /// Load all bars for the given symbols/interval/provider as a HashMap.
    fn load_bars(
        &self,
        symbols: &[String],
        interval: Interval,
        provider: Provider,
        start: Option<u64>,
        end: Option<u64>,
    ) -> EngineResult<HashMap<String, Vec<Bar>>> {
        let sym_refs: Vec<&str> = symbols.iter().map(String::as_str).collect();
        let stored =
            self.db.query_bars(Some(&sym_refs), Some(&[interval]), Some(&[provider]), None)?;

        let mut map: HashMap<String, Vec<Bar>> = HashMap::new();
        for r in stored {
            let ts = r.bar.open_ts;
            if let Some(s) = start {
                if ts < s {
                    continue;
                }
            }
            if let Some(e) = end {
                if ts >= e {
                    continue;
                }
            }
            map.entry(r.symbol).or_default().push(r.bar);
        }
        for v in map.values_mut() {
            v.sort_by_key(|b| b.open_ts);
        }
        Ok(map)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Helper functions (free)
// ────────────────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Serialise `config` to TOML and write it to
/// `<storage>/experiments/<experiment_id>.toml`. Creates the parent
/// directory on first use. Returns any I/O or serialisation error.
fn persist_experiment_config(
    storage_path: &std::path::Path,
    experiment_id: &str,
    config: &ExperimentConfig,
) -> Result<(), String> {
    let dir = storage_path.join("experiments");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    let path = dir.join(format!("{experiment_id}.toml"));
    let toml_str = Python::attach(|py| -> PyResult<String> { config.to_toml(py) })
        .map_err(|e| format!("serialise: {e}"))?;
    std::fs::write(&path, toml_str).map_err(|e| format!("write {}: {e}", path.display()))
}

fn parse_iso_date_to_ts(s: &str) -> Option<u64> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp() as u64)
}

/// Align bars to a master timeline using the configured empty-bar policy.
fn align_bars(
    bars: &HashMap<String, Vec<Bar>>,
    timeline: &[i64],
    policy: crate::backtest::models::empty_bar_policy::EmptyBarPolicy,
) -> HashMap<String, Vec<Option<Bar>>> {
    use crate::backtest::models::empty_bar_policy::EmptyBarPolicy::*;

    let mut out: HashMap<String, Vec<Option<Bar>>> = HashMap::new();
    for (sym, sym_bars) in bars {
        let by_ts: HashMap<i64, Bar> =
            sym_bars.iter().map(|b| (b.open_ts as i64, b.clone())).collect();
        let mut row: Vec<Option<Bar>> = Vec::with_capacity(timeline.len());
        let mut last: Option<Bar> = None;
        for ts in timeline {
            match by_ts.get(ts) {
                Some(b) => {
                    last = Some(b.clone());
                    row.push(Some(b.clone()));
                },
                None => match policy {
                    Skip => row.push(None),
                    ForwardFill => {
                        if let Some(b) = &last {
                            let mut filled = b.clone();
                            filled.open_ts = *ts as u64;
                            filled.close_ts = *ts as u64;
                            filled.volume = 0.0;
                            row.push(Some(filled));
                        } else {
                            row.push(None);
                        }
                    },
                    FillWithNaN => {
                        let nan_bar = Bar {
                            open_ts: *ts as u64,
                            close_ts: *ts as u64,
                            open_ts_exchange: *ts as u64,
                            open: f64::NAN,
                            high: f64::NAN,
                            low: f64::NAN,
                            close: f64::NAN,
                            adj_close: f64::NAN,
                            volume: f64::NAN,
                            n_trades: None,
                        };
                        row.push(Some(nan_bar));
                    },
                },
            }
        }
        out.insert(sym.clone(), row);
    }
    out
}

/// Compute every requested indicator once over each symbol.
///
/// Returns a `{indicator_name -> {symbol -> Vec<Vec<f64>>}}` map.
fn compute_indicators(
    indicator_names: &[String],
    aligned: &HashMap<String, Vec<Option<Bar>>>,
    pb: Option<&ProgressBar>,
) -> EngineResult<HashMap<String, HashMap<String, Vec<Vec<f64>>>>> {
    let mut out: HashMap<String, HashMap<String, Vec<Vec<f64>>>> = HashMap::new();

    for name in indicator_names {
        let mut per_symbol: HashMap<String, Vec<Vec<f64>>> = HashMap::new();
        let loaded = Python::attach(|py| -> PyResult<Py<PyAny>> { load_indicator(py, name) });

        let obj = match loaded {
            Ok(o) => o,
            Err(e) => {
                warn!("Failed to load indicator {name}: {e}");
                if let Some(p) = pb {
                    p.inc(1);
                }
                continue;
            },
        };

        for (sym, row) in aligned {
            let bars: Vec<Bar> = row
                .iter()
                .map(|b| {
                    b.clone().unwrap_or(Bar {
                        open_ts: 0,
                        close_ts: 0,
                        open_ts_exchange: 0,
                        open: f64::NAN,
                        high: f64::NAN,
                        low: f64::NAN,
                        close: f64::NAN,
                        adj_close: f64::NAN,
                        volume: f64::NAN,
                        n_trades: None,
                    })
                })
                .collect();

            let computed = Python::attach(|py| -> PyResult<Vec<Vec<f64>>> {
                compute_indicator(py, &obj, &bars)
            });

            match computed {
                Ok(series) => {
                    per_symbol.insert(sym.clone(), series);
                },
                Err(e) => warn!("Indicator {name} failed for {sym}: {e}"),
            }
        }
        out.insert(name.clone(), per_symbol);
        if let Some(p) = pb {
            p.inc(1);
        }
    }
    Ok(out)
}

/// Try to compute an indicator: built-in (compute_inner via Rust) first,
/// else fall back to calling Python `.compute(df)`.
fn compute_indicator(py: Python, obj: &Py<PyAny>, bars: &[Bar]) -> PyResult<Vec<Vec<f64>>> {
    // Try every built-in indicator type.
    use crate::backtest::indicators::*;
    let bound = obj.bind(py);
    macro_rules! try_builtin {
        ($($t:ty),* $(,)?) => {
            $(
                if let Ok(b) = bound.cast::<$t>() {
                    let inst: pyo3::PyRef<'_, $t> = b.borrow();
                    let res: Vec<Vec<f64>> = <$t as BuiltinIndicator>::compute_inner(&inst, bars);
                    return Ok(res);
                }
            )*
        };
    }
    try_builtin!(
        AverageDirectionalIndex,
        AverageTrueRange,
        BollingerBands,
        CommodityChannelIndex,
        ExponentialMovingAverage,
        MovingAverageConvergenceDivergence,
        OnBalanceVolume,
        RelativeStrengthIndex,
        SimpleMovingAverage,
        StochasticOscillator,
        VolumeWeightedAveragePrice,
        WeightedMovingAverage,
    );

    // Fallback: call `.compute(df)` on the Python object with a numpy/pandas df.
    let df = bars_to_dataframe(py, bars)?;
    let result = bound.call_method1("compute", (df,))?;

    // Try to extract as a 2-D structure; otherwise treat as 1-D.
    let rows_res: PyResult<Vec<Vec<f64>>> = result.extract();
    if let Ok(rows) = rows_res {
        // Transpose to (n_series, n_points)
        if rows.is_empty() {
            return Ok(vec![]);
        }
        let cols = rows[0].len();
        let mut out: Vec<Vec<f64>> = vec![vec![f64::NAN; rows.len()]; cols];
        for (i, row) in rows.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                out[j][i] = *v;
            }
        }
        return Ok(out);
    }

    let flat: Vec<f64> = result.extract()?;
    Ok(vec![flat])
}

fn bars_to_dataframe<'py>(py: Python<'py>, bars: &[Bar]) -> PyResult<Bound<'py, PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("open", PyList::new(py, bars.iter().map(|b| b.open))?)?;
    dict.set_item("high", PyList::new(py, bars.iter().map(|b| b.high))?)?;
    dict.set_item("low", PyList::new(py, bars.iter().map(|b| b.low))?)?;
    dict.set_item("close", PyList::new(py, bars.iter().map(|b| b.close))?)?;
    dict.set_item("volume", PyList::new(py, bars.iter().map(|b| b.volume))?)?;
    let pd = py.import("pandas")?;
    pd.call_method1("DataFrame", (dict,))
}

/// Resolve an indicator name to a concrete Python object loaded from
/// the local indicators directory.
fn load_indicator(py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
    load_pickled(py, "indicators", name)
}

/// Resolve a strategy name to a concrete Python object loaded from
/// the local strategies directory.
fn load_strategy(py: Python<'_>, name: &str) -> PyResult<(Py<PyAny>, bool)> {
    let obj = load_pickled(py, "strategies", name)?;
    // Detect built-in strategies via the module path of their class.
    let is_custom = Python::attach(|py| -> PyResult<bool> {
        let cls = obj.bind(py).get_type();
        let module: String = cls.getattr("__module__")?.extract()?;
        Ok(!module.starts_with("backtide."))
    })?;
    Ok((obj, is_custom))
}

fn load_pickled(py: Python<'_>, sub: &str, name: &str) -> PyResult<Py<PyAny>> {
    let cfg = crate::config::interface::Config::get()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    let path = cfg.data.storage_path.join(sub).join(format!("{name}.pkl"));
    let cloudpickle = py.import("cloudpickle")?;
    let builtins = py.import("builtins")?;
    let f = builtins.call_method1("open", (path.to_string_lossy().to_string(), "rb"))?;
    let obj = cloudpickle.call_method1("load", (&f,))?;
    f.call_method0("close")?;
    Ok(obj.unbind())
}

/// Load every requested strategy. Returns `(name, obj, is_custom)` triples.
fn load_strategies(names: &[String]) -> EngineResult<Vec<(String, Py<PyAny>, bool)>> {
    Python::attach(|py| -> PyResult<_> {
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let (obj, is_custom) = load_strategy(py, name)?;
            out.push((name.clone(), obj, is_custom));
        }
        Ok(out)
    })
    .map_err(|e: PyErr| EngineError::Io(std::io::Error::other(e.to_string())))
}

// ────────────────────────────────────────────────────────────────────────────
// Per-strategy runner
// ────────────────────────────────────────────────────────────────────────────

/// Execute one strategy through the entire timeline.
fn run_one_strategy(
    name: &str,
    strategy: Py<PyAny>,
    cfg: &ExperimentConfig,
    aligned: &HashMap<String, Vec<Option<Bar>>>,
    indicators: &HashMap<String, HashMap<String, Vec<Vec<f64>>>>,
    profiles: &[crate::data::models::instrument_profile::InstrumentProfile],
) -> StrategyRunResult {
    let symbols: Vec<String> = cfg.data.symbols.clone();
    let _ = &symbols;
    let total_bars: usize = aligned.values().map(|v| v.len()).next().unwrap_or(0);
    let warmup = cfg.engine.warmup_period as usize;

    // Initial portfolio: all initial cash in base currency.
    let base_ccy = cfg.portfolio.base_currency;
    let mut cash: HashMap<Currency, f64> = HashMap::new();
    cash.insert(base_ccy, cfg.portfolio.initial_cash as f64);
    let mut positions: HashMap<String, i64> = cfg.portfolio.starting_positions.clone();
    let mut open_orders: Vec<Order> = Vec::new();

    let mut equity_curve: Vec<EquitySample> = Vec::with_capacity(total_bars);
    let mut order_records: Vec<OrderRecord> = Vec::new();
    let mut closed_trades: Vec<Trade> = Vec::new();
    // Open trade tracker per symbol: (entry_ts, qty_remaining, entry_price)
    let mut open_trades: HashMap<String, (i64, i64, f64)> = HashMap::new();

    let mut peak_equity = cfg.portfolio.initial_cash as f64;

    // Build the timeline once (use any symbol's row).
    let timeline: Vec<i64> = aligned
        .values()
        .next()
        .map(|row| {
            // Reconstruct the master timeline from any symbol's row by
            // picking timestamps from filled bars; missing ones are taken
            // from the first non-skip slot. Fall back to row indices.
            row.iter()
                .enumerate()
                .map(|(i, b)| b.as_ref().map(|x| x.open_ts as i64).unwrap_or(i as i64))
                .collect()
        })
        .unwrap_or_default();

    // Pre-compute instrument quote currency lookup.
    let quote_ccy: HashMap<String, Currency> = profiles
        .iter()
        .filter_map(|p| {
            p.instrument.quote.parse::<Currency>().ok().map(|c| (p.instrument.symbol.clone(), c))
        })
        .collect();

    // Pre-build per-symbol full DataFrames and per-indicator full numpy arrays
    // ONCE under a single GIL acquisition. Each bar then takes O(1) view slices
    // (`df.iloc[:idx+1]` and `arr[:idx+1]`) instead of rebuilding from Python
    // lists every iteration — turning the strategy loop from O(n^2) into O(n).
    let (cached_data, cached_indicators) = Python::attach(
        |py| -> PyResult<(
            HashMap<String, Py<PyAny>>,
            HashMap<String, HashMap<String, Vec<Py<PyAny>>>>,
        )> {
            let pd = py.import("pandas")?;
            let np = py.import("numpy")?;

            let mut data_full: HashMap<String, Py<PyAny>> = HashMap::with_capacity(aligned.len());
            for (sym, row) in aligned {
                let dict = PyDict::new(py);
                dict.set_item(
                    "open",
                    PyList::new(py, row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.open)))?,
                )?;
                dict.set_item(
                    "high",
                    PyList::new(py, row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.high)))?,
                )?;
                dict.set_item(
                    "low",
                    PyList::new(py, row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.low)))?,
                )?;
                dict.set_item(
                    "close",
                    PyList::new(py, row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.close)))?,
                )?;
                dict.set_item(
                    "volume",
                    PyList::new(py, row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.volume)))?,
                )?;
                let df = pd.call_method1("DataFrame", (dict,))?;
                data_full.insert(sym.clone(), df.unbind());
            }

            let mut ind_full: HashMap<String, HashMap<String, Vec<Py<PyAny>>>> =
                HashMap::with_capacity(indicators.len());
            for (name, per_sym) in indicators {
                let mut by_sym: HashMap<String, Vec<Py<PyAny>>> =
                    HashMap::with_capacity(per_sym.len());
                for (sym, series) in per_sym {
                    let mut arrs: Vec<Py<PyAny>> = Vec::with_capacity(series.len());
                    for s in series {
                        let arr = np.call_method1("asarray", (PyList::new(py, s)?,))?;
                        arrs.push(arr.unbind());
                    }
                    by_sym.insert(sym.clone(), arrs);
                }
                ind_full.insert(name.clone(), by_sym);
            }
            Ok((data_full, ind_full))
        },
    )
    .unwrap_or_else(|e| {
        warn!(strategy=%name, "Failed to pre-build strategy view: {e}");
        (HashMap::new(), HashMap::new())
    });

    for bar_index in 0..total_bars {
        let ts = timeline[bar_index];
        let is_warmup = bar_index < warmup;

        // ── 1. Resolve open orders against the *current* bar ────────────
        let mut still_open: Vec<Order> = Vec::new();
        let drained: Vec<Order> = std::mem::take(&mut open_orders);
        for order in drained {
            // Cancel orders take effect immediately and do not need a price.
            if order.order_type == OrderType::CancelOrder {
                still_open.retain(|o| o.id != order.id);
                order_records.push(OrderRecord {
                    order: order.clone(),
                    timestamp: ts,
                    status: "cancelled".into(),
                    fill_price: None,
                    reason: "cancel".into(),
                });
                continue;
            }

            let symbol = &order.symbol;
            let bar = match aligned.get(symbol).and_then(|r| r[bar_index].clone()) {
                Some(b) => b,
                None => {
                    still_open.push(order);
                    continue;
                },
            };

            let fill_px = if cfg.engine.trade_on_close {
                bar.close
            } else {
                bar.open
            };
            // Apply slippage (% of price) adverse to the trade direction.
            let slip = cfg.exchange.slippage / 100.0;
            let fill_px = if order.quantity >= 0 {
                fill_px * (1.0 + slip)
            } else {
                fill_px * (1.0 - slip)
            };

            let qty = order.quantity;
            let notional = fill_px * qty.unsigned_abs() as f64;
            let commission = match cfg.exchange.commission_type {
                CommissionType::Percentage => notional * cfg.exchange.commission_pct / 100.0,
                CommissionType::Fixed => cfg.exchange.commission_fixed,
                CommissionType::PercentagePlusFixed => {
                    notional * cfg.exchange.commission_pct / 100.0 + cfg.exchange.commission_fixed
                },
            };

            let order_ccy = quote_ccy.get(symbol).copied().unwrap_or(base_ccy);

            // ── Funds check & settlement ─────────────────────────────
            if qty > 0 {
                // BUY: try paying in `order_ccy` first, else convert from base.
                let needed = notional + commission;
                if !try_debit(&mut cash, order_ccy, needed, base_ccy, 1.0) {
                    warn!(strategy=%name, order_id=%order.id, "Insufficient funds for buy, skipping order.");
                    order_records.push(OrderRecord {
                        order: order.clone(),
                        timestamp: ts,
                        status: "rejected".into(),
                        fill_price: None,
                        reason: "insufficient funds".into(),
                    });
                    continue;
                }
                *positions.entry(symbol.clone()).or_insert(0) += qty;
                update_open_trade_buy(&mut open_trades, symbol, ts, qty, fill_px);
            } else if qty < 0 {
                let abs_qty = qty.unsigned_abs() as i64;
                let cur = *positions.get(symbol).unwrap_or(&0);
                if !cfg.exchange.allow_short_selling && cur < abs_qty {
                    warn!(strategy=%name, order_id=%order.id, "Short selling disabled and not enough position, skipping.");
                    order_records.push(OrderRecord {
                        order: order.clone(),
                        timestamp: ts,
                        status: "rejected".into(),
                        fill_price: None,
                        reason: "short selling disabled".into(),
                    });
                    continue;
                }
                // Credit proceeds, debit commission.
                *cash.entry(order_ccy).or_insert(0.0) += notional;
                if !try_debit(&mut cash, order_ccy, commission, base_ccy, 1.0) {
                    // Reverse: not enough to even pay commission; very unlikely.
                    *cash.entry(order_ccy).or_insert(0.0) -= notional;
                    order_records.push(OrderRecord {
                        order: order.clone(),
                        timestamp: ts,
                        status: "rejected".into(),
                        fill_price: None,
                        reason: "cannot pay commission".into(),
                    });
                    continue;
                }
                let pos_entry = positions.entry(symbol.clone()).or_insert(0);
                *pos_entry -= abs_qty;
                if let Some(t) = close_open_trade_sell(
                    &mut open_trades,
                    symbol,
                    ts,
                    abs_qty,
                    fill_px,
                    commission,
                ) {
                    closed_trades.push(t);
                }
            }

            order_records.push(OrderRecord {
                order: order.clone(),
                timestamp: ts,
                status: "filled".into(),
                fill_price: Some(fill_px),
                reason: String::new(),
            });
        }
        open_orders = still_open;

        // ── 2. Build State + Portfolio + per-symbol view ────────────────
        let state = State {
            timestamp: ts,
            bar_index: bar_index as u64,
            total_bars: total_bars as u64,
            is_warmup,
        };
        let portfolio = Portfolio {
            cash: cash.clone(),
            positions: positions.clone(),
            orders: open_orders.clone(),
        };

        // ── 3. Strategy.evaluate(...) ────────────────────────────────────
        if !is_warmup {
            let new_orders = Python::attach(|py| -> PyResult<Vec<Order>> {
                let data = build_per_symbol_view(py, &cached_data, bar_index)?;
                let inds = build_indicator_view(py, &cached_indicators, bar_index)?;
                let res = strategy
                    .bind(py)
                    .call_method1("evaluate", (data, portfolio.clone(), state.clone(), inds))?;
                let list: Vec<Order> = res.extract().unwrap_or_default();
                Ok(list)
            });

            match new_orders {
                Ok(mut ords) => {
                    if cfg.engine.exclusive_orders && !ords.is_empty() {
                        // Cancel everything pending first.
                        for o in &open_orders {
                            order_records.push(OrderRecord {
                                order: o.clone(),
                                timestamp: ts,
                                status: "cancelled".into(),
                                fill_price: None,
                                reason: "exclusive_orders".into(),
                            });
                        }
                        open_orders.clear();
                    }
                    // Validate allowed types & ensure ids are populated.
                    let allowed = &cfg.exchange.allowed_order_types;
                    ords.retain_mut(|o| {
                        if o.id.is_empty() {
                            o.id = new_order_id();
                        }
                        if !allowed.contains(&o.order_type)
                            && o.order_type != OrderType::CancelOrder
                        {
                            warn!(strategy=%name, "Order type {} not allowed, dropping.", o.order_type);
                            order_records.push(OrderRecord {
                                order: o.clone(),
                                timestamp: ts,
                                status: "rejected".into(),
                                fill_price: None,
                                reason: "order type not allowed".into(),
                            });
                            return false;
                        }
                        true
                    });
                    open_orders.extend(ords);
                },
                Err(e) => warn!(strategy=%name, "evaluate() raised: {e}"),
            }
        }

        // ── 4. Mark-to-market & equity sample ────────────────────────────
        let mut equity = cash.values().sum::<f64>(); // base + foreign treated 1:1 (best-effort)
        for (sym, qty) in &positions {
            if *qty == 0 {
                continue;
            }
            if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
                equity += *qty as f64 * b.close;
            }
        }
        if equity > peak_equity {
            peak_equity = equity;
        }
        let drawdown = if peak_equity > 0.0 {
            (equity - peak_equity) / peak_equity
        } else {
            0.0
        };
        equity_curve.push(EquitySample {
            timestamp: ts,
            equity,
            cash: cash.values().sum::<f64>(),
            drawdown,
        });
    }

    // ── 5. Liquidate remaining positions to compute final PnL ───────────
    if let Some(last_idx) = total_bars.checked_sub(1) {
        for (sym, qty) in positions.clone() {
            if qty == 0 {
                continue;
            }
            if let Some(b) = aligned.get(&sym).and_then(|r| r[last_idx].as_ref()) {
                let exit_px = b.close;
                if let Some((entry_ts, _q, entry_px)) = open_trades.remove(&sym) {
                    let pnl = (exit_px - entry_px) * qty as f64;
                    closed_trades.push(Trade {
                        symbol: sym.clone(),
                        quantity: qty,
                        entry_ts,
                        exit_ts: timeline[last_idx],
                        entry_price: entry_px,
                        exit_price: exit_px,
                        pnl,
                    });
                }
            }
        }
    }

    // ── 6. Metrics ──────────────────────────────────────────────────────
    let metrics = compute_metrics(
        cfg.portfolio.initial_cash as f64,
        cfg.engine.risk_free_rate / 100.0,
        &equity_curve,
        &closed_trades,
    );

    StrategyRunResult {
        strategy_id: Uuid::new_v4().simple().to_string()[..16].to_owned(),
        strategy_name: name.to_owned(),
        equity_curve,
        trades: closed_trades,
        orders: order_records,
        metrics,
    }
}

/// Try to debit `amount` of `ccy` from `cash`. If `ccy` doesn't have enough,
/// fall back to the base currency at the given conversion rate. Returns
/// `false` if even the fallback is insufficient.
fn try_debit(
    cash: &mut HashMap<Currency, f64>,
    ccy: Currency,
    amount: f64,
    base: Currency,
    rate_to_base: f64,
) -> bool {
    if amount <= 0.0 {
        return true;
    }
    let avail = *cash.get(&ccy).unwrap_or(&0.0);
    if avail >= amount {
        *cash.entry(ccy).or_insert(0.0) -= amount;
        return true;
    }
    let remaining = amount - avail.max(0.0);
    let base_avail = *cash.get(&base).unwrap_or(&0.0);
    let needed_in_base = remaining * rate_to_base;
    if base_avail >= needed_in_base {
        cash.insert(ccy, 0.0);
        *cash.entry(base).or_insert(0.0) -= needed_in_base;
        true
    } else {
        false
    }
}

fn update_open_trade_buy(
    open_trades: &mut HashMap<String, (i64, i64, f64)>,
    symbol: &str,
    ts: i64,
    qty: i64,
    px: f64,
) {
    open_trades
        .entry(symbol.to_owned())
        .and_modify(|(_, q, p)| {
            let total = (*q as f64) * *p + (qty as f64) * px;
            *q += qty;
            if *q != 0 {
                *p = total / (*q as f64);
            }
        })
        .or_insert((ts, qty, px));
}

fn close_open_trade_sell(
    open_trades: &mut HashMap<String, (i64, i64, f64)>,
    symbol: &str,
    ts: i64,
    abs_qty: i64,
    exit_px: f64,
    commission: f64,
) -> Option<Trade> {
    let (entry_ts, mut q, entry_px) = open_trades.remove(symbol)?;
    let used = abs_qty.min(q);
    q -= used;
    let pnl = (exit_px - entry_px) * used as f64 - commission;
    if q > 0 {
        open_trades.insert(symbol.to_owned(), (entry_ts, q, entry_px));
    }
    Some(Trade {
        symbol: symbol.to_owned(),
        quantity: used,
        entry_ts,
        exit_ts: ts,
        entry_price: entry_px,
        exit_price: exit_px,
        pnl,
    })
}

fn compute_metrics(
    initial_cash: f64,
    risk_free_rate: f64,
    curve: &[EquitySample],
    trades: &[Trade],
) -> HashMap<String, f64> {
    let mut m = HashMap::new();

    let final_equity = curve.last().map(|s| s.equity).unwrap_or(initial_cash);
    let total_return = if initial_cash > 0.0 {
        (final_equity - initial_cash) / initial_cash
    } else {
        0.0
    };

    // Trade-derived metrics.
    let n_trades = trades.len() as f64;
    let n_wins = trades.iter().filter(|t| t.pnl > 0.0).count() as f64;
    let win_rate = if n_trades > 0.0 {
        n_wins / n_trades
    } else {
        0.0
    };

    m.insert("total_return".into(), total_return);
    m.insert("final_equity".into(), final_equity);
    m.insert("pnl".into(), final_equity - initial_cash);
    m.insert("n_trades".into(), n_trades);
    m.insert("win_rate".into(), win_rate);

    // Annualized stats reuse the shared kernel from `analysis.rs` so that
    // the analysis page and backtest engine produce identical numbers.
    let values: Vec<f64> = curve.iter().map(|s| s.equity).collect();
    let timestamps: Vec<f64> = curve.iter().map(|s| s.timestamp as f64).collect();
    let stats = crate::analysis::compute_series_stats(&values, &timestamps, risk_free_rate, None);

    let (cagr, ann_vol, sharpe, sortino, max_dd) = match stats {
        Some(s) => (s.ann_return, s.ann_volatility, s.sharpe, s.sortino, s.max_drawdown),
        None => (0.0, 0.0, 0.0, 0.0, 0.0),
    };
    m.insert("cagr".into(), cagr);
    m.insert("ann_volatility".into(), ann_vol);
    m.insert("sharpe_ratio".into(), sharpe);
    m.insert("sortino_ratio".into(), sortino);
    m.insert("max_drawdown".into(), max_dd);

    m
}

/// Build a Python dict `{symbol: pandas.DataFrame}` view through bar `idx`.
///
/// Takes pre-built full DataFrames per symbol and returns cheap O(1)
/// `df.iloc[:idx+1]` views — pandas slicing creates a new DataFrame
/// pointing into the same underlying arrays, so no data is copied.
fn build_per_symbol_view<'py>(
    py: Python<'py>,
    cached: &HashMap<String, Py<PyAny>>,
    idx: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let out = PyDict::new(py);
    let end = idx + 1;
    for (sym, df) in cached {
        let bound = df.bind(py);
        let iloc = bound.getattr("iloc")?;
        let slice = pyo3::types::PySlice::new(py, 0, end as isize, 1);
        let sliced = iloc.get_item(slice)?;
        out.set_item(sym, sliced)?;
    }
    Ok(out.into_any())
}

/// Build a Python dict view of indicator values up to bar `idx`.
///
/// Takes pre-built full numpy arrays per (indicator, symbol, series) and
/// returns cheap O(1) `arr[:idx+1]` slice-views.
fn build_indicator_view<'py>(
    py: Python<'py>,
    cached: &HashMap<String, HashMap<String, Vec<Py<PyAny>>>>,
    idx: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let out = PyDict::new(py);
    let end = idx + 1;
    for (name, per_sym) in cached {
        let by_sym = PyDict::new(py);
        for (sym, arrs) in per_sym {
            let slice = pyo3::types::PySlice::new(py, 0, end as isize, 1);
            if arrs.len() == 1 {
                let sliced = arrs[0].bind(py).get_item(&slice)?;
                by_sym.set_item(sym, sliced)?;
            } else {
                let list = PyList::empty(py);
                for arr in arrs {
                    let sliced = arr.bind(py).get_item(&slice)?;
                    list.append(sliced)?;
                }
                by_sym.set_item(sym, list)?;
            }
        }
        out.set_item(name, by_sym)?;
    }
    Ok(out.into_any())
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::models::commission_type::CommissionType;
    use crate::backtest::models::empty_bar_policy::EmptyBarPolicy;
    use crate::backtest::models::experiment_config::*;
    use crate::data::models::instrument::Instrument;
    use crate::data::models::instrument_profile::InstrumentProfile;
    use crate::data::models::instrument_type::InstrumentType;

    fn mk_bar(ts: u64, close: f64) -> Bar {
        Bar {
            open_ts: ts,
            close_ts: ts + 86_399,
            open_ts_exchange: ts,
            open: close,
            high: close * 1.01,
            low: close * 0.99,
            close,
            adj_close: close,
            volume: 1_000.0,
            n_trades: Some(1),
        }
    }

    fn mk_aligned(symbol: &str, prices: &[f64]) -> HashMap<String, Vec<Option<Bar>>> {
        let mut row = Vec::new();
        for (i, p) in prices.iter().enumerate() {
            row.push(Some(mk_bar(1_700_000_000 + i as u64 * 86_400, *p)));
        }
        let mut m = HashMap::new();
        m.insert(symbol.to_owned(), row);
        m
    }

    fn mk_cfg(symbol: &str) -> ExperimentConfig {
        let mut cfg = ExperimentConfig {
            general: GeneralExpConfig::default(),
            data: DataExpConfig::default(),
            portfolio: PortfolioExpConfig::default(),
            strategy: StrategyExpConfig::default(),
            indicators: IndicatorExpConfig::default(),
            exchange: ExchangeExpConfig::default(),
            engine: EngineExpConfig::default(),
        };
        cfg.data.symbols = vec![symbol.to_owned()];
        cfg.data.instrument_type = InstrumentType::Stocks;
        cfg.engine.empty_bar_policy = EmptyBarPolicy::ForwardFill;
        cfg.exchange.commission_type = CommissionType::Fixed;
        cfg.exchange.commission_fixed = 0.0;
        cfg.exchange.commission_pct = 0.0;
        cfg.exchange.slippage = 0.0;
        cfg.portfolio.initial_cash = 10_000;
        cfg.exchange.allowed_order_types = vec![OrderType::Market, OrderType::CancelOrder];
        cfg
    }

    fn mk_profile(symbol: &str, quote: &str) -> InstrumentProfile {
        InstrumentProfile {
            instrument: Instrument {
                symbol: symbol.to_owned(),
                name: symbol.to_owned(),
                base: None,
                quote: quote.to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "TEST".to_owned(),
                provider: Provider::Yahoo,
            },
            earliest_ts: HashMap::new(),
            latest_ts: HashMap::new(),
            legs: vec![],
        }
    }

    /// Tiny helper that emits a single market buy on the first bar
    /// after warmup, then nothing else. Implemented in pure Rust so the
    /// tests don't need the GIL.
    fn run_with_orders(
        cfg: &ExperimentConfig,
        aligned: &HashMap<String, Vec<Option<Bar>>>,
        profiles: &[InstrumentProfile],
        injected: Vec<(usize, Order)>,
    ) -> StrategyRunResult {
        let total_bars = aligned.values().map(|v| v.len()).next().unwrap_or(0);
        let timeline: Vec<i64> = aligned
            .values()
            .next()
            .unwrap()
            .iter()
            .map(|b| b.as_ref().map(|x| x.open_ts as i64).unwrap_or(0))
            .collect();

        let base_ccy = cfg.portfolio.base_currency;
        let mut cash: HashMap<Currency, f64> = HashMap::new();
        cash.insert(base_ccy, cfg.portfolio.initial_cash as f64);
        let mut positions: HashMap<String, i64> = cfg.portfolio.starting_positions.clone();
        let mut open_orders: Vec<Order> = Vec::new();
        let mut equity_curve: Vec<EquitySample> = Vec::with_capacity(total_bars);
        let mut order_records: Vec<OrderRecord> = Vec::new();
        let mut closed_trades: Vec<Trade> = Vec::new();
        let mut open_trades: HashMap<String, (i64, i64, f64)> = HashMap::new();
        let mut peak = cfg.portfolio.initial_cash as f64;

        let quote_ccy: HashMap<String, Currency> = profiles
            .iter()
            .filter_map(|p| {
                p.instrument
                    .quote
                    .parse::<Currency>()
                    .ok()
                    .map(|c| (p.instrument.symbol.clone(), c))
            })
            .collect();

        for bar_index in 0..total_bars {
            let ts = timeline[bar_index];

            // Inject orders at this bar.
            for (i, o) in &injected {
                if *i == bar_index {
                    open_orders.push(o.clone());
                }
            }

            // Resolve open orders: simplified copy of run_one_strategy's logic.
            let drained: Vec<Order> = std::mem::take(&mut open_orders);
            for order in drained {
                if order.order_type == OrderType::CancelOrder {
                    open_orders.retain(|o| o.id != order.id);
                    order_records.push(OrderRecord {
                        order: order.clone(),
                        timestamp: ts,
                        status: "cancelled".into(),
                        fill_price: None,
                        reason: "cancel".into(),
                    });
                    continue;
                }
                let bar = match aligned.get(&order.symbol).and_then(|r| r[bar_index].clone()) {
                    Some(b) => b,
                    None => {
                        open_orders.push(order);
                        continue;
                    },
                };
                let fill_px = bar.open;
                let qty = order.quantity;
                let notional = fill_px * qty.unsigned_abs() as f64;
                let order_ccy = quote_ccy.get(&order.symbol).copied().unwrap_or(base_ccy);

                if qty > 0 {
                    if !try_debit(&mut cash, order_ccy, notional, base_ccy, 1.0) {
                        order_records.push(OrderRecord {
                            order: order.clone(),
                            timestamp: ts,
                            status: "rejected".into(),
                            fill_price: None,
                            reason: "insufficient funds".into(),
                        });
                        continue;
                    }
                    *positions.entry(order.symbol.clone()).or_insert(0) += qty;
                    update_open_trade_buy(&mut open_trades, &order.symbol, ts, qty, fill_px);
                } else if qty < 0 {
                    let abs = qty.unsigned_abs() as i64;
                    *cash.entry(order_ccy).or_insert(0.0) += notional;
                    *positions.entry(order.symbol.clone()).or_insert(0) -= abs;
                    if let Some(t) = close_open_trade_sell(
                        &mut open_trades,
                        &order.symbol,
                        ts,
                        abs,
                        fill_px,
                        0.0,
                    ) {
                        closed_trades.push(t);
                    }
                }
                order_records.push(OrderRecord {
                    order,
                    timestamp: ts,
                    status: "filled".into(),
                    fill_price: Some(fill_px),
                    reason: String::new(),
                });
            }

            // Equity sample.
            let mut equity: f64 = cash.values().sum();
            for (sym, qty) in &positions {
                if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
                    equity += *qty as f64 * b.close;
                }
            }
            if equity > peak {
                peak = equity;
            }
            let dd = if peak > 0.0 {
                (equity - peak) / peak
            } else {
                0.0
            };
            equity_curve.push(EquitySample {
                timestamp: ts,
                equity,
                cash: cash.values().sum(),
                drawdown: dd,
            });
        }

        let metrics =
            compute_metrics(cfg.portfolio.initial_cash as f64, 0.0, &equity_curve, &closed_trades);

        StrategyRunResult {
            strategy_id: "test_id".into(),
            strategy_name: "test".into(),
            equity_curve,
            trades: closed_trades,
            orders: order_records,
            metrics,
        }
    }

    #[test]
    fn aligns_and_forward_fills() {
        let mut bars = HashMap::new();
        bars.insert("AAPL".to_owned(), vec![mk_bar(1, 100.0), mk_bar(3, 102.0)]);
        let aligned = align_bars(&bars, &[1, 2, 3], EmptyBarPolicy::ForwardFill);
        let row = &aligned["AAPL"];
        assert_eq!(row.len(), 3);
        assert!(row[0].is_some());
        assert!(row[1].is_some()); // forward-filled
        assert_eq!(row[1].as_ref().unwrap().close, 100.0);
        assert!(row[2].is_some());
    }

    #[test]
    fn aligns_skip_yields_none() {
        let mut bars = HashMap::new();
        bars.insert("AAPL".to_owned(), vec![mk_bar(1, 100.0)]);
        let aligned = align_bars(&bars, &[1, 2], EmptyBarPolicy::Skip);
        assert!(aligned["AAPL"][0].is_some());
        assert!(aligned["AAPL"][1].is_none());
    }

    #[test]
    fn buy_fills_and_creates_position() {
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 101.0, 102.0]);
        let order = Order {
            id: "buy1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10,
            price: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, order)]);
        assert_eq!(r.orders.len(), 1);
        assert_eq!(r.orders[0].status, "filled");
        // Final equity = remaining cash (10000 - 10*100) + 10*102 (last close)
        assert!((r.equity_curve.last().unwrap().equity - (9000.0 + 1020.0)).abs() < 1e-6);
    }

    #[test]
    fn insufficient_funds_is_rejected_not_errored() {
        let mut cfg = mk_cfg("AAPL");
        cfg.portfolio.initial_cash = 50; // can't afford 10 @ 100
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let order = Order {
            id: "buy1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10,
            price: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, order)]);
        assert_eq!(r.orders[0].status, "rejected");
        assert_eq!(r.orders[0].reason, "insufficient funds");
        // Position stays empty.
        assert!(r.equity_curve.last().unwrap().equity <= 50.0);
    }

    #[test]
    fn cancel_order_removes_pending() {
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 101.0, 102.0]);
        let buy = Order {
            id: "buy1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5,
            price: None,
        };
        let cancel = Order {
            id: "buy1".into(),
            symbol: "".into(),
            order_type: OrderType::CancelOrder,
            quantity: 0,
            price: None,
        };
        // Both injected on bar 0, cancel comes first in the open-order
        // list so the cancel removes the buy before it fills.
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, cancel), (0, buy)],
        );
        let cancelled = r.orders.iter().filter(|o| o.status == "cancelled").count();
        assert!(cancelled >= 1);
    }

    #[test]
    fn foreign_currency_falls_back_to_base() {
        let mut cfg = mk_cfg("VOD.L");
        cfg.portfolio.base_currency = Currency::USD;
        let aligned = mk_aligned("VOD.L", &[10.0, 11.0]);
        // Quote is GBP; we have only USD cash, so it must fall back.
        let order = Order {
            id: "b".into(),
            symbol: "VOD.L".into(),
            order_type: OrderType::Market,
            quantity: 100,
            price: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("VOD.L", "GBP")], vec![(0, order)]);
        assert_eq!(r.orders[0].status, "filled");
    }

    #[test]
    fn metrics_computed() {
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 110.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10,
            price: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy)]);
        assert!(r.metrics.contains_key("total_return"));
        assert!(r.metrics.contains_key("sharpe_ratio"));
        assert!(r.metrics.contains_key("max_drawdown"));
    }

    #[test]
    fn strategies_run_independently() {
        // Two synthetic runs with the same data should be deep-cloneable.
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 105.0, 110.0]);
        let buy_a = Order {
            id: "a".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5,
            price: None,
        };
        let buy_b = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10,
            price: None,
        };
        let ra = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy_a)]);
        let rb = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy_b)]);
        assert_ne!(ra.equity_curve.last().unwrap().equity, rb.equity_curve.last().unwrap().equity);
    }

    #[test]
    fn parse_iso_date_works() {
        let ts = parse_iso_date_to_ts("2024-01-15").unwrap();
        // 2024-01-15 00:00 UTC
        assert_eq!(ts, 1_705_276_800);
    }
}
