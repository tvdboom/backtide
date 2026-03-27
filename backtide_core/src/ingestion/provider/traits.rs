//! Trait that all market-data providers must implement.

use crate::models::asset::{Asset, AssetType};
use crate::models::bar::Interval;
use crate::utils::http::HttpError;
use async_trait::async_trait;
use thiserror::Error;

/// Errors that a [`Provider`] implementation may return.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// The provider failed to authenticate (e.g. crumb fetch failed).
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// An HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] HttpError),

    /// The response body could not be parsed as valid JSON.
    #[error("Failed to parse JSON response: {0}")]
    Json(#[from] serde_json::Error),

    /// The requested symbol does not exist or is not served by this provider.
    #[error("symbol not found: {0}")]
    NotFound(String),

    /// Any other provider-specific failure not covered by the variants above.
    #[error("{0}")]
    Other(String),

    /// The provider's rate limit was hit. Callers should wait at least
    /// `retry_after_secs` before retrying.
    #[error("rate limited – retry after {retry_after_secs}s")]
    RateLimited {
        retry_after_secs: u64,
    },

    /// The response had an unexpected structure (e.g., missing fields).
    #[error("Unexpected response structure: {0}")]
    UnexpectedResponse(String),

    /// The provider does not support the requested [`AssetType`].
    #[error("unsupported asset type: {0:?}")]
    UnsupportedAssetType(AssetType),
}

pub type ProviderResult<T> = Result<T, ProviderError>;

/// Abstraction over a market-data source.
#[async_trait]
pub trait DataProvider: Send + Sync {
    /// The intervals supported by this provider.
    fn intervals(&self) -> Vec<Interval>;

    /// Returns an overview of the most important assets of `asset_type` that
    /// the provider serves. May be expensive – callers should cache the result.
    async fn list_assets(&self, asset_type: AssetType, limit: usize) -> ProviderResult<Vec<Asset>>;

    // /// Fetch metadata for a single symbol without downloading any bars.
    async fn get_asset(&self, symbol: &str) -> ProviderResult<Asset>;

    // /// Download OHLCV bars for `symbol` at `interval` from `start` to `end`.
    // /// Returns an empty `Vec` when the symbol had no trading activity that period
    // /// (e.g. before listing date), which is distinct from a `ProviderError`.
    // async fn download_batch(
    //     &self,
    //     symbol: &str,
    //     interval: Interval,
    //     start: i64,
    //     end: i64,
    // ) -> ProviderResult<Vec<Bar>>;
}
