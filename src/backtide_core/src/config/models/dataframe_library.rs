use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// DataFrame library used by the engine's frontend.
///
/// Tabular data exchanged with user code (e.g., storage query results,
/// indicator and strategies inputs/outputs). Read more in the
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
            DataFrameLibrary::Pandas => "pd.DataFrame",
            DataFrameLibrary::Polars => "pl.DataFrame",
        }
    }
}

impl_python_enum_variants!(DataFrameLibrary);

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

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::{PyInt, PyString};

    #[test]
    fn dataframe_library_exposes_metadata_and_python_extraction() {
        assert_eq!(DataFrameLibrary::default(), DataFrameLibrary::Pandas);
        assert_eq!(DataFrameLibrary::Pandas.__repr__(), "pandas");
        assert_eq!(DataFrameLibrary::Pandas.class_name(), "pd.DataFrame");
        assert_eq!(DataFrameLibrary::Polars.__repr__(), "polars");
        assert_eq!(DataFrameLibrary::Polars.class_name(), "pl.DataFrame");

        Python::attach(|py| {
            let direct = Py::new(py, DataFrameLibrary::Polars).unwrap().into_bound(py).into_any();
            assert_eq!(direct.extract::<DataFrameLibrary>().unwrap(), DataFrameLibrary::Polars);
            assert_eq!(
                PyString::new(py, "PANDAS").extract::<DataFrameLibrary>().unwrap(),
                DataFrameLibrary::Pandas
            );
            assert!(PyString::new(py, "arrow").extract::<DataFrameLibrary>().is_err());
            assert!(PyInt::new(py, 1).extract::<DataFrameLibrary>().is_err());
        });
    }
}
