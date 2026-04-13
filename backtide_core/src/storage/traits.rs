//! Trait that storage solutions must implement.

use crate::constants::BarKey;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::storage::errors::StorageResult;
use crate::storage::models::bar_series::BarSeries;
use crate::storage::models::dividend_series::DividendSeries;
use crate::storage::models::stored_bar::StoredBar;
use crate::storage::models::stored_dividend::StoredDividend;
use async_trait::async_trait;
use std::collections::HashMap;

/// Abstraction over a storage solution.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Initialize all tables in the database.
    fn init(&self) -> StorageResult<()>;

    /// Get the (min_ts, max_ts) of stored bars.
    fn get_bar_ranges(&self) -> StorageResult<HashMap<BarKey, (u64, u64)>>;

    /// Return all stored bars.
    fn get_all_bars(&self) -> StorageResult<Vec<StoredBar>>;

    /// Return all stored dividends.
    fn get_all_dividends(&self) -> StorageResult<Vec<StoredDividend>>;

    /// Store multiple series of OHLC data in a single transaction.
    fn write_bars_bulk(&self, series: &[BarSeries]) -> StorageResult<()>;

    /// Store multiple series of dividend events in a single transaction.
    fn write_dividends_bulk(&self, series: &[DividendSeries]) -> StorageResult<()>;

    /// Delete all bars for a given symbol, filtered by interval and provider.
    /// Orphaned dividends are removed when no bars remain for its symbol.
    fn delete_symbols(
        &self,
        symbol: &str,
        interval: Option<Interval>,
        provider: Option<Provider>,
    ) -> StorageResult<u64>;
}
