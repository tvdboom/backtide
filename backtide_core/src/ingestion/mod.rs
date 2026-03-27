use crate::ingestion::provider::Provider;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub mod provider;
mod ingester;

/// Register all ingestion types to `backtide.core.ingestion`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.ingestion")?;

    m.add_class::<Provider>()?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.ingestion", &m)?;

    Ok(())
}
