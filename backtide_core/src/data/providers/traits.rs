//! Trait that market-data providers must implement.

use crate::constants::Symbol;
use crate::data::errors::DataResult;
use crate::data::models::bar::Bar;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use async_trait::async_trait;

/// Abstraction over a market-data source.
#[async_trait]
pub trait DataProvider: Send + Sync {
    /// Get a single instrument given its symbol.
    async fn get_instrument(
        &self,
        symbol: &Symbol,
        instrument_type: InstrumentType,
    ) -> DataResult<Instrument>;

    /// Returns the usable download range for an instrument at a given interval.
    async fn get_download_range(
        &self,
        instrument: Instrument,
        interval: Interval,
    ) -> DataResult<(u64, u64)>;

    /// List the most important instruments for a given instrument type.
    async fn list_instruments(
        &self,
        instrument_type: InstrumentType,
        limit: usize,
    ) -> DataResult<Vec<Instrument>>;

    /// Download OHLCV bars for `symbol` at `interval` from `start` to `end`.
    /// Returns an empty `Vec` when the symbol had no trading activity that period
    /// (e.g., before listing date), which is distinct from a [`DataResult`].
    async fn download_batch(
        &self,
        symbol: &str,
        instrument_type: InstrumentType,
        interval: Interval,
        start: u64,
        end: u64,
    ) -> DataResult<Vec<Bar>>;
}
