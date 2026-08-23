use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// The period at which foreign currency balances are converted.
///
/// Used in combination with [`CurrencyConversionMode.EndOfPeriod`][CurrencyConversionMode]
/// to specify the frequency of automatic conversions.
///
/// See Also
/// --------
/// - backtide.data:Currency
/// - backtide.backtest:CurrencyConversionMode
/// - backtide.backtest:ExchangeExpConfig
#[pyclass(skip_from_py_object, frozen, eq, hash, module = "backtide.backtest")]
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
#[strum(ascii_case_insensitive)]
pub enum ConversionPeriod {
    #[default]
    Day,
    Week,
    Month,
    Year,
}

#[pymethods]
impl ConversionPeriod {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown conversion period: {s}")))
    }

    fn __repr__(&self) -> String {
        self.to_string().to_lowercase()
    }

    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }
}

impl_python_enum_variants!(ConversionPeriod, default);

impl<'a, 'py> FromPyObject<'a, 'py> for ConversionPeriod {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<ConversionPeriod>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown conversion period {s:?}.")))
    }
}
