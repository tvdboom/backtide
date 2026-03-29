//! Trait that all market-data providers must implement.

use crate::models::asset::{Asset, AssetType};
use crate::models::bar::Interval;
use async_trait::async_trait;

/// Abstraction over a market-data source.
#[async_trait]
pub trait DataProvider: Send + Sync {
    /// The intervals supported by this provider.
    fn intervals(&self) -> Vec<Interval>;

    /// Returns an overview of the most important assets of `asset_type` that
    /// the provider serves. May be expensive – callers should cache the result.
    async fn list_assets(&self, asset_type: AssetType, limit: usize) -> IngestionResult<Vec<Asset>>;

    // /// Fetch metadata for a single symbol without downloading any bars.
    async fn get_asset(&self, asset_type: AssetType, symbol: &str) -> IngestionResult<Asset>;

    // /// Download OHLCV bars for `symbol` at `interval` from `start` to `end`.
    // /// Returns an empty `Vec` when the symbol had no trading activity that period
    // /// (e.g. before listing date), which is distinct from a `IngestionError`.
    // async fn download_batch(
    //     &self,
    //     symbol: &str,
    //     interval: Interval,
    //     start: i64,
    //     end: i64,
    // ) -> IngestionResult<Vec<Bar>>;
}
