//! Data-layer benchmarks for `backtide_core`.
//!
//! Measures the latency of live market-data API calls through the
//! [`DataProvider`] trait for the Yahoo Finance provider.
//!
//! **These benchmarks hit real network endpoints.** Results are inherently
//! noisier than the storage benchmarks and may vary with network conditions
//! and provider rate-limits. Each benchmark uses [`Bencher::iter_custom`]
//! to make exactly one API call per measured iteration, keeping the total
//! request count low enough to avoid rate-limiting.
//!
//! Benchmarks included:
//!
//! | Group                        | What it measures                                            |
//! |------------------------------|-------------------------------------------------------------|
//! | `ohlc_download/1sym_1m`      | Download ~7 days of 1-minute bars for 1 symbol via Yahoo.   |
//! | `ohlc_download/10sym_1d`     | Download ~30 days of daily bars for 10 symbols via Yahoo.   |
//!
//! Run with:
//!
//! ```sh
//! cargo bench --manifest-path backtide_core/Cargo.toml --bench data_bench
//! ```

use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, Criterion};
use futures::future::join_all;

use backtide_core::data::models::instrument_type::InstrumentType;
use backtide_core::data::models::interval::Interval;
use backtide_core::data::providers::traits::DataProvider;
use backtide_core::data::providers::yahoo::YahooFinance;

/// Build a [`Criterion`] instance tuned for network-bound benchmarks.
///
/// Uses the minimum sample size (10) and a short measurement / warm-up
/// window so that the total number of live API requests stays well below
/// provider rate-limits.
fn network_criterion() -> Criterion {
    Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(1))
        .measurement_time(Duration::from_secs(15))
        .noise_threshold(0.15)
}

/// Benchmark downloading ~7 days of 1-minute bars for a single symbol.
///
/// Calls [`YahooFinance::download_batch`] for `"AAPL"` with
/// [`Interval::OneMinute`] over the last 7 days. Uses `iter_custom`
/// so that each of the 10 samples performs exactly `iters` sequential
/// API calls, keeping the total request count predictable.
fn bench_ohlc_download_1sym_1m(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("failed to build tokio runtime");
    let yahoo = rt.block_on(YahooFinance::new()).expect("failed to init Yahoo provider");

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let start = now - 7 * 86_400;

    c.bench_function("ohlc_download/1sym_1m", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let t = Instant::now();
                for _ in 0..iters {
                    yahoo
                        .download_batch(
                            "AAPL",
                            InstrumentType::Stocks,
                            Interval::OneMinute,
                            start,
                            now,
                        )
                        .await
                        .expect("yahoo download failed");
                }
                t.elapsed()
            })
        });
    });
}

/// Benchmark downloading ~30 days of daily bars for 10 symbols concurrently.
///
/// Calls [`YahooFinance::download_batch`] for 10 common US stock tickers
/// with [`Interval::OneDay`] over the last 30 days. All 10 downloads run
/// concurrently via [`futures::future::join_all`] within each measured
/// iteration, mirroring real-world multi-symbol ingestion.
fn bench_ohlc_download_10sym_1d(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("failed to build tokio runtime");
    let yahoo = rt.block_on(YahooFinance::new()).expect("failed to init Yahoo provider");

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let start = now - 30 * 86_400;

    let symbols = ["AAPL", "MSFT", "GOOG", "AMZN", "TSLA", "META", "NVDA", "JPM", "V", "JNJ"];

    c.bench_function("ohlc_download/10sym_1d", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let t = Instant::now();
                for _ in 0..iters {
                    let futures = symbols.iter().map(|&sym| {
                        yahoo.download_batch(
                            sym,
                            InstrumentType::Stocks,
                            Interval::OneDay,
                            start,
                            now,
                        )
                    });
                    let results = join_all(futures).await;
                    for r in results {
                        r.expect("yahoo download failed");
                    }
                }
                t.elapsed()
            })
        });
    });
}

// ────────────────────────────────────────────────────────────────────────────
// Harness
// ────────────────────────────────────────────────────────────────────────────

criterion_group! {
    name = data_benches;
    config = network_criterion();
    targets =
        bench_ohlc_download_1sym_1m,
        bench_ohlc_download_10sym_1d,
}

criterion_main!(data_benches);
