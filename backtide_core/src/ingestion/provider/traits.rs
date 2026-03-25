//! Trait that all market-data providers must implement.

use async_trait::async_trait;
use thiserror::Error;
use crate::models::asset::{Asset, AssetType};

/// Errors that a [`Provider`] implementation may return.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// The requested symbol does not exist or is not served by this provider.
    #[error("symbol not found: {0}")]
    NotFound(String),

    /// The provider does not support the requested [`AssetType`].
    #[error("unsupported asset type: {0:?}")]
    UnsupportedAssetType(AssetType),

    /// The provider's rate limit was hit. Callers should wait at least
    /// `retry_after_secs` before retrying.
    #[error("rate limited – retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    /// An underlying HTTP transport error from `reqwest`.
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Any other provider-specific failure not covered by the variants above.
    #[error("{0}")]
    Other(String),
}

pub type ProviderResult<T> = Result<T, ProviderError>;

/// Abstraction over a market-data source.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Short identifier used for logging and config keys, e.g. `"yahoo"`, `"kraken"`.
    fn id(&self) -> &str;

    /// Returns an overview of the most important assets of `asset_type` that
    /// the provider serves. May be expensive – callers should cache the result.
    async fn list_assets(&self, asset_type: AssetType) -> ProviderResult<Vec<Asset>>;

    /// Fetch metadata for a single symbol without downloading any bars.
    async fn get_asset(&self, symbol: &str) -> ProviderResult<Asset>;

    /// Download OHLCV bars for `symbol` at `interval` from `start` to `end`.
    /// Returns an empty `Vec` when the symbol had no trading activity that period
    /// (e.g. before listing date), which is distinct from a `ProviderError`.
    async fn download_batch(
        &self,
        symbol: &str,
        interval: Interval,
        start: i64,
        end: i64,
    ) -> ProviderResult<Vec<Bar>>;
}
