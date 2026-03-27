//! Data provider definitions.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumString};

pub mod traits;
pub mod yahoo;

/// A supported market data provider.
#[pyclass(skip_from_py_object)]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    Display,
    EnumString,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum Provider {
    Yahoo,
    Binance,
    Kraken,
}

#[pymethods]
impl Provider {
    fn __repr__(&self) -> String {
        self.to_string()
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for Provider {
    type Error = PyErr;

    /// Parse the provider from a string.
    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        let s: String = obj.extract()?;
        s.parse().map_err(|_| {
            PyValueError::new_err(format!(
                "unknown provider {s:?}; expected one of: yahoo, binance, kraken"
            ))
        })
    }
}
