mod config;
mod constants;
mod ingestion;
mod models;
mod utils;

use pyo3::prelude::*;

#[pymodule]
fn core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    config::register(m)?;
    Ok(())
}
