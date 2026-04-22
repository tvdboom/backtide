use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// DataFrame library used for returning tabular data.
///
/// Which library to use for tabular data exchanged with user code (e.g.,
/// storage query results, indicator inputs/outputs). Read more in the
/// [user guide][configuration].
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
pub enum DataFrameLibrary {
    Numpy,
    #[default]
    Pandas,
    Polars,
}

#[pymethods]
impl DataFrameLibrary {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    fn __repr__(&self) -> String {
        self.to_string().to_lowercase()
    }

    /// Return the Python class name.
    #[getter]
    fn class_name(&self) -> &str {
        match self {
            DataFrameLibrary::Numpy => "np.ndarray",
            DataFrameLibrary::Pandas => "pd.DataFrame",
            DataFrameLibrary::Polars => "pl.DataFrame",
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

impl<'a, 'py> FromPyObject<'a, 'py> for DataFrameLibrary {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<DataFrameLibrary>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown dataframe_library {s:?}.")))
    }
}

