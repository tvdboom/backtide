//! Python interface for the storage module.

use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::engine::Engine;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// Return all stored OHLCV bars as a pandas DataFrame.
///
/// Each row represents a single bar. The DataFrame columns are:
/// `symbol`, `instrument_type`, `interval`, `provider`, `open_ts`,
/// `close_ts`, `open_ts_exchange`, `open`, `high`, `low`, `close`,
/// `adj_close`, `volume`, and `n_trades`.
///
/// Returns
/// -------
/// pd.DataFrame
///     All bars currently held in the database.
///
/// See Also
/// --------
/// - backtide.storage:delete_symbols
/// - backtide.data:download_instruments
/// - backtide.storage:get_dividends
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import get_bars
///
/// df = get_bars()
/// print(df.head())
/// ```
#[pyfunction]
pub fn get_bars(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let engine = Engine::get()?;
    let rows = engine.get_all_bars()?;

    let n = rows.len();
    let mut symbols = Vec::with_capacity(n);
    let mut instrument_types = Vec::with_capacity(n);
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
        instrument_types.push(r.instrument_type);
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
    data.set_item("instrument_type", PyList::new(py, &instrument_types)?)?;
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

    let pd = py.import("pandas")?;
    let df = pd.call_method1("DataFrame", (data,))?;
    Ok(df.unbind())
}

/// Return a pre-aggregated summary of stored bars as a pandas DataFrame.
///
/// Each row represents one (symbol, interval, provider) series. The `sparkline`
/// column contains the last 365 `adj_close` values.
///
/// Returns
/// -------
/// pd.DataFrame
///     One summary row per stored series.
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import get_bars_summary
///
/// df = get_bars_summary()
/// print(df.head())
/// ```
#[pyfunction]
pub fn get_bars_summary(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let engine = Engine::get()?;
    let rows = engine.get_bars_summary()?;

    let n = rows.len();
    let mut symbols = Vec::with_capacity(n);
    let mut instrument_types = Vec::with_capacity(n);
    let mut intervals = Vec::with_capacity(n);
    let mut providers = Vec::with_capacity(n);
    let mut first_ts = Vec::with_capacity(n);
    let mut last_ts = Vec::with_capacity(n);
    let mut n_rows = Vec::with_capacity(n);
    let mut sparklines: Vec<Py<PyAny>> = Vec::with_capacity(n);

    for r in rows {
        symbols.push(r.symbol);
        instrument_types.push(r.instrument_type);
        intervals.push(r.interval);
        providers.push(r.provider);
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
    data.set_item("first_ts", PyList::new(py, &first_ts)?)?;
    data.set_item("last_ts", PyList::new(py, &last_ts)?)?;
    data.set_item("n_rows", PyList::new(py, &n_rows)?)?;
    data.set_item("sparkline", PyList::new(py, &sparklines)?)?;

    let pd = py.import("pandas")?;
    let df = pd.call_method1("DataFrame", (data,))?;
    Ok(df.unbind())
}

/// Return all stored dividend events as a pandas DataFrame.
///
/// Each row represents a single dividend payment. The DataFrame columns
/// are: `symbol`, `provider`, `ex_date`, and `amount`.
///
/// Returns
/// -------
/// pd.DataFrame
///     All dividend events currently held in the database.
///
/// See Also
/// --------
/// - backtide.storage:delete_symbols
/// - backtide.data:download_instruments
/// - backtide.storage:get_bars
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import get_dividends
///
/// df = get_dividends()
/// print(df.head())
/// ```
#[pyfunction]
pub fn get_dividends(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let engine = Engine::get()?;
    let rows = engine.get_all_dividends()?;

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

    let pd = py.import("pandas")?;
    let df = pd.call_method1("DataFrame", (data,))?;
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
/// - backtide.data:download_instruments
/// - backtide.storage:get_bars
/// - backtide.storage:get_dividends
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
