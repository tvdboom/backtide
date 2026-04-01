//! Custom errors raised during data ingestion.

use crate::config::ConfigError;
use crate::constants::Symbol;
use crate::utils::http::HttpError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

/// Errors that the [`DataIngester`] implementation might return.
#[derive(Debug, Error)]
pub enum DataError {
    /// Failed to authenticate (e.g. provider crumb fetch failed).
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// An error when trying to retrieve the config file.
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),

    /// An HTTP client related error.
    #[error("HTTP error: {0}")]
    Http(#[from] HttpError),

    /// A filesystem or I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The response body could not be parsed as valid JSON.
    #[error("Failed to parse JSON response: {0}")]
    Json(#[from] serde_json::Error),

    /// The requested value does not exist or is not served.
    #[error("symbol not found: {0}")]
    SymbolNotFound(Symbol),

    /// Any other failure not covered by the other variants.
    #[error("{0}")]
    Other(String),

    /// The rate limit was hit.
    /// Callers should wait `retry_after_secs` before retrying.
    #[error("rate limited – retry after {retry_after_secs}s")]
    RateLimited {
        retry_after_secs: u64,
    },

    /// The response had an unexpected structure (e.g., missing fields).
    #[error("Unexpected response structure: {0}")]
    UnexpectedResponse(String),
}

pub type DataResult<T> = Result<T, DataError>;

impl From<DataError> for PyErr {
    fn from(e: DataError) -> PyErr {
        PyRuntimeError::new_err(e.to_string())
    }
}
