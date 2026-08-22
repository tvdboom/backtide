//! Live simulation engine benchmarks.
//!
//! Measures the deterministic hot paths used for every completed live candle:
//! mark-to-market processing and market-order execution. Network latency is
//! intentionally excluded.

use backtide_core::backtest::models::{Order, OrderId, OrderType};
use backtide_core::live::engine::SessionBroker;
use backtide_core::live::models::{MarketUpdate, SessionConfig};
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

fn market(timestamp: u64) -> MarketUpdate {
    MarketUpdate {
        provider: "mock".to_owned(),
        symbol: "BTC-USD".to_owned(),
        quote_currency: Some("USD".to_owned()),
        interval: "1m".to_owned(),
        open_ts: timestamp,
        close_ts: timestamp + 60,
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.5,
        volume: 10.0,
        n_trades: Some(5),
        is_final: true,
        received_ts: timestamp as i64 + 60,
    }
}

fn order() -> Order {
    Order {
        id: OrderId::new(),
        symbol: "BTC-USD".to_owned(),
        quantity: 0.01,
        order_type: OrderType::Market,
        price: None,
        limit_price: None,
        sizer: None,
    }
}

fn bench_live_engine(criterion: &mut Criterion) {
    criterion.bench_function("live/process_market_update", |bencher| {
        bencher.iter_batched(
            || SessionBroker::new(SessionConfig::default()).unwrap(),
            |mut broker| broker.process(market(1_700_000_000), Vec::new()),
            BatchSize::SmallInput,
        );
    });

    criterion.bench_function("live/process_market_order", |bencher| {
        bencher.iter_batched(
            || SessionBroker::new(SessionConfig::default()).unwrap(),
            |mut broker| broker.process(market(1_700_000_000), vec![order()]),
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_live_engine);
criterion_main!(benches);
