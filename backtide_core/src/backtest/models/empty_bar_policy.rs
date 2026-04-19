use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// How to handle bars with no trading activity.
///
/// Controls what the engine does when a bar has no market data.
///
/// Attributes
/// ----------
/// name : str
///     The human-readable display name of the variant.
///
/// See Also
/// --------
/// - backtide.data:Bar
/// - backtide.data:Interval
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
pub enum EmptyBarPolicy {
    Skip,
    #[default]
    ForwardFill,
    FillWithNaN,
}

#[pymethods]
impl EmptyBarPolicy {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown empty bar policy: {s}")))
    }

    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }
    pub fn __str__(&self) -> String {
        self.to_string()
    }

    /// The human-readable display name of the variant.
    #[getter]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Skip => "Skip",
            Self::ForwardFill => "Forward-fill",
            Self::FillWithNaN => "Fill with NaN",
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
        Py::new(py, Self::ForwardFill).unwrap()
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

impl<'a, 'py> FromPyObject<'a, 'py> for EmptyBarPolicy {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<EmptyBarPolicy>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown empty bar policy {s:?}.")))
    }
}
