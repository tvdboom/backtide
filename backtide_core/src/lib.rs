mod config;
mod constants;
mod ingestion;
mod models;
mod utils;

use pyo3::prelude::*;

#[pymodule]
fn core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    ingestion::register(m)?;
    models::register(m)?;
    config::register(m)?;
    Ok(())
}
