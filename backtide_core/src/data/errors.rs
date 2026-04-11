use crate::constants::Symbol;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::utils::http::HttpError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

/// Errors that the data module raises.
#[derive(Debug, Error)]
pub enum DataError {
    /// Failed to authenticate (e.g. provider crumb fetch failed).
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// An HTTP client related error.
    #[error("HTTP error: {0}")]
    Http(#[from] HttpError),

    /// Direct conversion and all triangulation legs are degenerate for this pair.
    #[error("No conversion path from '{from}' to '{to}'")]
    NoConversionPath {
        from: String,
        to: String,
    },

    /// The requested value does not exist or is not served.
    #[error("Symbol not found: {0}")]
    SymbolNotFound(Symbol),

    /// The response had an unexpected structure (e.g., missing fields).
    #[error("Unexpected response: {0}")]
    UnexpectedResponse(String),

    /// The instrument type is not supported by the provider.
    #[error("Unsupported instrument type: {0}")]
    UnsupportedInstrumentType(InstrumentType),

    /// The interval is not supported by the provider.
    #[error("Unsupported interval: {0}")]
    UnsupportedInterval(Interval),
}

pub type DataResult<T> = Result<T, DataError>;

impl From<DataError> for PyErr {
    fn from(e: DataError) -> PyErr {
        PyRuntimeError::new_err(e.to_string())
    }
}
