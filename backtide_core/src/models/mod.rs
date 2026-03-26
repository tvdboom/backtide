//! Data models module.

use crate::models::asset::{Asset, AssetType};
use crate::models::currency::Currency;
use pyo3::prelude::*;
use crate::models::bar::{Bar, Interval};

pub mod asset;
pub mod bar;
pub mod currency;

/// Register all ingestion types to `backtide.core.ingestion`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.models")?;

    m.add_class::<AssetType>()?;
    m.add_class::<Asset>()?;
    m.add_class::<Bar>()?;
    m.add_class::<Currency>()?;
    m.add_class::<Interval>()?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.models", &m)?;

    Ok(())
}
