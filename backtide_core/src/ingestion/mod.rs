use crate::ingestion::ingester::list_assets;
use crate::ingestion::provider::Provider;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

mod ingester;
pub mod provider;

/// Register all ingestion types to `backtide.core.ingestion`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.ingestion")?;

    m.add_class::<Provider>()?;

    m.add_function(wrap_pyfunction!(list_assets, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.ingestion", &m)?;

    Ok(())
}
