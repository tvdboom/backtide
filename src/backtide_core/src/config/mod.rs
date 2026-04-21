use crate::config::interface::*;
use crate::config::models::data_backend::DataBackend;
use crate::config::models::log_level::LogLevel;
use crate::config::models::triangulation_strategy::TriangulationStrategy;
use pyo3::prelude::*;

pub mod errors;
pub mod interface;
pub mod models;
pub mod utils;

/// Register all config types and free functions into `backtide.core.config`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.config")?;

    m.add_class::<DataBackend>()?;
    m.add_class::<LogLevel>()?;
    m.add_class::<TriangulationStrategy>()?;

    m.add_class::<PyConfig>()?;
    m.add_class::<DataConfig>()?;
    m.add_class::<DisplayConfig>()?;
    m.add_class::<GeneralConfig>()?;

    m.add_function(wrap_pyfunction!(get_config, &m)?)?;
    m.add_function(wrap_pyfunction!(load_config, &m)?)?;
    m.add_function(wrap_pyfunction!(set_config, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.config", &m)?;

    Ok(())
}
