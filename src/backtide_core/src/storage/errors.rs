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

    /// A filesystem or I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Persisted data could not be decoded into the current domain model.
    #[error("corrupt stored data: {0}")]
    CorruptData(String),

    /// A value could not be serialized for persistence.
    #[error("storage serialization failed: {0}")]
    Serialization(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

impl From<StorageError> for PyErr {
    fn from(e: StorageError) -> PyErr {
        PyRuntimeError::new_err(e.to_string())
    }
}
