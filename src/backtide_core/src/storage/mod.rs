use crate::storage::interface::*;
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

    m.add_function(wrap_pyfunction!(query_bars, &m)?)?;
    m.add_function(wrap_pyfunction!(query_bars_summary, &m)?)?;
    m.add_function(wrap_pyfunction!(query_dividends, &m)?)?;
    m.add_function(wrap_pyfunction!(query_experiments, &m)?)?;
    m.add_function(wrap_pyfunction!(query_experiment_strategies, &m)?)?;
    m.add_function(wrap_pyfunction!(query_instruments, &m)?)?;
    m.add_function(wrap_pyfunction!(delete_symbols, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.storage", &m)?;

    Ok(())
}
