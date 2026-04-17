//! Python interface for the storage module.

use crate::config::interface::Config;
use crate::config::models::dataframe_backend::DataframeBackend;
use crate::data::models::exchange::Exchange;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::engine::Engine;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

// ────────────────────────────────────────────────────────────────────────────
// Helper functions
// ────────────────────────────────────────────────────────────────────────────

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

/// Parse a Python value that may be a single item or a list into `Vec<T>`.
fn parse_one_or_many<'py, T>(value: Bound<'py, PyAny>) -> PyResult<Vec<T>>
where
    T: for<'a> pyo3::FromPyObject<'a, 'py>,
    for<'a> <T as pyo3::FromPyObject<'a, 'py>>::Error: Into<pyo3::PyErr>,
{
    if let Ok(seq) = value.extract::<Vec<Bound<'py, PyAny>>>() {
        seq.iter().map(|item| item.extract::<T>().map_err(Into::into)).collect::<PyResult<Vec<_>>>()
    } else {
        Ok(vec![value.extract::<T>().map_err(Into::into)?])
    }
}

/// Build a column as a [`PyList`] by mapping a field over a slice of rows.
macro_rules! col {
    ($py:expr, $rows:expr, $f:expr) => {
        PyList::new($py, $rows.iter().map($f))?
    };
}

/// Build a [`PyDict`] column-store from a slice of rows and convert it to a
/// dataframe in a single expression.
macro_rules! to_df {
    ($py:expr, $rows:expr, { $($key:literal => $f:expr),* $(,)? }) => {{
        let data = PyDict::new($py);
        $(data.set_item($key, col!($py, $rows, $f))?;)*
        dict_to_dataframe($py, &data).map(Bound::unbind)
    }};
}

// ────────────────────────────────────────────────────────────────────────────
// Public interface
// ────────────────────────────────────────────────────────────────────────────

/// Return stored instrument metadata, optionally filtered.
///
/// When called with no arguments, returns all instruments. When
/// `instrument_type`, `provider`, and/or `exchange` are given, only
/// matching rows are returned.
///
/// Parameters
/// ----------
/// instrument_type : str | InstrumentType | list[str | InstrumentType] | None, default=None
///     Filter by instrument type. Accepts a single value or a list.
///
/// provider : str | Provider | list[str | Provider] | None, default=None
///     Filter by data provider. Accepts a single value or a list.
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
#[pyo3(signature = (instrument_type: "str | InstrumentType | list[str | InstrumentType] | None"=None, provider: "str | Provider | list[str | Provider] | None"=None, exchange: "str | Exchange | list[str | Exchange] | None"=None, *, limit: "int | None"=None))]
pub fn query_instruments(
    instrument_type: Option<Bound<'_, PyAny>>,
    provider: Option<Bound<'_, PyAny>>,
    exchange: Option<Bound<'_, PyAny>>,
    limit: Option<usize>,
) -> PyResult<Vec<Instrument>> {
    let its: Option<Vec<InstrumentType>> = instrument_type.map(parse_one_or_many).transpose()?;
    let provs: Option<Vec<Provider>> = provider.map(parse_one_or_many).transpose()?;
    let exchanges: Option<Vec<Exchange>> = exchange.map(parse_one_or_many).transpose()?;

    let engine = Engine::get()?;
    Ok(engine.query_instruments(its.as_deref(), provs.as_deref(), exchanges.as_deref(), limit)?)
}

/// Return stored OHLCV bars as a dataframe.
///
/// Each row represents a single bar. The dataframe columns are:
/// `symbol`, `interval`, `provider`, `open_ts`, `close_ts`,
/// `open_ts_exchange`, `open`, `high`, `low`, `close`, `adj_close`,
/// `volume`, and `n_trades`.
///
/// Parameters
/// ----------
/// symbol : str | list[str] | None, default=None
///     Filter by symbol. Accepts a single symbol or a list. `None` returns all.
///
/// interval : str | Interval | list[str | Interval] | None, default=None
///     Filter by bar interval. Accepts a single value or a list.
///
/// provider : str | Provider | list[str | Provider] | None, default=None
///     Filter by data provider. Accepts a single value or a list.
///
/// limit : int | None, default=None
///     Maximum number of rows to return. `None` means no limit.
///
/// Returns
/// -------
/// pd.DataFrame | pl.DataFrame
///     Matching bars from the database.
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
#[pyo3(signature = (symbol: "str | list[str] | None"=None, interval: "str | Interval | list[str | Interval] | None"=None, provider: "str | Provider | list[str | Provider] | None"=None, *, limit: "int | None"=None))]
pub fn query_bars(
    py: Python<'_>,
    symbol: Option<Bound<'_, PyAny>>,
    interval: Option<Bound<'_, PyAny>>,
    provider: Option<Bound<'_, PyAny>>,
    limit: Option<usize>,
) -> PyResult<Py<PyAny>> {
    let symbols: Option<Vec<String>> = symbol.map(parse_one_or_many).transpose()?;
    let intervals: Option<Vec<Interval>> = interval.map(parse_one_or_many).transpose()?;
    let providers: Option<Vec<Provider>> = provider.map(parse_one_or_many).transpose()?;

    let sym_refs: Option<Vec<&str>> =
        symbols.as_ref().map(|v| v.iter().map(|s| s.as_str()).collect());
    let rows = Engine::get()?.query_bars(
        sym_refs.as_deref(),
        intervals.as_deref(),
        providers.as_deref(),
        limit,
    )?;

    to_df!(py, rows, {
        "symbol"           => |r| &r.symbol,
        "interval"         => |r| &r.interval,
        "provider"         => |r| &r.provider,
        "open_ts"          => |r| r.bar.open_ts,
        "close_ts"         => |r| r.bar.close_ts,
        "open_ts_exchange" => |r| r.bar.open_ts_exchange,
        "open"             => |r| r.bar.open,
        "high"             => |r| r.bar.high,
        "low"              => |r| r.bar.low,
        "close"            => |r| r.bar.close,
        "adj_close"        => |r| r.bar.adj_close,
        "volume"           => |r| r.bar.volume,
        "n_trades"         => |r| r.bar.n_trades,
    })
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
    let rows = Engine::get()?.query_bars_summary()?;

    // Sparklines are nested PyLists — materialise them fallibly up front
    // since the col! / to_df! macros cannot propagate errors from inner
    // PyList::new calls.
    let sparklines = rows
        .iter()
        .map(|r| PyList::new(py, &r.sparkline).map(Bound::unbind))
        .collect::<PyResult<Vec<_>>>()?;

    let data = PyDict::new(py);
    data.set_item("symbol", col!(py, rows, |r| &r.symbol))?;
    data.set_item("instrument_type", col!(py, rows, |r| &r.instrument_type))?;
    data.set_item("interval", col!(py, rows, |r| &r.interval))?;
    data.set_item("provider", col!(py, rows, |r| &r.provider))?;
    data.set_item("name", col!(py, rows, |r| r.name.as_deref().unwrap_or_default()))?;
    data.set_item("base", col!(py, rows, |r| r.base.as_deref().unwrap_or_default()))?;
    data.set_item("quote", col!(py, rows, |r| r.quote.as_deref().unwrap_or_default()))?;
    data.set_item("exchange", col!(py, rows, |r| r.exchange.as_deref().unwrap_or_default()))?;
    data.set_item("first_ts", col!(py, rows, |r| r.first_ts))?;
    data.set_item("last_ts", col!(py, rows, |r| r.last_ts))?;
    data.set_item("n_rows", col!(py, rows, |r| r.n_rows))?;
    data.set_item("sparkline", PyList::new(py, &sparklines)?)?;
    dict_to_dataframe(py, &data).map(Bound::unbind)
}

/// Return stored dividend events as a dataframe.
///
/// Each row represents a single dividend payment. The DataFrame columns
/// are: `symbol`, `provider`, `ex_date`, and `amount`.
///
/// Parameters
/// ----------
/// symbol : str | list[str] | None, default=None
///     Filter by symbol. Accepts a single symbol or a list. ``None`` returns all.
///
/// provider : str | Provider | list[str | Provider] | None, default=None
///     Filter by data provider. Accepts a single value or a list.
///
/// limit : int | None, default=None
///     Maximum number of rows to return. ``None`` means no limit.
///
/// Returns
/// -------
/// pd.DataFrame | pl.DataFrame
///     Matching dividend events from the database.
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
#[pyo3(signature = (symbol: "str | list[str] | None"=None, provider: "str | Provider | list[str | Provider] | None"=None, *, limit: "int | None"=None))]
pub fn query_dividends(
    py: Python<'_>,
    symbol: Option<Bound<'_, PyAny>>,
    provider: Option<Bound<'_, PyAny>>,
    limit: Option<usize>,
) -> PyResult<Py<PyAny>> {
    let symbols: Option<Vec<String>> = symbol.map(parse_one_or_many).transpose()?;
    let providers: Option<Vec<Provider>> = provider.map(parse_one_or_many).transpose()?;

    let sym_refs: Option<Vec<&str>> =
        symbols.as_ref().map(|v| v.iter().map(|s| s.as_str()).collect());
    let rows = Engine::get()?.query_dividends(sym_refs.as_deref(), providers.as_deref(), limit)?;

    to_df!(py, rows, {
        "symbol"   => |r| &r.symbol,
        "provider" => |r| &r.provider,
        "ex_date"  => |r| r.dividend.ex_date,
        "amount"   => |r| r.dividend.amount,
    })
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
#[pyo3(signature = (symbol: "str | list[str] | None"=None, interval: "str | Interval | None"=None, provider: "str | Provider | None"=None, *, series: "list[tuple[str, str, str]] | None"=None))]
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

    Ok(Engine::get()?.delete_symbols(&tuples)?)
}
