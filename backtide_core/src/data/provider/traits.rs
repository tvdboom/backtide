//! Trait that all market-data providers must implement.

use crate::data::errors::DataResult;
use crate::data::models::asset::{Asset, AssetType, Symbol};
use crate::data::models::bar::Interval;
use async_trait::async_trait;

/// Abstraction over a market-data source.
#[async_trait]
pub trait DataProvider: Send + Sync {
    /// Fetch metadata for a single symbol without downloading any bars.
    async fn get_asset(&self, symbol: &Symbol, asset_type: AssetType) -> DataResult<Asset>;

    /// Returns an overview of the most important assets of `asset_type`.
    /// May be expensive – callers should cache the result.
    async fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>>;

    /// The intervals supported by this provider.
    fn list_intervals(&self) -> Vec<Interval>;

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
