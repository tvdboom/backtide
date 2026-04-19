use crate::data::models::provider::Provider;
use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// The category an [`Instrument`] belongs to.
///
/// Attributes
/// ----------
/// is_equity : bool
///     Whether the instrument type has ownership stakes (true for stocks and ETF).
///
/// See Also
/// --------
/// - backtide.data:Bar
/// - backtide.data:Instrument
/// - backtide.data:Interval
#[pyclass(skip_from_py_object, frozen, eq, hash, module = "backtide.data")]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    Display,
    EnumIter,
    EnumString,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum InstrumentType {
    #[default]
    Stocks,
    Etf,
    Forex,
    Crypto,
}

impl InstrumentType {
    pub fn default_provider(&self) -> Provider {
        match self {
            Self::Stocks => Provider::Yahoo,
            Self::Etf => Provider::Yahoo,
            Self::Forex => Provider::Yahoo,
            Self::Crypto => Provider::Binance,
        }
    }
}

#[pymethods]
impl InstrumentType {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown instrument type: {s}")))
    }

    /// Make the class pickable (required by streamlit).
    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }
    pub fn __str__(&self) -> &'static str {
        match self {
            Self::Stocks => "Stocks",
            Self::Etf => "ETF",
            Self::Forex => "Forex",
            Self::Crypto => "Crypto",
        }
    }

    /// Return the default variant.
    ///
    /// Returns
    /// -------
    /// self
    ///     The default variant.
    #[staticmethod]
    fn get_default(py: Python<'_>) -> Py<Self> {
        Py::new(py, Self::Stocks).unwrap()
    }

    /// Return all variants.
    ///
    /// Returns
    /// -------
    /// list[self]
    ///     All variants of this type.
    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }

    /// Whether the instrument type has ownership stakes (true for stocks and etf).
    #[getter]
    pub fn is_equity(&self) -> bool {
        matches!(self, InstrumentType::Stocks | InstrumentType::Etf)
    }

    /// Material icon to visually represent this instrument type.
    ///
    /// Returns
    /// -------
    /// str
    ///     Material icon identifier.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Stocks => ":material/candlestick_chart:",
            Self::Etf => ":material/account_balance:",
            Self::Forex => ":material/currency_exchange:",
            Self::Crypto => ":material/currency_bitcoin:",
        }
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for InstrumentType {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<InstrumentType>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown instrument_type {s:?}.")))
    }
}

