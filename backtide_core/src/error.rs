//! Error types for the market data library.

use thiserror::Error;

/// All errors that can occur when fetching market data.
#[derive(Debug, Error)]
pub enum MarketDataError {
    /// An HTTP request failed after all retries were exhausted.
    #[error("HTTP request failed after {retries} retries: {source}")]
    Http {
        retries: u32,
        #[source]
        source: reqwest::Error,
    },

    /// The response body could not be parsed as valid JSON.
    #[error("Failed to parse JSON response: {0}")]
    Json(#[from] serde_json::Error),

    /// The response had an unexpected structure (e.g. missing fields).
    #[error("Unexpected response structure: {0}")]
    UnexpectedResponse(String),

    /// The provider failed to authenticate (e.g. crumb fetch failed).
    #[error("Authentication failed: {0}")]
    Auth(String),
}

impl From<MarketDataError> for pyo3::PyErr {
    /// Convert a [`MarketDataError`] into a Python `RuntimeError` so it can be
    /// raised and displayed in the frontend.
    fn from(err: MarketDataError) -> pyo3::PyErr {
        pyo3::exceptions::PyRuntimeError::new_err(err.to_string())
    }
}
