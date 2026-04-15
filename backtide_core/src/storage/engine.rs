//! Implementation of storage related methods for [`Engine`].

use crate::constants::BarKey;
use crate::data::models::exchange::Exchange;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_type::InstrumentType;
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
    /// Returns stored bars, optionally filtered.
    pub fn query_bars(
        &self,
        symbol: Option<&str>,
        interval: Option<Interval>,
        provider: Option<Provider>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredBar>> {
        self.db.query_bars(symbol, interval, provider, limit)
    }

    /// Returns all stored (symbol, interval, provider) -> (min_ts, max_ts) in one query.
    pub fn query_bar_ranges(&self) -> StorageResult<HashMap<BarKey, (u64, u64)>> {
        self.db.query_bar_ranges()
    }

    /// Returns a pre-aggregated summary of stored bars.
    pub fn query_bars_summary(&self) -> StorageResult<Vec<BarSummary>> {
        self.db.query_bars_summary()
    }

    /// Returns stored dividends, optionally filtered.
    pub fn query_dividends(
        &self,
        symbol: Option<&str>,
        provider: Option<Provider>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredDividend>> {
        self.db.query_dividends(symbol, provider, limit)
    }

    /// Returns stored instrument metadata, optionally filtered.
    pub fn query_instruments(
        &self,
        instrument_type: Option<InstrumentType>,
        provider: Option<Provider>,
        exchanges: Option<&[Exchange]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Instrument>> {
        self.db.query_instruments(instrument_type, provider, exchanges, limit)
    }

    /// Writes many bar series to storage in a single transaction.
    pub fn write_bars_bulk(&self, series: &[BarSeries]) -> StorageResult<()> {
        self.db.write_bars_bulk(series)
    }

    /// Writes many dividend series to storage in a single transaction.
    pub fn write_dividends_bulk(&self, series: &[DividendSeries]) -> StorageResult<()> {
        self.db.write_dividends_bulk(series)
    }

    /// Upsert instrument metadata rows.
    pub fn write_instruments(&self, instruments: &[Instrument]) -> StorageResult<()> {
        self.db.write_instruments(instruments)
    }

    /// Deletes bars (and orphaned dividends/instruments) for one or more series.
    pub fn delete_symbols(
        &self,
        series: &[(String, Option<Interval>, Option<Provider>)],
    ) -> StorageResult<u64> {
        self.db.delete_symbols(series)
    }
}
