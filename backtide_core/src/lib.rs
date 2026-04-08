mod config;
mod constants;
mod data;
mod engine;
mod errors;
mod storage;
mod utils;

use pyo3::prelude::*;

/// Register the Python interface for `backtide.core`.
#[pymodule]
fn core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    config::register(m)?;
    data::register(m)?;
    storage::register(m)?;
    utils::register(m)?;
    Ok(())
}
