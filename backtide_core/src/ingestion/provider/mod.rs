//! Data provider definitions.

use pyo3::pyclass;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumString};

mod traits;
pub mod yahoo;

/// A supported market data provider.
#[pyclass(from_py_object)]
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
}
