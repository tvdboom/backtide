use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// How foreign currency proceeds are converted back to the base currency.
///
/// Determines the timing and conditions under which non-base-currency
/// balances are exchanged. The chosen mode affects cash flow timing
/// and may influence simulation results when exchange rates fluctuate.
///
/// Attributes
/// ----------
/// name : str
///     The human-readable display name of the variant.
///
/// See Also
/// --------
/// - backtide.backtest:ConversionPeriod
/// - backtide.data:Currency
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
pub enum CurrencyConversionMode {
    #[default]
    Immediate,
    HoldUntilThreshold,
    EndOfPeriod,
    CustomInterval,
}

#[pymethods]
impl CurrencyConversionMode {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse()
            .map_err(|_| PyValueError::new_err(format!("Unknown currency conversion mode: {s}")))
    }

    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }
    fn __repr__(&self) -> String {
        self.to_string()
    }

    /// The human-readable display name of the variant.
    #[getter]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Immediate => "Immediately convert to base currency",
            Self::HoldUntilThreshold => "Hold until threshold, then convert",
            Self::EndOfPeriod => "Convert at end of period",
            Self::CustomInterval => "Convert at custom interval",
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
        Py::new(py, Self::default()).unwrap()
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
}

impl<'a, 'py> FromPyObject<'a, 'py> for CurrencyConversionMode {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<CurrencyConversionMode>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse()
            .map_err(|_| PyValueError::new_err(format!("Unknown currency conversion mode {s:?}.")))
    }
}
