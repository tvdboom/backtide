use crate::data::interface::{get_asset, get_assets, list_assets, list_intervals};
use crate::data::models::asset::{Asset, AssetType};
use crate::data::models::bar::{Bar, Interval};
use crate::data::models::currency::Currency;
use crate::data::providers::provider::Provider;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

mod download;
pub mod errors;
mod interface;
pub mod models;
pub mod providers;
pub mod utils;

/// Register the Python interface for `backtide.core.data`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.data")?;

    m.add_class::<AssetType>()?;
    m.add_class::<Asset>()?;
    m.add_class::<Bar>()?;
    m.add_class::<Currency>()?;
    m.add_class::<Interval>()?;
    m.add_class::<Provider>()?;

    m.add_function(wrap_pyfunction!(get_asset, &m)?)?;
    m.add_function(wrap_pyfunction!(get_assets, &m)?)?;
    m.add_function(wrap_pyfunction!(list_assets, &m)?)?;
    m.add_function(wrap_pyfunction!(list_intervals, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.data", &m)?;

    Ok(())
}
