//! Python interface for metric metadata.

use crate::metrics::engine::builtin_metric_definitions;
use crate::metrics::models::MetricDefinition;
use pyo3::prelude::*;

/// Return metadata for all built-in Rust metrics.
///
/// Returns
/// -------
/// list[[MetricDefinition]]
///     Stable definitions used by experiment configuration and result displays.
#[pyfunction]
pub fn list_builtin_metrics() -> Vec<MetricDefinition> {
    builtin_metric_definitions()
}
