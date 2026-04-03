//! Custom errors raised during data ingestion.

use crate::constants::Symbol;
use crate::data::models::asset_type::AssetType;
use crate::utils::http::HttpError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

/// Errors that the data module raises.
#[derive(Debug, Error)]
pub enum DataError {
    /// Failed to authenticate (e.g. provider crumb fetch failed).
    #[error("authentication failed: {0}")]
    Auth(String),

    /// An HTTP client related error.
    #[error("HTTP error: {0}")]
    Http(#[from] HttpError),

    /// The response body could not be parsed as valid JSON.
    #[error("failed to parse JSON response: {0}")]
    Json(#[from] serde_json::Error),

    /// The requested value does not exist or is not served.
    #[error("Symbol not found: {0}")]
    SymbolNotFound(Symbol),

    /// The response had an unexpected structure (e.g., missing fields).
    #[error("unexpected response structure: {0}")]
    UnexpectedResponse(String),

    /// The asset type is not supported by the provider.
    #[error("unsupported asset type: {0}")]
    UnsupportedAssetType(AssetType),
}

pub type DataResult<T> = Result<T, DataError>;

impl From<DataError> for PyErr {
    fn from(e: DataError) -> PyErr {
        PyRuntimeError::new_err(e.to_string())
    }
}
