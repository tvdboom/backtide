//! Backtest benchmarks for `backtide_core`.
//!
//! Times the per-bar `decide` loop of every built-in strategy over a
//! synthetic AAPL-like price history (~11 000 daily bars ≈ 44 years).
//! Indicator values (SMA, RSI, MACD, BB, ATR) are pre-computed in pure
//! Rust via the [`Indicator`] trait so that the benchmark exercises only
//! the strategy-decision hot path — the same code the engine dispatches
//! to on every bar for built-in strategies.
//!
//! Benchmarks included:
//!
//! | Group                          | What it measures                                      |
//! |--------------------------------|-------------------------------------------------------|
//! | `backtest/<StrategyName>`      | Full decide loop for a single strategy over all bars. |
//!
//! Run with:
//!
//! ```sh
//! cargo bench --manifest-path backtide_core/Cargo.toml --bench backtest_bench
//! ```

use std::collections::HashMap;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

use backtide_core::backtest::indicators::{
    AverageTrueRange, BollingerBands, Indicator, MovingAverageConvergenceDivergence,
    RelativeStrengthIndex, SimpleMovingAverage,
};
use backtide_core::backtest::models::portfolio::Portfolio;
use backtide_core::backtest::models::state::State;
use backtide_core::strategies::interface::{
    AdaptiveRsi, AlphaRsiPro, BollingerMeanReversion, BuiltinStrategy, BuyAndHold, DoubleTop,
    HybridAlphaRsi, IndicatorView, Macd, Momentum, MultiBollingerRotation, RiskAverse, Roc,
    RocRotation, Rsi, Rsrs, RsrsRotation, SmaCrossover, SmaNaive, TripleRsiRotation, TurtleTrading,
    Vcp,
};
use backtide_core::data::models::bar::Bar;
use backtide_core::data::models::currency::Currency;
use backtide_core::data::models::instrument_type::InstrumentType;

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

/// Number of daily bars to simulate (~44 years of trading days).
const N_BARS: usize = 11_000;

/// Mirror of the private `auto_indicator_name` helper in `strategies.rs`.
fn auto_indicator_name(acronym: &str, args: &[String]) -> String {
    let arg_str = if args.is_empty() {
        "default".to_owned()
    } else {
        args.join("_")
    };
    let sanitized = arg_str.replace('.', "p").replace('-', "n").replace(' ', "");
    format!("{acronym}_{sanitized}")
}

fn fmt_arg<T: std::fmt::Debug>(v: T) -> String {
    format!("{v:?}")
}

/// Generate synthetic AAPL-like bars with a gentle upward drift and
/// realistic-looking OHLCV structure.
fn generate_aapl_bars() -> Vec<Bar> {
    let mut bars = Vec::with_capacity(N_BARS);
    let mut price: f64 = 0.50; // AAPL-like starting price (pre-split)
    let base_ts: u64 = 347_155_200; // ~1981-01-02 in Unix seconds

    for i in 0..N_BARS {
        // Deterministic pseudo-random walk (no external RNG needed).
        let phase = (i as f64) * 0.1;
        let drift = 0.0003; // slight upward bias
        let noise = (phase.sin() * 0.02) + (phase * 0.37).cos() * 0.015;
        price *= 1.0 + drift + noise;
        price = price.max(0.10);

        let open = price;
        let high = price * (1.0 + 0.005 + (phase * 1.3).sin().abs() * 0.02);
        let low = price * (1.0 - 0.005 - (phase * 1.7).cos().abs() * 0.02);
        let close = low + (high - low) * (0.4 + 0.2 * (phase * 0.9).sin());
        let volume = 50_000_000.0 + 20_000_000.0 * (phase * 0.5).sin();

        let ts = base_ts + (i as u64) * 86_400;
        bars.push(Bar {
            open_ts: ts,
            close_ts: ts + 86_400,
            open_ts_exchange: ts,
            open,
            high,
            low,
            close,
            adj_close: close,
            volume,
            n_trades: Some(500_000),
        });
    }
    bars
}

/// Pre-compute all indicators any built-in strategy might need and
/// return the `name -> symbol -> Vec<series>` map expected by [`IndicatorView`].
fn precompute_indicators(bars: &[Bar]) -> HashMap<String, HashMap<String, Vec<Vec<f64>>>> {
    let sym = "AAPL";
    let mut map: HashMap<String, HashMap<String, Vec<Vec<f64>>>> = HashMap::new();

    // Helper to insert indicator results under the canonical name.
    let mut insert = |name: String, series: Vec<Vec<f64>>| {
        let mut per_sym = HashMap::new();
        per_sym.insert(sym.to_owned(), series);
        map.insert(name, per_sym);
    };

    // SMA periods used by strategies: 14, 20, 50
    for p in [14, 20, 50] {
        let sma = SimpleMovingAverage::new(p);
        insert(auto_indicator_name("SMA", &[fmt_arg(p)]), sma.compute_inner(bars));
    }

    // RSI periods: 5, 8, 14, 28
    for p in [5, 8, 14, 28] {
        let rsi = RelativeStrengthIndex::new(p);
        insert(auto_indicator_name("RSI", &[fmt_arg(p)]), rsi.compute_inner(bars));
    }

    // Bollinger Bands: (20, 2.0)
    {
        let bb = BollingerBands::new(20, 2.0);
        insert(
            auto_indicator_name("BB", &[fmt_arg(20_usize), fmt_arg(2.0_f64)]),
            bb.compute_inner(bars),
        );
    }

    // MACD: (12, 26, 9)
    {
        let macd = MovingAverageConvergenceDivergence::new(12, 26, 9);
        insert(
            auto_indicator_name("MACD", &[fmt_arg(12_usize), fmt_arg(26_usize), fmt_arg(9_usize)]),
            macd.compute_inner(bars),
        );
    }

    // ATR periods: 14, 20
    for p in [14, 20] {
        let atr = AverageTrueRange::new(p);
        insert(auto_indicator_name("ATR", &[fmt_arg(p)]), atr.compute_inner(bars));
    }

    map
}

/// Build a starting portfolio with $100 000 USD.
fn starting_portfolio() -> Portfolio {
    let mut cash = HashMap::new();
    cash.insert(Currency::USD, 100_000.0);
    Portfolio {
        cash,
        positions: HashMap::new(),
        orders: Vec::new(),
    }
}

/// Run a single strategy's `decide` method over all bars, simulating the
/// engine's per-bar dispatch loop. Returns the total number of orders
/// generated (used as a black-box output to prevent dead-code
/// elimination).
fn run_strategy_loop(
    strategy: &BuiltinStrategy,
    closes: &[f64],
    indicators: &HashMap<String, HashMap<String, Vec<Vec<f64>>>>,
    total_bars: usize,
) -> usize {
    let mut portfolio = starting_portfolio();
    let mut total_orders = 0usize;
    let mut instrument_types = HashMap::new();
    instrument_types.insert("AAPL".to_owned(), InstrumentType::Stocks);

    for bar_idx in 0..total_bars {
        let slice = &closes[..=bar_idx];
        let closes_view: Vec<(String, &[f64])> = vec![("AAPL".to_owned(), slice)];
        let ind_view = IndicatorView::new(indicators, bar_idx);
        let state = State {
            timestamp: 347_155_200 + (bar_idx as i64) * 86_400,
            bar_index: bar_idx as u64,
            total_bars: total_bars as u64,
            is_warmup: false,
        };

        let orders = strategy.decide(
            &closes_view,
            &ind_view,
            &portfolio,
            &state,
            &instrument_types,
            InstrumentType::Stocks,
        );
        total_orders += orders.len();

        // Naively apply market orders to portfolio so strategies see
        // positions on subsequent bars (keeps BuyAndHold & friends
        // from repeatedly ordering).
        for o in &orders {
            let price = closes[bar_idx];
            if price <= 0.0 {
                continue;
            }
            let cur = portfolio.positions.get(&o.symbol).copied().unwrap_or(0.0);
            if o.quantity > 0.0 {
                // Buy: deduct cash, add position.
                let cost = o.quantity * price;
                if let Some(c) = portfolio.cash.get_mut(&Currency::USD) {
                    *c -= cost;
                }
                portfolio.positions.insert(o.symbol.clone(), cur + o.quantity);
            } else if o.quantity < 0.0 {
                // Sell: add cash, remove position.
                let proceeds = o.quantity.abs() * price;
                if let Some(c) = portfolio.cash.get_mut(&Currency::USD) {
                    *c += proceeds;
                }
                let new_pos = cur + o.quantity;
                if new_pos.abs() < 1e-12 {
                    portfolio.positions.remove(&o.symbol);
                } else {
                    portfolio.positions.insert(o.symbol.clone(), new_pos);
                }
            }
        }
    }
    total_orders
}

/// Criterion configuration for backtest benchmarks.
fn backtest_criterion() -> Criterion {
    Criterion::default().sample_size(10).measurement_time(Duration::from_secs(10))
}

// ────────────────────────────────────────────────────────────────────────────
// Benchmark functions — one per built-in strategy
// ────────────────────────────────────────────────────────────────────────────

macro_rules! bench_strategy {
    ($fn_name:ident, $variant:ident, $ctor:expr) => {
        fn $fn_name(c: &mut Criterion) {
            let bars = generate_aapl_bars();
            let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
            let indicators = precompute_indicators(&bars);
            let strategy = BuiltinStrategy::$variant($ctor);

            c.bench_function(concat!("backtest/", stringify!($variant)), |b| {
                b.iter(|| run_strategy_loop(&strategy, &closes, &indicators, N_BARS));
            });
        }
    };
}

bench_strategy!(bench_adaptive_rsi, AdaptiveRsi, AdaptiveRsi::new(8, 28));
bench_strategy!(bench_alpha_rsi_pro, AlphaRsiPro, AlphaRsiPro::new(14, 20));
bench_strategy!(
    bench_bollinger_mean_reversion,
    BollingerMeanReversion,
    BollingerMeanReversion::new(20, 2.0)
);
bench_strategy!(bench_buy_and_hold, BuyAndHold, BuyAndHold::new(None));
bench_strategy!(bench_double_top, DoubleTop, DoubleTop::new(60));
bench_strategy!(bench_hybrid_alpha_rsi, HybridAlphaRsi, HybridAlphaRsi::new(8, 28, 20));
bench_strategy!(bench_macd, Macd, Macd::new(12, 26, 9));
bench_strategy!(bench_momentum, Momentum, Momentum::new(14, 50));
bench_strategy!(
    bench_multi_bb_rotation,
    MultiBollingerRotation,
    MultiBollingerRotation::new(20, 2.0, 5, 20)
);
bench_strategy!(bench_risk_averse, RiskAverse, RiskAverse::new(14, 20));
bench_strategy!(bench_roc, Roc, Roc::new(12));
bench_strategy!(bench_roc_rotation, RocRotation, RocRotation::new(12, 5, 20));
bench_strategy!(bench_rsi, Rsi, Rsi::new(14, 20, 2.0));
bench_strategy!(bench_rsrs, Rsrs, Rsrs::new(18));
bench_strategy!(bench_rsrs_rotation, RsrsRotation, RsrsRotation::new(18, 5, 20));
bench_strategy!(bench_sma_crossover, SmaCrossover, SmaCrossover::new(20, 50));
bench_strategy!(bench_sma_naive, SmaNaive, SmaNaive::new(20));
bench_strategy!(
    bench_triple_rsi_rotation,
    TripleRsiRotation,
    TripleRsiRotation::new(5, 14, 28, 5, 20)
);
bench_strategy!(bench_turtle_trading, TurtleTrading, TurtleTrading::new(20, 10, 20));
bench_strategy!(bench_vcp, Vcp, Vcp::new(60, 3));

// ────────────────────────────────────────────────────────────────────────────
// Harness
// ────────────────────────────────────────────────────────────────────────────

criterion_group! {
    name = backtest_benches;
    config = backtest_criterion();
    targets =
        bench_adaptive_rsi,
        bench_alpha_rsi_pro,
        bench_bollinger_mean_reversion,
        bench_buy_and_hold,
        bench_double_top,
        bench_hybrid_alpha_rsi,
        bench_macd,
        bench_momentum,
        bench_multi_bb_rotation,
        bench_risk_averse,
        bench_roc,
        bench_roc_rotation,
        bench_rsi,
        bench_rsrs,
        bench_rsrs_rotation,
        bench_sma_crossover,
        bench_sma_naive,
        bench_triple_rsi_rotation,
        bench_turtle_trading,
        bench_vcp,
}

criterion_main!(backtest_benches);
