//! Implementation of storage related methods for [`Engine`].

use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::engine::Engine;
use crate::storage::errors::StorageResult;
use crate::storage::models::bar_series::BarSeries;
use crate::storage::models::dividend_series::DividendSeries;
use crate::storage::models::stored_bar::StoredBar;
use crate::storage::models::stored_dividend::StoredDividend;

impl Engine {
    /// Writes many bar series to storage in a single transaction.
    pub fn write_bars_bulk(&self, series: &[BarSeries]) -> StorageResult<()> {
        self.db.write_bars_bulk(series)
    }

    /// Writes many dividend series to storage in a single transaction.
    pub fn write_dividends_bulk(&self, series: &[DividendSeries]) -> StorageResult<()> {
        self.db.write_dividends_bulk(series)
    }

    /// Returns the earliest and latest stored timestamps for the given series.
    pub fn get_stored_range(
        &self,
        symbol: &str,
        interval: Interval,
        provider: Provider,
    ) -> StorageResult<Option<(u64, u64)>> {
        self.db.get_stored_range(symbol, interval, provider)
    }

    /// Returns all stored bars.
    pub fn get_all_bars(&self) -> StorageResult<Vec<StoredBar>> {
        self.db.get_all_bars()
    }

    /// Returns all stored dividends.
    pub fn get_all_dividends(&self) -> StorageResult<Vec<StoredDividend>> {
        self.db.get_all_dividends()
    }

    /// Deletes all stored bars matching the given symbol, provider, and interval.
    /// Orphaned dividends are cleaned up automatically.
    pub fn delete_symbols(
        &self,
        symbol: &str,
        interval: Option<Interval>,
        provider: Option<Provider>,
    ) -> StorageResult<u64> {
        self.db.delete_symbols(symbol, interval, provider)
    }
}
