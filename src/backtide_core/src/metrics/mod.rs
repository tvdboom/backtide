//! Performance metrics and custom-metric integration.

use crate::metrics::interface::list_builtin_metrics;
use crate::metrics::models::MetricDefinition;
use pyo3::prelude::*;

pub mod engine;
pub mod interface;
pub mod models;
pub mod utils;

/// Register the Python interface for `backtide.core.metrics`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(parent.py(), "backtide.metrics")?;
    module.add_class::<MetricDefinition>()?;
    module.add_function(wrap_pyfunction!(list_builtin_metrics, &module)?)?;
    parent.add_submodule(&module)?;
    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.metrics", &module)?;
    Ok(())
}
