//! Implementation of the [`Provider`] enum.

use crate::data::models::currency::Currency;
use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, FromPyObject, PyAny, PyErr};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumString};

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
    Coinbase,
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

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, PyErr> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<Provider>() {
            return Ok(bound.borrow().clone());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown provider {s:?}.")))
    }
}
