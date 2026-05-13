use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// The outcome status of a finished experiment.
///
/// See Also
/// --------
/// - backtide.backtest:ExperimentResult
/// - backtide.backtest:run_experiment
#[pyclass(skip_from_py_object, frozen, eq, hash, module = "backtide.backtest")]
#[derive(
    Clone,
    Copy,
    Debug,
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
pub enum ExperimentStatus {
    /// Every strategy completed without errors.
    Success,
    /// Some strategies succeeded and some failed.
    Partial,
    /// All strategies failed or the experiment could not run.
    Error,
}

#[pymethods]
impl ExperimentStatus {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown experiment status: {s}")))
    }

    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }
    pub fn __repr__(&self) -> String {
        self.to_string().to_lowercase()
    }

    /// A short human-readable description of this status.
    ///
    /// Returns
    /// -------
    /// str
    ///     Description of the variant.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Success => "Every strategy completed without errors.",
            Self::Partial => "Some strategies succeeded and some failed.",
            Self::Error => "All strategies failed or the experiment could not run.",
        }
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

impl<'a, 'py> FromPyObject<'a, 'py> for ExperimentStatus {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(bound) = obj.cast::<ExperimentStatus>() {
            return Ok(*bound.borrow());
        }
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown experiment status {s:?}.")))
    }
}
