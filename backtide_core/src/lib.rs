mod config;
mod constants;
mod data;
mod utils;

use crate::utils::tracing::init_tracing;
use pyo3::prelude::*;

/// Register the Python interface for `backtide.core`.
#[pymodule]
fn core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(init_tracing, m)?)?;

    data::register(m)?;
    config::register(m)?;
    Ok(())
}
