//! Trait that storage solutions must implement.

use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;
use crate::storage::errors::StorageResult;
use crate::storage::models::bar_series::BarSeries;
use crate::storage::models::storage_summary::StorageSummary;
use async_trait::async_trait;

/// Abstraction over a storage solution.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Initialize all tables in the database.
    fn init(&self) -> StorageResult<()>;

    /// Store multiple series of OHLC data in a single transaction.
    fn write_bars_bulk(&self, series: &[BarSeries]) -> StorageResult<()>;

    /// Get the (min_ts, max_ts) of stored bars for a given symbol/provider/interval.
    /// Returns `None` if no data exists.
    fn get_stored_range(
        &self,
        symbol: &str,
        interval: Interval,
        provider: Provider,
    ) -> StorageResult<Option<(u64, u64)>>;

    /// Return a summary for every (symbol, provider, interval) series in the database.
    fn get_summary(&self) -> StorageResult<Vec<StorageSummary>>;

    /// Delete all bars for a given (symbol, provider, interval) series.
    fn delete_rows(
        &self,
        symbol: &str,
        interval: Option<Interval>,
        provider: Option<Provider>,
    ) -> StorageResult<u64>;
}
