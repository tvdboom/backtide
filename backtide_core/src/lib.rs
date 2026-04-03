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
    data::register(m)?;
    config::register(m)?;
    utils::register(m)?;
    Ok(())
}
