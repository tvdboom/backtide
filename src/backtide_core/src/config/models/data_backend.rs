use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// DataFrame backend used for returning tabular data.
///
/// Controls which DataFrame library is used when storage functions return
/// tabular data. Read more in the [user guide][configuration].
///
/// Attributes
/// ----------
/// class_name : str
///     Return the Python class name.
#[pyclass(skip_from_py_object, frozen, eq, hash, module = "backtide.config")]
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
pub enum DataBackend {
    Numpy,
    #[default]
    Pandas,
    Polars,
}

#[pymethods]
impl DataBackend {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    fn __repr__(&self) -> String {
        self.to_string().to_lowercase()
    }

    /// Return the Python class name.
    #[getter]
    fn class_name(&self) -> &str {
        match self {
            DataBackend::Numpy => "np.ndarray",
            DataBackend::Pandas => "pd.DataFrame",
            DataBackend::Polars => "pl.DataFrame",
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

impl<'a, 'py> FromPyObject<'a, 'py> for DataBackend {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<DataBackend>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown data_backend {s:?}.")))
    }
}
