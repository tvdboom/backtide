//! Custom errors raised during data ingestion.

use duckdb::Error;
use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

/// Errors that the storage module raises.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Failed to initialize the database.
    #[error("duckdb error: {0}")]
    DuckDB(#[from] Error),
}

pub type StorageResult<T> = Result<T, StorageError>;

impl From<StorageError> for PyErr {
    fn from(e: StorageError) -> PyErr {
        PyRuntimeError::new_err(e.to_string())
    }
}
