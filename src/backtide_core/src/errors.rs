//! Custom errors raised during data ingestion.

use crate::config::errors::ConfigError;
use crate::data::errors::DataError;
use crate::storage::errors::StorageError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

/// Errors that the [`Engine`] implementation might return.
#[derive(Debug, Error)]
pub enum EngineError {
    /// An error when trying to retrieve the config file.
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// An error caused by the data module.
    #[error("{0}")]
    Data(#[from] DataError),

    /// An error while running an experiment.
    #[error("{0}")]
    Experiment(String),

    /// A filesystem or I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An error caused by the storage module.
    #[error("{0}")]
    Storage(#[from] StorageError),
}

pub type EngineResult<T> = Result<T, EngineError>;

impl From<EngineError> for PyErr {
    fn from(e: EngineError) -> PyErr {
        PyRuntimeError::new_err(e.to_string())
    }
}
