use crate::utils::interface::{clear_cache, init_logging};
use pyo3::prelude::*;
use pyo3::{wrap_pyfunction, Bound, PyResult};

pub mod dataframe;
pub mod experiment_log;
pub mod http;
pub mod interface;
pub mod progress;

/// Register the Python interface for `backtide.core.utils`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.utils")?;

    m.add_function(wrap_pyfunction!(clear_cache, &m)?)?;
    m.add_function(wrap_pyfunction!(init_logging, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.utils", &m)?;

    Ok(())
}
