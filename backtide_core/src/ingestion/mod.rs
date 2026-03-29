use crate::ingestion::interface::{get_assets, list_assets, list_intervals};
use crate::ingestion::provider::Provider;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub mod errors;
mod ingester;
mod interface;
pub mod provider;

/// Register all ingestion types to `backtide.core.ingestion`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.ingestion")?;

    m.add_class::<Provider>()?;

    m.add_function(wrap_pyfunction!(get_assets, &m)?)?;
    m.add_function(wrap_pyfunction!(list_assets, &m)?)?;
    m.add_function(wrap_pyfunction!(list_intervals, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.ingestion", &m)?;

    Ok(())
}
