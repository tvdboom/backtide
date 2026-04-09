use crate::data::models::provider::Provider;
use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// The broad category an [`Asset`] belongs to.
///
/// See Also
/// --------
/// - backtide.data:Asset
/// - backtide.data:Bar
/// - backtide.data:Interval
#[pyclass(skip_from_py_object, module = "backtide.data")]
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
pub enum AssetType {
    #[default]
    Stocks,
    Etf,
    Forex,
    Crypto,
}

impl AssetType {
    pub fn default(&self) -> Provider {
        match self {
            Self::Stocks => Provider::Yahoo,
            Self::Etf => Provider::Yahoo,
            Self::Forex => Provider::Yahoo,
            Self::Crypto => Provider::Binance,
        }
    }
}

#[pymethods]
impl AssetType {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown asset type: {s}")))
    }

    /// Make the class pickable (required by streamlit).
    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }

    fn __hash__(&self) -> u64 {
        *self as u64
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
    #[staticmethod]
    fn get_default(py: Python<'_>) -> Py<Self> {
        Py::new(py, Self::Stocks).unwrap()
    }

    /// Return all variants.
    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }

    /// Whether the asset type has ownership stakes (true for stocks and etf).
    #[getter]
    fn is_equity(&self) -> bool {
        matches!(self, AssetType::Stocks | AssetType::Etf)
    }

    /// Material icon to visually represent this asset type.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Stocks => ":material/candlestick_chart:",
            Self::Etf => ":material/account_balance:",
            Self::Forex => ":material/currency_exchange:",
            Self::Crypto => ":material/currency_bitcoin:",
        }
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for AssetType {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<AssetType>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown asset_type {s:?}.")))
    }
}
