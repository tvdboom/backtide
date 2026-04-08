use crate::storage::interface::{delete_rows, get_summary};
use crate::storage::models::storage_summary::StorageSummary;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub mod duckdb;
mod engine;
pub mod errors;
mod interface;
pub mod models;
pub mod traits;

/// Register the Python interface for `backtide.core.storage`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.storage")?;

    m.add_class::<StorageSummary>()?;

    m.add_function(wrap_pyfunction!(get_summary, &m)?)?;
    m.add_function(wrap_pyfunction!(delete_rows, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.storage", &m)?;

    Ok(())
}
