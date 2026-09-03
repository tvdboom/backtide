use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// Strategy with which to triangulate currencies.
///
/// With which approach to convert currencies to the `base_currency`. Read
/// more in the [user guide][currency-conversion].
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
pub enum TriangulationStrategy {
    #[default]
    Direct,
    Earliest,
}

#[pymethods]
impl TriangulationStrategy {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    fn __repr__(&self) -> String {
        self.to_string().to_lowercase()
    }
}

impl_python_enum_variants!(TriangulationStrategy);

impl<'a, 'py> FromPyObject<'a, 'py> for TriangulationStrategy {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<TriangulationStrategy>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse()
            .map_err(|_| PyValueError::new_err(format!("Unknown triangulation_strategy {s:?}.")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::{PyInt, PyString};

    #[test]
    fn triangulation_strategy_exposes_variants_and_python_extraction() {
        assert_eq!(TriangulationStrategy::default(), TriangulationStrategy::Direct);
        assert_eq!(TriangulationStrategy::Direct.__repr__(), "direct");
        assert_eq!(TriangulationStrategy::Earliest.__repr__(), "earliest");

        Python::attach(|py| {
            let direct =
                Py::new(py, TriangulationStrategy::Earliest).unwrap().into_bound(py).into_any();
            assert_eq!(
                direct.extract::<TriangulationStrategy>().unwrap(),
                TriangulationStrategy::Earliest
            );
            assert_eq!(
                PyString::new(py, "DIRECT").extract::<TriangulationStrategy>().unwrap(),
                TriangulationStrategy::Direct
            );
            assert!(PyString::new(py, "fastest").extract::<TriangulationStrategy>().is_err());
            assert!(PyInt::new(py, 1).extract::<TriangulationStrategy>().is_err());
        });
    }
}
