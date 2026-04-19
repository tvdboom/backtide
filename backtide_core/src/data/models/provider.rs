use crate::data::models::interval::Interval;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumString, IntoEnumIterator};

/// A supported market data provider.
///
/// See Also
/// --------
/// - backtide.data:Instrument
/// - backtide.data:InstrumentType
/// - backtide.data:Interval
#[pyclass(skip_from_py_object, frozen, eq, hash, module = "backtide.data")]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    Display,
    EnumString,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum Provider {
    Yahoo,
    Binance,
    Coinbase,
    Kraken,
}

#[pymethods]
impl Provider {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown provider {s:?}.")))
    }

    fn __repr__(&self) -> String {
        self.to_string().to_lowercase()
    }

    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Provider>().into_any();
        Ok((cls, (self.to_string(),)))
    }

    /// List the supported intervals.
    ///
    /// Returns
    /// -------
    /// list[[Interval]]
    ///     Supported intervals.
    fn intervals(&self) -> Vec<Interval> {
        match self {
            Provider::Coinbase => Interval::iter().filter(|i| *i != Interval::OneWeek).collect(),
            _ => Interval::iter().collect(),
        }
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for Provider {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<Provider>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown provider {s:?}.")))
    }
}

