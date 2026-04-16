//! Python interface for the storage module.

use crate::config::interface::Config;
use crate::config::models::dataframe_backend::DataframeBackend;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::engine::Engine;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// Build a DataFrame from a Python dict, using the configured backend.
fn dict_to_dataframe<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    match Config::get()?.display.dataframe_backend {
        DataframeBackend::Pandas => {
            let pd = py.import("pandas")?;
            pd.call_method1("DataFrame", (data,))
        },
        DataframeBackend::Polars => {
            let pl = py.import("polars")?;
            pl.call_method1("from_dict", (data,))
        },
    }
}

/// Flatten `Option<String>` to `String`, replacing `None` with `""`.
#[inline]
fn opt_str(v: Option<String>) -> String {
    v.unwrap_or_default()
}

/// Return all stored OHLCV bars as a dataframe.
///
/// Each row represents a single bar. The dataframe columns are:
/// `symbol`, `interval`, `provider`, `open_ts`, `close_ts`,
/// `open_ts_exchange`, `open`, `high`, `low`, `close`, `adj_close`,
/// `volume`, and `n_trades`.
///
/// Returns
/// -------
/// pd.DataFrame | pl.DataFrame
///     All bars currently held in the database.
///
/// See Also
/// --------
/// - backtide.data:download_bars
/// - backtide.storage:query_bars_summary
/// - backtide.storage:query_dividends
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import query_bars
///
/// df = query_bars()
/// print(df.head())
/// ```
#[pyfunction]
pub fn query_bars(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let engine = Engine::get()?;
    let rows = engine.query_bars(None, None, None, None)?;

    let n = rows.len();
    let mut symbols = Vec::with_capacity(n);
    let mut intervals = Vec::with_capacity(n);
    let mut providers = Vec::with_capacity(n);
    let mut open_ts = Vec::with_capacity(n);
    let mut close_ts = Vec::with_capacity(n);
    let mut open_ts_exchange = Vec::with_capacity(n);
    let mut open = Vec::with_capacity(n);
    let mut high = Vec::with_capacity(n);
    let mut low = Vec::with_capacity(n);
    let mut close = Vec::with_capacity(n);
    let mut adj_close = Vec::with_capacity(n);
    let mut volume = Vec::with_capacity(n);
    let mut n_trades: Vec<Option<i32>> = Vec::with_capacity(n);

    for r in rows {
        symbols.push(r.symbol);
        intervals.push(r.interval);
        providers.push(r.provider);
        open_ts.push(r.bar.open_ts);
        close_ts.push(r.bar.close_ts);
        open_ts_exchange.push(r.bar.open_ts_exchange);
        open.push(r.bar.open);
        high.push(r.bar.high);
        low.push(r.bar.low);
        close.push(r.bar.close);
        adj_close.push(r.bar.adj_close);
        volume.push(r.bar.volume);
        n_trades.push(r.bar.n_trades);
    }

    let data = PyDict::new(py);
    data.set_item("symbol", PyList::new(py, &symbols)?)?;
    data.set_item("interval", PyList::new(py, &intervals)?)?;
    data.set_item("provider", PyList::new(py, &providers)?)?;
    data.set_item("open_ts", PyList::new(py, &open_ts)?)?;
    data.set_item("close_ts", PyList::new(py, &close_ts)?)?;
    data.set_item("open_ts_exchange", PyList::new(py, &open_ts_exchange)?)?;
    data.set_item("open", PyList::new(py, &open)?)?;
    data.set_item("high", PyList::new(py, &high)?)?;
    data.set_item("low", PyList::new(py, &low)?)?;
    data.set_item("close", PyList::new(py, &close)?)?;
    data.set_item("adj_close", PyList::new(py, &adj_close)?)?;
    data.set_item("volume", PyList::new(py, &volume)?)?;
    data.set_item("n_trades", PyList::new(py, &n_trades)?)?;

    let df = dict_to_dataframe(py, &data)?;
    Ok(df.unbind())
}

/// Return a pre-aggregated summary of stored bars as a dataframe.
///
/// Each row represents one (symbol, interval, provider) series. The `sparkline`
/// column contains the last 365 `adj_close` values.
///
/// Returns
/// -------
/// pd.DataFrame | pl.DataFrame
///     One summary row per stored series.
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import query_bars_summary
///
/// df = query_bars_summary()
/// print(df.head())
/// ```
#[pyfunction]
pub fn query_bars_summary(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let engine = Engine::get()?;
    let rows = engine.query_bars_summary()?;

    let n = rows.len();
    let mut symbols = Vec::with_capacity(n);
    let mut instrument_types = Vec::with_capacity(n);
    let mut intervals = Vec::with_capacity(n);
    let mut providers = Vec::with_capacity(n);
    let mut names = Vec::with_capacity(n);
    let mut bases = Vec::with_capacity(n);
    let mut quotes = Vec::with_capacity(n);
    let mut exchanges = Vec::with_capacity(n);
    let mut first_ts = Vec::with_capacity(n);
    let mut last_ts = Vec::with_capacity(n);
    let mut n_rows = Vec::with_capacity(n);
    let mut sparklines: Vec<Py<PyAny>> = Vec::with_capacity(n);

    for r in rows {
        symbols.push(r.symbol);
        instrument_types.push(r.instrument_type);
        intervals.push(r.interval);
        providers.push(r.provider);
        names.push(opt_str(r.name));
        bases.push(opt_str(r.base));
        quotes.push(opt_str(r.quote));
        exchanges.push(opt_str(r.exchange));
        first_ts.push(r.first_ts);
        last_ts.push(r.last_ts);
        n_rows.push(r.n_rows);
        sparklines.push(PyList::new(py, &r.sparkline)?.unbind().into());
    }

    let data = PyDict::new(py);
    data.set_item("symbol", PyList::new(py, &symbols)?)?;
    data.set_item("instrument_type", PyList::new(py, &instrument_types)?)?;
    data.set_item("interval", PyList::new(py, &intervals)?)?;
    data.set_item("provider", PyList::new(py, &providers)?)?;
    data.set_item("name", PyList::new(py, &names)?)?;
    data.set_item("base", PyList::new(py, &bases)?)?;
    data.set_item("quote", PyList::new(py, &quotes)?)?;
    data.set_item("exchange", PyList::new(py, &exchanges)?)?;
    data.set_item("first_ts", PyList::new(py, &first_ts)?)?;
    data.set_item("last_ts", PyList::new(py, &last_ts)?)?;
    data.set_item("n_rows", PyList::new(py, &n_rows)?)?;
    data.set_item("sparkline", PyList::new(py, &sparklines)?)?;

    let df = dict_to_dataframe(py, &data)?;
    Ok(df.unbind())
}

/// Return all stored dividend events as a dataframe.
///
/// Each row represents a single dividend payment. The DataFrame columns
/// are: `symbol`, `provider`, `ex_date`, and `amount`.
///
/// Returns
/// -------
/// pd.DataFrame | pl.DataFrame
///     All dividend events currently held in the database.
///
/// See Also
/// --------
/// - backtide.storage:delete_symbols
/// - backtide.data:download_bars
/// - backtide.storage:query_bars
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import query_dividends
///
/// df = query_dividends()
/// print(df.head())
/// ```
#[pyfunction]
pub fn query_dividends(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let engine = Engine::get()?;
    let rows = engine.query_dividends(None, None, None)?;

    let n = rows.len();
    let mut symbols = Vec::with_capacity(n);
    let mut providers = Vec::with_capacity(n);
    let mut ex_dates = Vec::with_capacity(n);
    let mut amounts = Vec::with_capacity(n);

    for r in rows {
        symbols.push(r.symbol);
        providers.push(r.provider);
        ex_dates.push(r.dividend.ex_date);
        amounts.push(r.dividend.amount);
    }

    let data = PyDict::new(py);
    data.set_item("symbol", PyList::new(py, &symbols)?)?;
    data.set_item("provider", PyList::new(py, &providers)?)?;
    data.set_item("ex_date", PyList::new(py, &ex_dates)?)?;
    data.set_item("amount", PyList::new(py, &amounts)?)?;

    let df = dict_to_dataframe(py, &data)?;
    Ok(df.unbind())
}

/// Delete bars (and orphaned dividends) from the database.
///
/// Accepts either individual arguments for a single symbol (or list of
/// symbols), or a `series` list of `(symbol, interval, provider)` triples
/// for bulk deletion. All deletions run in a single database transaction.
///
/// Parameters
/// ----------
/// symbol : str | list[str] | None = None
///     One or more symbols to delete. Mutually exclusive with `series`.
///
/// interval : str | [Interval] | None = None
///     The bar interval to remove. Applies to every symbol when `symbol`
///     is given. Ignored when `series` is given.
///
/// provider : str | [Provider] | None = None
///     The data provider to remove. Applies to every symbol when `symbol`
///     is given. Ignored when `series` is given.
///
/// series : list[tuple[str, str, str]] | None = None
///     Explicit list of `(symbol, interval, provider)` triples to delete.
///     Mutually exclusive with `symbol`.
///
/// Returns
/// -------
/// int
///     Number of bar rows deleted.
///
/// See Also
/// --------
/// - backtide.data:download_bars
/// - backtide.storage:query_bars
/// - backtide.storage:query_dividends
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import delete_symbols
///
/// # Delete all stored data for a single symbol
/// delete_symbols("AAPL")  # norun
///
/// # Delete daily bars for multiple symbols
/// delete_symbols(["BTC-USDT", "ETH-USDT"], interval="1d")  # norun
///
/// # Bulk-delete specific series
/// delete_symbols(series=[("AAPL", "1d", "yahoo"), ("MSFT", "1h", "yahoo")])  # norun
/// ```
#[pyfunction]
#[pyo3(signature = (symbol=None, interval=None, provider=None, *, series=None))]
pub fn delete_symbols(
    symbol: Option<Bound<'_, PyAny>>,
    interval: Option<Bound<'_, PyAny>>,
    provider: Option<Bound<'_, PyAny>>,
    series: Option<Vec<(String, String, String)>>,
) -> PyResult<u64> {
    let tuples: Vec<(String, Option<Interval>, Option<Provider>)> = if let Some(series) = series {
        // Bulk mode: each triple has explicit (symbol, interval, provider).
        series
            .into_iter()
            .map(|(sym, iv, prov)| {
                let interval: Interval = iv.parse().map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("invalid interval: {e}"))
                })?;
                let provider: Provider = prov.parse().map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("invalid provider: {e}"))
                })?;
                Ok((sym, Some(interval), Some(provider)))
            })
            .collect::<PyResult<Vec<_>>>()?
    } else if let Some(symbol) = symbol {
        // Legacy mode: (symbol(s), optional interval, optional provider).
        let symbols: Vec<String> = if let Ok(s) = symbol.extract::<String>() {
            vec![s]
        } else {
            symbol.extract::<Vec<String>>()?
        };
        let provider = provider.map(|p| p.extract::<Provider>()).transpose()?;
        let interval = interval.map(|i| i.extract::<Interval>()).transpose()?;
        symbols.into_iter().map(|s| (s, interval, provider)).collect()
    } else {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "Either `symbol` or `series` must be provided.",
        ));
    };

    let engine = Engine::get()?;
    Ok(engine.delete_symbols(&tuples)?)
}

/// Return stored instrument metadata, optionally filtered.
///
/// When called with no arguments, returns all instruments. When
/// ``instrument_type``, ``provider``, and/or ``exchange`` are given, only
/// matching rows are returned.
///
/// Parameters
/// ----------
/// instrument_type : str | InstrumentType | None, default=None
///     Filter by instrument type.
///
/// provider : str | Provider | None, default=None
///     Filter by data provider.
///
/// exchange : str | Exchange | list[str | Exchange] | None, default=None
///     Filter by exchange. Accepts a single exchange or a list.
///
/// limit : int | None, default=None
///     Maximum number of instruments to return. ``None`` means no limit.
///
/// Returns
/// -------
/// list[Instrument]
///     Matching instruments from the database.
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import query_instruments
///
/// # All instruments
/// all_instruments = query_instruments()
///
/// # Filtered
/// stocks = query_instruments("stocks", "yahoo", limit=100)
///
/// # Filtered by exchange
/// nyse = query_instruments("stocks", exchange="XNYS")
/// ```
#[pyfunction]
#[pyo3(signature = (instrument_type=None, provider=None, exchange=None, *, limit=None))]
pub fn query_instruments(
    instrument_type: Option<Bound<'_, PyAny>>,
    provider: Option<Bound<'_, PyAny>>,
    exchange: Option<Bound<'_, PyAny>>,
    limit: Option<usize>,
) -> PyResult<Vec<crate::data::models::instrument::Instrument>> {
    use crate::data::models::exchange::Exchange;
    use crate::data::models::instrument_type::InstrumentType;

    let it = instrument_type.map(|v| v.extract::<InstrumentType>()).transpose()?;
    let prov = provider.map(|v| v.extract::<Provider>()).transpose()?;

    let exchanges: Option<Vec<Exchange>> = exchange
        .map(|v| {
            if let Ok(seq) = v.extract::<Vec<Bound<'_, PyAny>>>() {
                seq.iter().map(|item| item.extract::<Exchange>()).collect::<PyResult<Vec<_>>>()
            } else {
                Ok(vec![v.extract::<Exchange>()?])
            }
        })
        .transpose()?;

    let engine = Engine::get()?;
    Ok(engine.query_instruments(it, prov, exchanges.as_deref(), limit)?)
}
