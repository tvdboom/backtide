//! Implementation of storage related methods for [`Engine`].

use crate::constants::BarKey;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::engine::Engine;
use crate::storage::errors::StorageResult;
use crate::storage::models::bar_series::BarSeries;
use crate::storage::models::bar_summary::BarSummary;
use crate::storage::models::dividend_series::DividendSeries;
use crate::storage::models::stored_bar::StoredBar;
use crate::storage::models::stored_dividend::StoredDividend;
use std::collections::HashMap;

impl Engine {
    /// Writes many bar series to storage in a single transaction.
    pub fn write_bars_bulk(&self, series: &[BarSeries]) -> StorageResult<()> {
        self.db.write_bars_bulk(series)
    }

    /// Writes many dividend series to storage in a single transaction.
    pub fn write_dividends_bulk(&self, series: &[DividendSeries]) -> StorageResult<()> {
        self.db.write_dividends_bulk(series)
    }

    /// Returns all stored (symbol, interval, provider) → (min_ts, max_ts) in one query.
    pub fn get_bar_ranges(&self) -> StorageResult<HashMap<BarKey, (u64, u64)>> {
        self.db.get_bar_ranges()
    }

    /// Returns a pre-aggregated summary of stored bars.
    pub fn get_bars_summary(&self) -> StorageResult<Vec<BarSummary>> {
        self.db.get_bars_summary()
    }

    /// Returns all stored bars.
    pub fn get_all_bars(&self) -> StorageResult<Vec<StoredBar>> {
        self.db.get_all_bars()
    }

    /// Returns all stored dividends.
    pub fn get_all_dividends(&self) -> StorageResult<Vec<StoredDividend>> {
        self.db.get_all_dividends()
    }

    /// Deletes bars (and orphaned dividends) for one or more series.
    pub fn delete_symbols(
        &self,
        series: &[(String, Option<Interval>, Option<Provider>)],
    ) -> StorageResult<u64> {
        self.db.delete_symbols(series)
    }
}
