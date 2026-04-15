use crate::constants::Symbol;
use crate::data::models::currency::Currency;
use crate::data::models::exchange::Exchange;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::provider::Provider;
use pyo3::prelude::*;
use pyo3::types::PyString;
use serde::Deserialize;

/// A tradeable financial instrument.
///
/// Each instrument is uniquely identified by a [symbol][nom-symbol] and
/// belongs to exactly one [instrument type].
///
/// Attributes
/// ----------
/// symbol : str
///     Ticker symbol as used on the exchange.
///
/// name : str
///     Human-readable name of the instrument.
///
/// base : str | [Currency] | None
///     The currency of the tradeable instrument. Only defined for forex and
///     crypto pairs.
///
/// quote : str | [Currency]
///     The currency the instrument trades on.
///
/// instrument_type : [InstrumentType]
///     Instrument type this instrument belongs to.
///
/// exchange : str | [Exchange]
///     The exchange this instrument is listed in.
///
/// provider : [Provider]
///     The data provider that sourced this instrument.
///
/// See Also
/// --------
/// - backtide.data:Bar
/// - backtide.data:InstrumentProfile
/// - backtide.data:Interval
#[pyclass(from_py_object, get_all, frozen, module = "backtide.data")]
#[derive(Debug, Clone, Deserialize)]
pub struct Instrument {
    pub symbol: Symbol,
    pub name: String,
    pub base: Option<String>,
    pub quote: String,
    pub instrument_type: InstrumentType,
    pub exchange: String,
    pub provider: Provider,
}

#[pymethods]
impl Instrument {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (symbol, name, base, quote, instrument_type, exchange, provider))]
    fn new(
        symbol: Symbol,
        name: String,
        base: Option<String>,
        quote: String,
        instrument_type: InstrumentType,
        exchange: String,
        provider: Provider,
    ) -> Self {
        Self {
            symbol,
            name,
            base,
            quote,
            instrument_type,
            exchange,
            provider,
        }
    }

    #[allow(clippy::type_complexity)]
    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(
        Bound<'py, PyAny>,
        (
            Symbol,
            String,
            Option<String>,
            String,
            InstrumentType,
            String,
            Provider,
        ),
    )> {
        let cls = py.get_type::<Instrument>().into_any();
        Ok((
            cls,
            (
                self.symbol.clone(),
                self.name.clone(),
                self.base.clone(),
                self.quote.clone(),
                self.instrument_type,
                self.exchange.clone(),
                self.provider.clone(),
            ),
        ))
    }

    pub fn __repr__(&self) -> String {
        format!(
            "Instrument(symbol={:?}, name={:?}, base={}, quote={:?}, instrument_type={:?}, exchange={:?}, provider={:?})",
            self.symbol,
            self.name,
            self.base.as_deref().map_or("None".to_owned(), |s| format!("{s:?}")),
            self.quote,
            self.instrument_type.to_string(),
            self.exchange,
            self.provider,
        )
    }

    #[getter]
    pub fn base(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        match &self.base {
            None => Ok(None),
            Some(s) => Ok(Some(match s.parse::<Currency>() {
                Ok(c) => Py::new(py, c)?.into_any(),
                Err(_) => PyString::new(py, s).unbind().into_any(),
            })),
        }
    }

    #[getter]
    pub fn quote(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(match self.quote.parse::<Currency>() {
            Ok(c) => Py::new(py, c)?.into_any(),
            Err(_) => PyString::new(py, &self.quote).unbind().into_any(),
        })
    }

    #[getter]
    pub fn exchange(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(match self.exchange.parse::<Exchange>() {
            Ok(c) => Py::new(py, c)?.into_any(),
            Err(_) => PyString::new(py, &self.exchange).unbind().into_any(),
        })
    }
}
