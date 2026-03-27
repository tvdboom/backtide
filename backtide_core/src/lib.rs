mod config;
mod constants;
mod ingestion;
mod models;
mod utils;

use crate::utils::utils::init_tracing;
use pyo3::prelude::*;

#[pymodule]
fn core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(init_tracing, m)?)?;

    ingestion::register(m)?;
    models::register(m)?;
    config::register(m)?;
    Ok(())
}
