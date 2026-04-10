//! Storage-layer benchmarks for `backtide_core`.
//!
//! Measures the throughput and latency of the [`DuckDb`] storage backend
//! through the [`Storage`] trait. Every benchmark creates an isolated,
//! temporary DuckDB database via [`tempfile`], so iterations never interfere
//! with one another.
//!
//! Benchmarks included:
//!
//! | Group                   | What it measures                                                 |
//! |-------------------------|------------------------------------------------------------------|
//! | `batch_bar_insert`      | Insert throughput at 100 / 10 000 rows.                          |
//! | `historical_read`       | Read latency via `get_summary` for 1 and 10 symbols.            |
//!
//! Run with:
//!
//! ```sh
//! cargo bench --manifest-path backtide_core/Cargo.toml --bench storage_bench
//! ```

use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use backtide_core::data::models::asset_type::AssetType;
use backtide_core::data::models::bar::Bar;
use backtide_core::data::models::interval::Interval;
use backtide_core::data::models::provider::Provider;
use backtide_core::storage::duckdb::DuckDb;
use backtide_core::storage::models::bar_series::BarSeries;
use backtide_core::storage::traits::Storage;

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

/// Generate `n` synthetic [`Bar`] structs with sequential timestamps.
///
/// Each bar has a unique `open_ts` starting at `1_000_000_000` and
/// incrementing by 86 400 (one day). Prices follow a simple ascending
/// pattern to make data easily verifiable.
fn generate_bars(n: usize) -> Vec<Bar> {
    (0..n)
        .map(|i| {
            let ts = 1_000_000_000 + (i as u64) * 86_400;
            Bar {
                open_ts: ts,
                close_ts: ts + 86_400,
                open_ts_exchange: ts,
                open: 100.0 + i as f64,
                high: 105.0 + i as f64,
                low: 95.0 + i as f64,
                close: 102.0 + i as f64,
                adj_close: 102.0 + i as f64,
                volume: 1_000_000.0 + i as f64,
                n_trades: Some(500),
            }
        })
        .collect()
}

/// Create a fresh [`DuckDb`] instance backed by a temporary directory.
///
/// The returned [`tempfile::TempDir`] handle must be kept alive for the
/// lifetime of the [`DuckDb`]; dropping it removes the directory.
fn fresh_db() -> (DuckDb, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let db = DuckDb::new(&dir.path().to_path_buf()).expect("failed to open DuckDb");
    db.init().expect("failed to init DuckDb");
    (db, dir)
}

/// Build a [`BarSeries`] for the given symbol using the provided bars.
fn make_series(symbol: &str, bars: Vec<Bar>) -> BarSeries {
    BarSeries {
        symbol: symbol.to_owned(),
        asset_type: AssetType::Stocks,
        interval: Interval::OneDay,
        provider: Provider::Yahoo,
        bars,
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Benchmarks
// ────────────────────────────────────────────────────────────────────────────

/// Benchmark bulk-insert throughput at 100 and 10 000 bars.
///
/// For each size a fresh DuckDB is created, the bars are generated once
/// and then inserted repeatedly to measure raw write throughput via
/// [`Storage::write_bars_bulk`].
fn bench_batch_bar_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_bar_insert");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(20);

    for size in [100, 10_000] {
        let bars = generate_bars(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &bars, |b, bars| {
            b.iter_with_setup(
                || {
                    let (db, dir) = fresh_db();
                    (db, dir, vec![make_series("BENCH", bars.clone())])
                },
                |(db, _dir, series)| {
                    db.write_bars_bulk(&series).expect("write failed");
                },
            );
        });
    }

    group.finish();
}

/// Benchmark [`Storage::get_summary`] read latency for a single symbol.
///
/// Pre-populates a DuckDB instance with 1 000 daily bars for one symbol,
/// then repeatedly calls `get_summary`. Measures the grouped aggregation
/// plus sparkline retrieval for a single-series database.
fn bench_historical_read_1sym(c: &mut Criterion) {
    let (db, _dir) = fresh_db();
    let bars = generate_bars(1_000);
    db.write_bars_bulk(&[make_series("AAPL", bars)])
        .expect("seed write failed");

    c.bench_function("historical_read/1sym", |b| {
        b.iter(|| {
            db.get_summary().expect("summary query failed");
        });
    });
}

/// Benchmark [`Storage::get_summary`] read latency across 10 symbols.
///
/// Seeds the database with 10 symbols × 1 000 bars each (10 000 bars
/// total), then measures how long the grouped summary query takes,
/// including sparkline retrieval for every series.
fn bench_historical_read_10sym(c: &mut Criterion) {
    let (db, _dir) = fresh_db();

    let symbols = [
        "AAPL", "MSFT", "GOOG", "AMZN", "TSLA", "META", "NVDA", "JPM", "V", "JNJ",
    ];

    let series: Vec<BarSeries> = symbols
        .iter()
        .map(|&sym| make_series(sym, generate_bars(1_000)))
        .collect();

    db.write_bars_bulk(&series).expect("seed write failed");

    c.bench_function("historical_read/10sym", |b| {
        b.iter(|| {
            db.get_summary().expect("summary query failed");
        });
    });
}

// ────────────────────────────────────────────────────────────────────────────
// Harness
// ────────────────────────────────────────────────────────────────────────────

criterion_group!(
    storage_benches,
    bench_batch_bar_insert,
    bench_historical_read_1sym,
    bench_historical_read_10sym,
);

criterion_main!(storage_benches);
