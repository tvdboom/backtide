//! Implementation of storage related methods for [`Engine`].

use crate::data::models::asset_type::AssetType;
use crate::data::models::bar::Bar;
use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;
use crate::engine::Engine;
use crate::storage::errors::StorageResult;
use crate::storage::models::storage_summary::StorageSummary;

impl Engine {
    /// Writes the given bars to storage for the specified group.
    pub fn write_bars(
        &self,
        symbol: &str,
        asset_type: AssetType,
        interval: Interval,
        provider: Provider,
        bars: &[Bar],
    ) -> StorageResult<()> {
        self.db.write_bars(symbol, asset_type, interval, provider, bars)
    }

    /// Returns the earliest and latest stored timestamps for the given group.
    pub fn get_stored_range(
        &self,
        symbol: &str,
        interval: Interval,
        provider: Provider,
    ) -> StorageResult<Option<(u64, u64)>> {
        self.db.get_stored_range(symbol, interval, provider)
    }

    /// Returns a summary of all data currently held in storage.
    pub fn get_summary(&self) -> StorageResult<Vec<StorageSummary>> {
        self.db.get_summary()
    }

    /// Deletes all stored rows matching the given symbol, provider, and interval.
    pub fn delete_rows(
        &self,
        symbol: &str,
        interval: Option<Interval>,
        provider: Option<Provider>,
    ) -> StorageResult<u64> {
        self.db.delete_rows(symbol, interval, provider)
    }
}
