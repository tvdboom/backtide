use crate::constants::Symbol;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use pyo3::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

/// A wrapper around an instrument with additional metadata.
///
/// Provides the information required to download an instrument, including the
/// download period and required currency conversions to reach the `base_currency`.
///
/// Attributes
/// ----------
/// instrument : [Instrument]
///     Instrument for which to provide the metadata.
///
/// earliest_ts : dict[[Interval], int]
///     Per interval, the earliest timestamp for which there is data (in UNIX
///     seconds).
///
/// latest_ts : dict[[Interval], int]
///     Per interval, the most recent timestamp for which there is data (in UNIX
///     seconds).
///
/// legs : list[str]
///     Symbols of the currency pairs required to convert from this instrument
///     to the base_currency.
///
/// See Also
/// --------
/// - backtide.data:Bar
/// - backtide.data:Instrument
/// - backtide.data:Interval
#[pyclass(from_py_object, get_all, frozen, module = "backtide.data")]
#[derive(Debug, Clone, Deserialize)]
pub struct InstrumentProfile {
    pub instrument: Instrument,
    pub earliest_ts: HashMap<Interval, u64>,
    pub latest_ts: HashMap<Interval, u64>,
    pub legs: Vec<Symbol>,
}

#[pymethods]
impl InstrumentProfile {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    fn new(
        instrument: Instrument,
        earliest_ts: HashMap<Interval, u64>,
        latest_ts: HashMap<Interval, u64>,
        legs: Vec<Symbol>,
    ) -> Self {
        Self {
            instrument,
            earliest_ts,
            latest_ts,
            legs,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(
        Bound<'py, PyAny>,
        (Instrument, HashMap<Interval, u64>, HashMap<Interval, u64>, Vec<Symbol>),
    )> {
        let cls = py.get_type::<Self>().into_any();
        Ok((
            cls,
            (
                self.instrument.clone(),
                self.earliest_ts.clone(),
                self.latest_ts.clone(),
                self.legs.to_vec(),
            ),
        ))
    }

    fn __repr__(&self) -> String {
        let earliest: Vec<String> =
            self.earliest_ts.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        let latest: Vec<String> = self.latest_ts.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        format!(
            "InstrumentProfile(instrument={}, earliest_ts={{{}}}, latest_ts={{{}}}, legs={:?})",
            self.instrument.__repr__(),
            earliest.join(", "),
            latest.join(", "),
            self.legs,
        )
    }

    #[getter]
    fn symbol(&self) -> &str {
        &self.instrument.symbol
    }
    #[getter]
    fn name(&self) -> &str {
        &self.instrument.name
    }
    #[getter]
    fn base(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.instrument.base(py)
    }
    #[getter]
    fn quote(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.instrument.quote(py)
    }
    #[getter]
    fn instrument_type(&self) -> InstrumentType {
        self.instrument.instrument_type
    }
    #[getter]
    fn exchange(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.instrument.exchange(py)
    }
    #[getter]
    fn provider(&self) -> Provider {
        self.instrument.provider
    }
}
