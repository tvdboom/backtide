use crate::constants::BarKey;
use crate::data::models::exchange::Exchange;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::storage::errors::StorageResult;
use crate::storage::models::bar_series::BarSeries;
use crate::storage::models::bar_summary::BarSummary;
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
    fn query_bar_ranges(&self) -> StorageResult<HashMap<BarKey, (u64, u64)>>;

    /// Return a pre-aggregated summary of stored bars.
    fn query_bars_summary(&self) -> StorageResult<Vec<BarSummary>>;

    /// Return stored bars, optionally filtered by symbol/interval/provider with a limit.
    fn query_bars(
        &self,
        symbol: Option<&str>,
        interval: Option<Interval>,
        provider: Option<Provider>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredBar>>;

    /// Return stored dividends, optionally filtered by symbol/provider with a limit.
    fn query_dividends(
        &self,
        symbol: Option<&str>,
        provider: Option<Provider>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredDividend>>;

    /// Return stored instrument metadata, optionally filtered by type/provider/exchanges with a limit.
    fn query_instruments(
        &self,
        instrument_type: Option<InstrumentType>,
        provider: Option<Provider>,
        exchanges: Option<&[Exchange]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Instrument>>;

    /// Upsert instrument metadata rows.
    fn write_instruments(&self, instruments: &[Instrument]) -> StorageResult<()>;

    /// Store multiple series of OHLC data in a single transaction.
    fn write_bars_bulk(&self, series: &[BarSeries]) -> StorageResult<()>;

    /// Store multiple series of dividend events in a single transaction.
    fn write_dividends_bulk(&self, series: &[DividendSeries]) -> StorageResult<()>;

    /// Delete bars (and orphaned dividends/instruments) for one or more series.
    fn delete_symbols(
        &self,
        series: &[(String, Option<Interval>, Option<Provider>)],
    ) -> StorageResult<u64>;
}
