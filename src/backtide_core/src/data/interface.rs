//! Python interface for the data module.

use crate::constants::Symbol;
use crate::data::models::*;
use crate::engine::Engine;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyAnyMethods;
use pyo3::{pyfunction, Bound, FromPyObject, PyAny, PyResult, Python};

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

/// Fetch a recent, non-persisted daily-bar preview for one instrument.
///
/// The returned bars are requested directly from the selected provider and are
/// never written to Backtide storage.
///
/// Parameters
/// ----------
/// symbol : str
///     Canonical instrument symbol.
///
/// instrument_type : str | [InstrumentType]
///     Instrument type used to normalize the provider request.
///
/// provider : str | [Provider]
///     Provider from which to request the preview.
///
/// limit : int, default=30
///     Number of recent daily bars to return, from 2 through 60.
///
/// Returns
/// -------
/// tuple[[Instrument], list[[Bar]]]
///     Resolved instrument metadata and chronological daily bars.
///
/// Raises
/// ------
/// ValueError
///     If `symbol` is empty or `limit` is outside the supported range.
///
/// See Also
/// --------
/// - backtide.data:download_bars
/// - backtide.data:fetch_instruments
/// - backtide.data:resolve_profiles
#[pyfunction]
#[pyo3(signature = (symbol: "str", instrument_type: "str | InstrumentType", provider: "str | Provider", *, limit: "int"=30) -> "tuple[Instrument, list[Bar]]")]
pub fn fetch_bar_preview(
    py: Python<'_>,
    symbol: String,
    instrument_type: InstrumentType,
    provider: Provider,
    limit: usize,
) -> PyResult<(Instrument, Vec<Bar>)> {
    let symbol = symbol.trim().to_owned();
    if symbol.is_empty() {
        return Err(PyValueError::new_err("symbol must not be empty"));
    }
    if !(2..=60).contains(&limit) {
        return Err(PyValueError::new_err("limit must be between 2 and 60"));
    }

    if provider == Provider::Yahoo {
        let engine = Engine::get()?;
        py.detach(|| engine.fetch_bar_preview(symbol, instrument_type, provider, limit))
            .map_err(Into::into)
    } else {
        crate::live::interface::fetch_rest_bar_preview(py, provider, symbol, instrument_type, limit)
    }
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
/// start : int | None, default=None
///     Optional start of the download window (Unix timestamp, inclusive). When
///     given, per-instrument ranges are clamped so that no data before this timestamp
///     is requested. If `None`, it uses the provider's earliest available date.
///
/// end : int | None, default=None
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
/// profiles = resolve_profiles(["AAPL", "MSFT"], "stocks", "1d")
/// result = download_bars(profiles)
/// print(result)
/// ```
#[pyfunction]
#[pyo3(signature = (profiles: "list[InstrumentProfile]", start: "int | None"=None, end: "int | None"=None, *, verbose: "bool"=true))]
pub fn download_bars(
    py: Python<'_>,
    profiles: Vec<InstrumentProfile>,
    start: Option<u64>,
    end: Option<u64>,
    verbose: bool,
) -> PyResult<DownloadResult> {
    let engine = Engine::get()?;

    // Release the GIL so HTTP workers and browser clients can continue running.
    Ok(py.detach(|| engine.download_bars(&profiles, start, end, verbose))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::{PyInt, PyList, PyString};
    use pyo3::Py;

    fn instrument(symbol: &str) -> Instrument {
        Instrument {
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            base: None,
            quote: "USD".to_owned(),
            instrument_type: InstrumentType::Stocks,
            exchange: "XNAS".to_owned(),
            provider: Provider::Yahoo,
        }
    }

    #[test]
    fn parse_input_accepts_single_values_and_sequences() {
        Python::attach(|py| {
            let single = PyString::new(py, "1d").into_any();
            assert_eq!(parse_input::<Interval>(single).unwrap(), vec![Interval::OneDay]);

            let values = PyList::new(py, ["1m", "1h"]).unwrap().into_any();
            assert_eq!(
                parse_input::<Interval>(values).unwrap(),
                vec![Interval::OneMinute, Interval::OneHour]
            );

            let invalid = PyList::new(py, ["1d", "invalid"]).unwrap().into_any();
            assert!(parse_input::<Interval>(invalid).is_err());
        });
    }

    #[test]
    fn parse_instrument_accepts_strings_instruments_and_mixed_sequences() {
        Python::attach(|py| {
            assert_eq!(
                parse_instrument(PyString::new(py, "AAPL").into_any()).unwrap(),
                vec!["AAPL"]
            );

            let rust_instrument =
                Py::new(py, instrument("MSFT")).unwrap().into_bound(py).into_any();
            assert_eq!(parse_instrument(rust_instrument).unwrap(), vec!["MSFT"]);

            let apple = PyString::new(py, "AAPL").into_any().unbind();
            let microsoft = Py::new(py, instrument("MSFT")).unwrap().into_any();
            let mixed = PyList::new(py, [apple, microsoft]).unwrap().into_any();
            assert_eq!(parse_instrument(mixed).unwrap(), vec!["AAPL", "MSFT"]);

            assert!(parse_instrument(PyInt::new(py, 1).into_any()).is_err());
            let invalid = PyList::new(py, [PyInt::new(py, 1)]).unwrap().into_any();
            assert!(parse_instrument(invalid).is_err());
        });
    }

    #[test]
    fn fetch_bar_preview_validates_before_accessing_the_engine() {
        Python::attach(|py| {
            assert!(fetch_bar_preview(
                py,
                "  ".to_owned(),
                InstrumentType::Stocks,
                Provider::Yahoo,
                30,
            )
            .is_err());
            assert!(fetch_bar_preview(
                py,
                "AAPL".to_owned(),
                InstrumentType::Stocks,
                Provider::Yahoo,
                1,
            )
            .is_err());
            assert!(fetch_bar_preview(
                py,
                "AAPL".to_owned(),
                InstrumentType::Stocks,
                Provider::Yahoo,
                61,
            )
            .is_err());
        });
    }
}
