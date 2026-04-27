//! Python interface for the data module.

use crate::constants::Symbol;
use crate::data::models::download_result::DownloadResult;
use crate::data::models::exchange::Exchange;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_profile::InstrumentProfile;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::engine::Engine;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyAnyMethods;
use pyo3::{pyfunction, Bound, FromPyObject, PyAny, PyResult};

// ────────────────────────────────────────────────────────────────────────────
// Helper functions
// ────────────────────────────────────────────────────────────────────────────

/// Parse input from Python into a vec of T.
fn parse_input<'py, T>(param: Bound<'py, PyAny>) -> PyResult<Vec<T>>
where
    for<'a> T: FromPyObject<'a, 'py>,
    for<'a> <T as FromPyObject<'a, 'py>>::Error: Into<pyo3::PyErr>,
{
    if let Ok(seq) = param.extract::<Vec<Bound<'py, PyAny>>>() {
        seq.iter().map(|item| item.extract::<T>().map_err(Into::into)).collect::<PyResult<_>>()
    } else {
        Ok(vec![param.extract::<T>().map_err(Into::into)?])
    }
}

/// Parse input from Python into a list of symbols.
fn parse_instrument(symbols: Bound<'_, PyAny>) -> PyResult<Vec<Symbol>> {
    if let Ok(seq) = symbols.extract::<Vec<Bound<'_, PyAny>>>() {
        // Parse symbols: Sequence[str | Instrument]
        seq.into_iter()
            .map(|item| {
                if let Ok(symbol) = item.extract::<String>() {
                    Ok(symbol)
                } else if let Ok(instr) = item.extract::<Instrument>() {
                    Ok(instr.symbol)
                } else {
                    Err(PyValueError::new_err(
                        "Parameter symbols must be a str, Instrument or a sequence of those.",
                    ))
                }
            })
            .collect::<PyResult<_>>()
    } else {
        // Parse symbols: str | Instrument
        if let Ok(symbol) = symbols.extract::<String>() {
            Ok(vec![symbol])
        } else if let Ok(instr) = symbols.extract::<Instrument>() {
            Ok(vec![instr.symbol])
        } else {
            Err(PyValueError::new_err(
                "Parameter symbols must be a str, Instrument or a sequence of those.",
            ))
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Public interface
// ────────────────────────────────────────────────────────────────────────────

/// Get instruments given their symbols.
///
/// Parameters
/// ----------
/// symbols : str | [Instrument] | list[str | [Instrument]]
///     Symbols for which to get the instruments. The symbols should be of the
///     [canonical form][canonical-symbols] expected by backtide.
///
/// instrument_type : str | [InstrumentType]
///     For which [instrument type] to get the instruments.
///
/// Returns
/// -------
/// list[[Instrument]]
///     Instruments corresponding to the provided symbols.
///
/// See Also
/// --------
/// - backtide.data:download_bars
/// - backtide.data:list_instruments
/// - backtide.data:resolve_profiles
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import fetch_instruments
///
/// print(fetch_instruments(["AAPL", "MSFT"], "stocks"))
/// ```
#[pyfunction]
#[pyo3(signature = (symbols: "str | Instrument | Sequence[str | Instrument]", instrument_type: "str | InstrumentType") -> "list[Instrument]")]
pub fn fetch_instruments(
    symbols: Bound<'_, PyAny>,
    instrument_type: Bound<'_, PyAny>,
) -> PyResult<Vec<Instrument>> {
    let symbols = parse_instrument(symbols)?;
    let instrument_type = instrument_type.extract::<InstrumentType>()?;

    let engine = Engine::get()?;
    Ok(engine.fetch_instruments(symbols, instrument_type)?)
}

/// Resolve the instrument profiles needed to download a set of symbols.
///
/// Resolves all instruments corresponding to the provided symbols. Also resolves
/// the required instruments to convert the given symbols to the base currency,
/// including any triangulation intermediaries. Returns a flat, deduplicated list.
///
/// Parameters
/// ----------
/// symbols : str | [Instrument] | list[str | [Instrument]]
///     Symbols for which to get the instruments. The symbols should be of the
///     [canonical form][canonical-symbols] expected by backtide.
///
/// instrument_type : str | [InstrumentType]
///     For which [instrument type] to get the instruments.
///
/// interval : str | [Interval] | list[str | [Interval]]
///     Interval(s) for which to resolve the download information.
///
/// verbose : bool, default=True
///     Whether to display a progress bar while resolving.
///
/// Returns
/// -------
/// list[[InstrumentProfile]]
///     Instrument profiles (direct instruments and currency legs, deduplicated).
///
/// See Also
/// --------
/// - backtide.data:download_bars
/// - backtide.data:fetch_instruments
/// - backtide.data:list_instruments
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import resolve_profiles
///
/// print(resolve_profiles(["AAPL", "MSFT"], "stocks", "1d"))
/// ```
#[pyfunction]
#[pyo3(signature = (symbols: "str | Instrument | Sequence[str | Instrument]", instrument_type: "str | InstrumentType", interval: "str | Interval | list[str | Interval]", *, verbose: "bool"=true))]
pub fn resolve_profiles(
    symbols: Bound<'_, PyAny>,
    instrument_type: Bound<'_, PyAny>,
    interval: Bound<'_, PyAny>,
    verbose: bool,
) -> PyResult<Vec<InstrumentProfile>> {
    let symbols = parse_instrument(symbols)?;
    let instrument_type = instrument_type.extract::<InstrumentType>()?;
    let interval = parse_input::<Interval>(interval)?;

    let engine = Engine::get()?;
    Ok(engine.resolve_profiles(symbols, instrument_type, interval, verbose)?)
}

/// List available instruments for a given instrument type.
///
/// When `exchanges` is provided, the `limit` is distributed evenly across the
/// specified exchanges.
///
/// Parameters
/// ----------
/// instrument_type : str | [InstrumentType]
///     For which [instrument type] to list the instruments.
///
/// exchange : str | [Exchange] | list[str | [Exchange]] | None, default=None
///     Optional exchange filter. If `None`, a default list of major exchanges is
///     used. If specified, only query those exchanges and distribute `limit` evenly
///     across them. This parameter is ignored for single-exchange providers.
///
/// limit : int, default=100
///     Maximum number of instruments to return. The actual number may be smaller,
///     but not larger.
///
/// verbose : bool, default=True
///     Whether to display a progress spinner in the terminal.
///
/// Returns
/// -------
/// list[[Instrument]]
///     Instruments for the given instrument type.
///
/// See Also
/// --------
/// - backtide.data:download_bars
/// - backtide.data:fetch_instruments
/// - backtide.data:resolve_profiles
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import list_instruments
///
/// print(list_instruments("crypto", limit=5))
/// ```
#[pyfunction]
#[pyo3(signature = (instrument_type: "str | InstrumentType", exchange: "str | Exchange | list[str | Exchange] | None"=None, *, limit: "int"=100, verbose: "bool"=true))]
pub fn list_instruments(
    instrument_type: Bound<'_, PyAny>,
    exchange: Option<Bound<'_, PyAny>>,
    limit: usize,
    verbose: bool,
) -> PyResult<Vec<Instrument>> {
    let instrument_type = instrument_type.extract::<InstrumentType>()?;
    let exchanges: Option<Vec<Exchange>> = exchange.map(parse_input::<Exchange>).transpose()?;

    let engine = Engine::get()?;
    Ok(engine.list_instruments(instrument_type, exchanges, limit, verbose)?)
}

/// Download OHLCV data for the instruments described in a list of profiles.
///
/// Concurrently downloads all instruments and legs, skipping data already stored
/// in the database.
///
/// Parameters
/// ----------
/// profiles : list[[InstrumentProfile]]
///     Resolved instrument profiles (run [`resolve_profiles`] first).
///
/// start : int or None, default=None
///     Optional start of the download window (Unix timestamp, inclusive). When
///     given, per-instrument ranges are clamped so that no data before this timestamp
///     is requested. If `None`, it uses the provider's earliest available date.
///
/// end : int or None, default=None
///     Optional end of the download window (Unix timestamp, exclusive). When
///     given, per-instrument ranges are clamped so that no data after this timestamp
///     is requested. If `None`, it uses the provider's latest available date.
///
/// verbose : bool, default=True
///     Whether to display a progress bar while downloading.
///
/// Returns
/// -------
/// [DownloadResult]
///     Summary of the download: succeeded/failed counts and per-task warnings.
///
/// See Also
/// --------
/// - backtide.storage:query_bars
/// - backtide.data:fetch_instruments
/// - backtide.data:resolve_profiles
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import resolve_profiles, download_bars
///
/// profiles = resolve_profiles(["AAPL", "MSFT"], "stocks", ["1h", "1d"])
/// result = download_bars(profiles)
/// print(result)
/// ```
#[pyfunction]
#[pyo3(signature = (profiles: "list[InstrumentProfile]", start: "int | None"=None, end: "int | None"=None, *, verbose: "bool"=true))]
pub fn download_bars(
    profiles: Vec<InstrumentProfile>,
    start: Option<u64>,
    end: Option<u64>,
    verbose: bool,
) -> PyResult<DownloadResult> {
    let engine = Engine::get()?;
    Ok(engine.download_bars(&profiles, start, end, verbose)?)
}
