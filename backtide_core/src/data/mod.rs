use crate::data::interface::{download_assets, get_assets, get_download_info, list_assets};
use crate::data::models::asset::Asset;
use crate::data::models::asset_meta::AssetMeta;
use crate::data::models::asset_type::AssetType;
use crate::data::models::bar::Bar;
use crate::data::models::country::Country;
use crate::data::models::currency::Currency;
use crate::data::models::download_info::DownloadInfo;
use crate::data::models::download_result::DownloadResult;
use crate::data::models::exchange::Exchange;
use crate::data::models::interval::Interval;
use models::provider::Provider;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

mod engine;
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
    m.add_class::<AssetMeta>()?;
    m.add_class::<Bar>()?;
    m.add_class::<Country>()?;
    m.add_class::<Currency>()?;
    m.add_class::<DownloadInfo>()?;
    m.add_class::<DownloadResult>()?;
    m.add_class::<Exchange>()?;
    m.add_class::<Interval>()?;
    m.add_class::<Provider>()?;

    m.add_function(wrap_pyfunction!(get_assets, &m)?)?;
    m.add_function(wrap_pyfunction!(get_download_info, &m)?)?;
    m.add_function(wrap_pyfunction!(list_assets, &m)?)?;
    m.add_function(wrap_pyfunction!(download_assets, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.data", &m)?;

    Ok(())
}
