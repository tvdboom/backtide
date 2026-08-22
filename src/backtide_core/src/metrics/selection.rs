//! Serializable metric names with optional in-memory Python implementations.

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

/// An ordered metric selection that serializes as names while retaining Python objects in memory.
#[derive(Clone, Debug)]
pub struct MetricSelection {
    names: Vec<String>,
    implementations: Arc<HashMap<String, Py<PyAny>>>,
}

impl MetricSelection {
    /// Build a selection containing only serializable metric names.
    pub fn from_names(names: Vec<String>) -> Self {
        Self {
            names,
            implementations: Arc::new(HashMap::new()),
        }
    }

    /// Clone the runtime Python implementations while attached to Python.
    pub fn implementations(&self, py: Python<'_>) -> HashMap<String, Py<PyAny>> {
        self.implementations
            .iter()
            .map(|(name, metric)| (name.clone(), metric.clone_ref(py)))
            .collect()
    }

    /// Return whether an in-memory Python implementation exists for a metric name.
    pub fn has_implementation(&self, name: &str) -> bool {
        self.implementations.contains_key(name)
    }

    fn from_python(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut names = Vec::new();
        let mut implementations = HashMap::new();

        if let Ok(values) = value.cast::<PyList>() {
            for item in values.iter() {
                Self::push_python(item.as_any(), &mut names, &mut implementations)?;
            }
        } else if let Ok(values) = value.cast::<PyTuple>() {
            for item in values.iter() {
                Self::push_python(item.as_any(), &mut names, &mut implementations)?;
            }
        } else {
            Self::push_python(value, &mut names, &mut implementations)?;
        }

        Ok(Self {
            names,
            implementations: Arc::new(implementations),
        })
    }

    fn push_python(
        value: &Bound<'_, PyAny>,
        names: &mut Vec<String>,
        implementations: &mut HashMap<String, Py<PyAny>>,
    ) -> PyResult<()> {
        if let Ok(name) = value.extract::<String>() {
            Self::push_name(names, name);
            return Ok(());
        }
        if let Ok(values) = value.cast::<PyDict>() {
            for (name, metric) in values.iter() {
                let name = name
                    .extract::<String>()
                    .map_err(|_| PyTypeError::new_err("Metric mapping keys must be strings."))?;
                Self::push_name(names, name.clone());
                implementations.insert(name, metric.as_any().clone().unbind());
            }
            return Ok(());
        }

        let name = value.get_type().name()?.to_string();
        Self::push_name(names, name.clone());
        implementations.insert(name, value.clone().unbind());
        Ok(())
    }

    fn push_name(names: &mut Vec<String>, name: String) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
}

impl Default for MetricSelection {
    fn default() -> Self {
        Self::from_names(Vec::new())
    }
}

impl From<Vec<String>> for MetricSelection {
    fn from(names: Vec<String>) -> Self {
        Self::from_names(names)
    }
}

impl Deref for MetricSelection {
    type Target = [String];

    fn deref(&self) -> &Self::Target {
        &self.names
    }
}

impl PartialEq for MetricSelection {
    fn eq(&self, other: &Self) -> bool {
        self.names == other.names
    }
}

impl Serialize for MetricSelection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.names.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MetricSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer).map(Self::from_names)
    }
}

impl<'py> IntoPyObject<'py> for MetricSelection {
    type Target = PyList;
    type Output = Bound<'py, PyList>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let values = PyList::empty(py);
        for name in self.names {
            if let Some(metric) = self.implementations.get(&name) {
                let mapping = PyDict::new(py);
                mapping.set_item(&name, metric.bind(py))?;
                values.append(mapping)?;
            } else {
                values.append(name)?;
            }
        }
        Ok(values)
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for MetricSelection {
    type Error = PyErr;

    fn extract(value: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        Self::from_python(value.as_any())
    }
}
