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
        symbols: Option<&[&str]>,
        intervals: Option<&[Interval]>,
        providers: Option<&[Provider]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredBar>> {
        self.db.query_bars(symbols, intervals, providers, limit)
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
        symbols: Option<&[&str]>,
        providers: Option<&[Provider]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<StoredDividend>> {
        self.db.query_dividends(symbols, providers, limit)
    }

    /// Returns stored instrument metadata, optionally filtered.
    pub fn query_instruments(
        &self,
        instrument_types: Option<&[InstrumentType]>,
        providers: Option<&[Provider]>,
        exchanges: Option<&[Exchange]>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Instrument>> {
        self.db.query_instruments(instrument_types, providers, exchanges, limit)
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

    /// Deletes a single experiment and all its child rows.
    ///
    /// Also best-effort removes the persisted source config file at
    /// `<storage>/experiments/<experiment_id>.toml`, if one exists.
    pub fn delete_experiment(&self, experiment_id: &str) -> StorageResult<u64> {
        let n = self.db.delete_experiment(experiment_id)?;
        let path =
            self.config.data.storage_path.join("experiments").join(format!("{experiment_id}.toml"));
        let _ = std::fs::remove_file(path);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::interface::Config;
    use crate::data::models::bar::Bar;
    use crate::data::models::dividend::Dividend;
    use crate::data::providers::traits::DataProvider;
    use crate::engine::{Engine, EngineCache};
    use crate::storage::duckdb::DuckDb;
    use crate::storage::models::bar_series::BarSeries;
    use crate::storage::models::dividend_series::DividendSeries;
    use crate::storage::traits::Storage;
    use std::sync::Arc;
    use strum::IntoEnumIterator;
    use tempfile::TempDir;
    use tokio::runtime::Runtime;

    /// Build a minimal Engine backed by a fresh DuckDb in a temporary directory.
    ///
    /// Providers are unused by storage tests, but every InstrumentType must
    /// be present in the providers map for the engine to be valid. We point
    /// them at any registered provider — the tests below never call out.
    fn test_engine() -> (Engine, TempDir) {
        let config = Box::leak(Box::new(Config::default()));
        let rt = Runtime::new().unwrap();

        let tmp = TempDir::new().unwrap();
        let db = DuckDb::new(&tmp.path().join("test.db")).unwrap();
        db.init().unwrap();

        // Minimal placeholder providers map. Storage tests don't invoke them.
        let providers: HashMap<InstrumentType, Arc<dyn DataProvider>> = InstrumentType::iter()
            .map(|it| {
                let p: Arc<dyn DataProvider> = Arc::new(
                    rt.block_on(crate::data::providers::yahoo::YahooFinance::new()).unwrap(),
                );
                (it, p)
            })
            .collect();

        (
            Engine {
                config,
                rt,
                providers,
                db: Box::new(db),
                cache: EngineCache::new(),
            },
            tmp,
        )
    }

    fn sample_bar(open_ts: u64) -> Bar {
        Bar {
            open_ts,
            close_ts: open_ts + 86_399,
            open_ts_exchange: open_ts,
            open: 100.0,
            high: 101.0,
            low: 99.5,
            close: 100.5,
            adj_close: 100.5,
            volume: 1_000.0,
            n_trades: Some(10),
        }
    }

    fn sample_instrument(symbol: &str) -> Instrument {
        Instrument {
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            base: None,
            quote: "USD".to_owned(),
            instrument_type: InstrumentType::Stocks,
            exchange: "XNAS".to_owned(),
            provider: Provider::Yahoo,
        }
    }

    // ── query_bars ──────────────────────────────────────────────────────

    #[test]
    fn query_bars_empty_when_database_fresh() {
        let (engine, _tmp) = test_engine();
        let bars = engine.query_bars(None, None, None, None).unwrap();
        assert!(bars.is_empty());
    }

    #[test]
    fn write_bars_bulk_then_query_round_trips() {
        let (engine, _tmp) = test_engine();
        let series = vec![BarSeries {
            symbol: "AAPL".into(),
            interval: Interval::OneDay,
            provider: Provider::Yahoo,
            bars: vec![sample_bar(1_700_000_000), sample_bar(1_700_086_400)],
        }];

        engine.write_bars_bulk(&series).unwrap();

        let stored = engine.query_bars(None, None, None, None).unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].symbol, "AAPL");
        assert_eq!(stored[0].interval, "1d");
    }

    #[test]
    fn query_bars_filters_by_symbol() {
        let (engine, _tmp) = test_engine();
        engine
            .write_bars_bulk(&[
                BarSeries {
                    symbol: "AAPL".into(),
                    interval: Interval::OneDay,
                    provider: Provider::Yahoo,
                    bars: vec![sample_bar(1_700_000_000)],
                },
                BarSeries {
                    symbol: "MSFT".into(),
                    interval: Interval::OneDay,
                    provider: Provider::Yahoo,
                    bars: vec![sample_bar(1_700_000_000)],
                },
            ])
            .unwrap();

        let only_aapl = engine.query_bars(Some(&["AAPL"]), None, None, None).unwrap();
        assert_eq!(only_aapl.len(), 1);
        assert_eq!(only_aapl[0].symbol, "AAPL");
    }

    #[test]
    fn query_bars_respects_limit() {
        let (engine, _tmp) = test_engine();
        engine
            .write_bars_bulk(&[BarSeries {
                symbol: "AAPL".into(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars: (0..5).map(|i| sample_bar(1_700_000_000 + i * 86_400)).collect(),
            }])
            .unwrap();

        let limited = engine.query_bars(None, None, None, Some(2)).unwrap();
        assert_eq!(limited.len(), 2);
    }

    // ── query_bar_ranges ────────────────────────────────────────────────

    #[test]
    fn query_bar_ranges_returns_min_and_max_ts() {
        let (engine, _tmp) = test_engine();
        engine
            .write_bars_bulk(&[BarSeries {
                symbol: "AAPL".into(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(1_700_000_000), sample_bar(1_700_172_800)],
            }])
            .unwrap();

        let ranges = engine.query_bar_ranges().unwrap();
        let key = ("AAPL".to_owned(), "1d".to_owned(), "yahoo".to_owned());
        let (min_ts, max_ts) = ranges.get(&key).copied().unwrap();
        assert_eq!(min_ts, 1_700_000_000);
        assert_eq!(max_ts, 1_700_172_800);
    }

    // ── query_bars_summary ──────────────────────────────────────────────

    #[test]
    fn query_bars_summary_returns_one_row_per_series() {
        let (engine, _tmp) = test_engine();
        engine
            .write_bars_bulk(&[BarSeries {
                symbol: "AAPL".into(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(1_700_000_000), sample_bar(1_700_086_400)],
            }])
            .unwrap();

        let summary = engine.query_bars_summary().unwrap();
        assert_eq!(summary.len(), 1);
    }

    // ── dividends ───────────────────────────────────────────────────────

    #[test]
    fn write_and_query_dividends_round_trip() {
        let (engine, _tmp) = test_engine();
        engine
            .write_dividends_bulk(&[DividendSeries {
                symbol: "AAPL".into(),
                provider: Provider::Yahoo,
                dividends: vec![Dividend {
                    ex_date: 1_700_000_000,
                    amount: 0.24,
                }],
            }])
            .unwrap();

        let divs = engine.query_dividends(None, None, None).unwrap();
        assert_eq!(divs.len(), 1);
        assert!((divs[0].dividend.amount - 0.24).abs() < 1e-9);
    }

    #[test]
    fn query_dividends_filters_by_symbol_and_limit() {
        let (engine, _tmp) = test_engine();
        engine
            .write_dividends_bulk(&[
                DividendSeries {
                    symbol: "AAPL".into(),
                    provider: Provider::Yahoo,
                    dividends: vec![Dividend {
                        ex_date: 1_700_000_000,
                        amount: 0.24,
                    }],
                },
                DividendSeries {
                    symbol: "MSFT".into(),
                    provider: Provider::Yahoo,
                    dividends: vec![Dividend {
                        ex_date: 1_700_000_000,
                        amount: 0.75,
                    }],
                },
            ])
            .unwrap();

        let only_aapl = engine.query_dividends(Some(&["AAPL"]), None, None).unwrap();
        assert_eq!(only_aapl.len(), 1);

        let limited = engine.query_dividends(None, None, Some(1)).unwrap();
        assert_eq!(limited.len(), 1);
    }

    // ── instruments ─────────────────────────────────────────────────────

    #[test]
    fn write_and_query_instruments_round_trip() {
        let (engine, _tmp) = test_engine();
        engine.write_instruments(&[sample_instrument("AAPL"), sample_instrument("MSFT")]).unwrap();

        let stored = engine.query_instruments(None, None, None, None).unwrap();
        assert_eq!(stored.len(), 2);
    }

    #[test]
    fn query_instruments_filters_by_type() {
        let (engine, _tmp) = test_engine();
        engine.write_instruments(&[sample_instrument("AAPL")]).unwrap();

        let stocks =
            engine.query_instruments(Some(&[InstrumentType::Stocks]), None, None, None).unwrap();
        assert_eq!(stocks.len(), 1);

        let crypto =
            engine.query_instruments(Some(&[InstrumentType::Crypto]), None, None, None).unwrap();
        assert!(crypto.is_empty());
    }

    // ── delete_symbols ──────────────────────────────────────────────────

    #[test]
    fn delete_symbols_removes_matching_rows() {
        let (engine, _tmp) = test_engine();
        engine.write_instruments(&[sample_instrument("AAPL")]).unwrap();
        engine
            .write_bars_bulk(&[BarSeries {
                symbol: "AAPL".into(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars: vec![sample_bar(1_700_000_000)],
            }])
            .unwrap();

        let deleted = engine
            .delete_symbols(&[("AAPL".to_owned(), Some(Interval::OneDay), Some(Provider::Yahoo))])
            .unwrap();
        assert!(deleted >= 1);

        let bars = engine.query_bars(Some(&["AAPL"]), None, None, None).unwrap();
        assert!(bars.is_empty());
    }
}
