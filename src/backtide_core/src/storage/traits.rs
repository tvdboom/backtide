use crate::backtest::models::{ExperimentConfig, ExperimentResult, RunResult};
use crate::constants::BarKey;
use crate::data::models::{Exchange, Instrument, InstrumentType, Interval, Provider};
use crate::storage::errors::StorageResult;
use crate::storage::models::*;
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
        symbols: Option<&[&str]>,
        intervals: Option<&[Interval]>,
        providers: Option<&[Provider]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredBar>>;

    /// Return stored dividends, optionally filtered by symbol/provider with a limit.
    fn query_dividends(
        &self,
        symbols: Option<&[&str]>,
        providers: Option<&[Provider]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredDividend>>;

    /// Return stored instrument metadata, optionally filtered by type/provider/exchanges with a limit.
    fn query_instruments(
        &self,
        instrument_types: Option<&[InstrumentType]>,
        providers: Option<&[Provider]>,
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

    /// Persist one experiment run to the database (all related tables).
    fn write_experiment(
        &self,
        config: &ExperimentConfig,
        result: &ExperimentResult,
    ) -> StorageResult<()>;

    /// Query experiments, optionally filtered by `experiment_id` (one or
    /// many ids) and/or `search` (matches name or any tag,
    /// case-insensitive substring). Filters combine with AND semantics.
    fn query_experiments(
        &self,
        experiment_id: Option<&[String]>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredExperiment>>;

    /// Load every persisted [`RunResult`] for a given experiment.
    fn query_strategy_runs(&self, experiment_id: &str) -> StorageResult<Vec<RunResult>>;

    /// Delete a single experiment and all its child rows.
    fn delete_experiment(&self, experiment_id: &str) -> StorageResult<u64>;
}
