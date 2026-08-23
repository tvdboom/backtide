use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// How trading commissions are calculated.
///
/// Each variant represents a different fee structure applied to
/// every executed order during the simulation.
///
/// See Also
/// --------
/// - backtide.data:Currency
/// - backtide.backtest:ExchangeExpConfig
/// - backtide.backtest:OrderType
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
pub enum CommissionType {
    #[default]
    Percentage,
    Fixed,
    #[strum(serialize = "PercentagePlusFixed")]
    PercentagePlusFixed,
}

#[pymethods]
impl CommissionType {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown commission type: {s}")))
    }

    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }
    pub fn __str__(&self) -> &'static str {
        match self {
            Self::Percentage => "Percentage (%)",
            Self::Fixed => "Fixed amount",
            Self::PercentagePlusFixed => "Percentage + Fixed",
        }
    }
}

impl_python_enum_variants!(CommissionType, default);

impl<'a, 'py> FromPyObject<'a, 'py> for CommissionType {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<CommissionType>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown commission type {s:?}.")))
    }
}
