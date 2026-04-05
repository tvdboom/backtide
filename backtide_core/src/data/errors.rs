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
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// An HTTP client related error.
    #[error("HTTP error: {0}")]
    Http(#[from] HttpError),

    /// A triangulation leg doesn't cover the full history of its primary asset.
    #[error(
        "Required symbol '{leg_symbol}' (earliest: {leg_earliest:?}) starts after \
         asset '{asset_symbol}' (earliest: {asset_earliest:?})"
    )]
    InsufficientLegHistory {
        asset_symbol: String,
        asset_earliest: Option<u64>,
        leg_symbol: String,
        leg_earliest: Option<u64>,
    },

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
    #[error("Unexpected response structure: {0}")]
    UnexpectedResponse(String),

    /// The asset type is not supported by the provider.
    #[error("Unsupported asset type: {0}")]
    UnsupportedAssetType(AssetType),
}

pub type DataResult<T> = Result<T, DataError>;

impl From<DataError> for PyErr {
    fn from(e: DataError) -> PyErr {
        PyRuntimeError::new_err(e.to_string())
    }
}
