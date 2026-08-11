//! Metric catalog data models.

use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

/// Metadata describing a built-in performance metric.
///
/// Attributes
/// ----------
/// key : str
///     Stable key stored in experiment results.
///
/// name : str
///     Human-readable display name.
///
/// description : str
///     Short explanation of the metric.
///
/// percentage : bool
///     Whether the value is a fractional percentage.
///
/// higher_is_better : bool
///     Whether larger values rank ahead of smaller values.
#[pyclass(get_all, eq, from_py_object, module = "backtide.metrics")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub key: String,
    pub name: String,
    pub description: String,
    pub percentage: bool,
    pub higher_is_better: bool,
}

impl MetricDefinition {
    pub(crate) fn new(
        key: &str,
        name: &str,
        description: &str,
        percentage: bool,
        higher_is_better: bool,
    ) -> Self {
        Self {
            key: key.to_owned(),
            name: name.to_owned(),
            description: description.to_owned(),
            percentage,
            higher_is_better,
        }
    }
}

#[pymethods]
impl MetricDefinition {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!("MetricDefinition(key={:?}, name={:?})", self.key, self.name)
    }
}
