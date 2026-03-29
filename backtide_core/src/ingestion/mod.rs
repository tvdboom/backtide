use crate::ingestion::provider::Provider;
use crate::ingestion::interface::{get_assets, list_assets};
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

mod ingester;
pub mod provider;
pub mod errors;
mod interface;

/// Register all ingestion types to `backtide.core.ingestion`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.ingestion")?;

    m.add_class::<Provider>()?;

    m.add_function(wrap_pyfunction!(get_assets, &m)?)?;
    m.add_function(wrap_pyfunction!(list_assets, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.ingestion", &m)?;

    Ok(())
}
