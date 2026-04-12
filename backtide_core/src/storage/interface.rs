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
/// pandas.DataFrame
///     All bars currently held in the database.
///
/// See Also
/// --------
/// - backtide.data:download_instruments
/// - backtide.storage:delete_symbols
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

/// Return all stored dividend events as a pandas DataFrame.
///
/// Each row represents a single dividend payment. The DataFrame columns
/// are: `symbol`, `provider`, `ex_date`, and `amount`.
///
/// Returns
/// -------
/// pandas.DataFrame
///     All dividend events currently held in the database.
///
/// See Also
/// --------
/// - backtide.data:download_instruments
/// - backtide.storage:delete_symbols
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
/// Removes bars matching the given symbol(s) and optional filters. When no
/// bars remain for a symbol (scoped to the provider if one is given), any
/// associated dividend records are removed as well.
///
/// Parameters
/// ----------
/// symbol : str | Sequence[str]
///     The symbols to delete.
///
/// interval : str | [Interval] | None = None
///     The bar interval for which to remove the data. If `None`, all
///     intervals will be deleted.
///
/// provider : str | [Provider] | None = None
///     The data provider for which to remove the data. If `None`, all
///     providers will be deleted.
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
/// # Delete all daily bars for multiple symbols
/// delete_symbols(["BTC-USDT", "ETH-USDT"], interval="1d")  # norun
/// ```
#[pyfunction]
#[pyo3(signature = (symbol: "str | Sequence[str]", interval: "str | Interval | None"=None, provider: "str | Provider | None"=None))]
pub fn delete_symbols(
    symbol: Bound<'_, PyAny>,
    interval: Option<Bound<'_, PyAny>>,
    provider: Option<Bound<'_, PyAny>>,
) -> PyResult<u64> {
    let symbols: Vec<String> = if let Ok(s) = symbol.extract::<String>() {
        vec![s]
    } else {
        symbol.extract::<Vec<String>>()?
    };

    let provider = provider.map(|p| p.extract::<Provider>()).transpose()?;
    let interval = interval.map(|i| i.extract::<Interval>()).transpose()?;

    let engine = Engine::get()?;

    let mut total = 0u64;
    for sym in &symbols {
        total += engine.delete_symbols(sym, interval, provider)?;
    }
    Ok(total)
}
