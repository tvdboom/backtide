//! Trait that all market-data providers must implement.

use crate::constants::Symbol;
use crate::data::errors::DataResult;
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use async_trait::async_trait;

/// Abstraction over a market-data source.
#[async_trait]
pub trait DataProvider: Send + Sync {
    /// Get a single asset given its symbol.
    async fn get_asset(&self, symbol: &Symbol, asset_type: AssetType) -> DataResult<Asset>;

    /// List the most important assets for a given asset type.
    async fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>>;

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
