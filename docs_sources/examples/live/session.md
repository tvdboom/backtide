# Complete live session

This bounded example connects to Kraken, collects completed one-minute BTC/USD candles, evaluates a
strategy, prints fills and equity, and always closes the feed. It submits simulated orders only.

```python
from backtide.live import LiveMarketFeed, Session, SessionConfig
from backtide.strategies import BuyAndHold

config = SessionConfig(
    initial_cash=25_000,
    commission_pct=0.10,
    slippage=0.05,
    max_position_size=25.0,
    metrics=["total_return", "sharpe", "max_dd"],
)
session = Session(config=config, strategy=BuyAndHold())
feed = LiveMarketFeed(
    provider="kraken",
    symbols=["BTC-USD"],
    interval="1m",
    include_partial=False,
)

processed = 0
try:
    while processed < 10:
        markets = feed.collect(max_events=5, timeout_seconds=15)
        if not markets:
            print("No completed candle arrived before the timeout")
            continue

        for market in markets:
            update = session.on_bar(market)
            if not update.processed:
                continue

            processed += 1
            for fill in update.fills:
                print(fill.order.symbol, fill.order.quantity, fill.fill_price)
            print(market.symbol, update.snapshot.equity, update.snapshot.metrics)

            if processed >= 10:
                break
finally:
    feed.cancel()

snapshot = session.snapshot()
print(snapshot.cash, snapshot.positions, snapshot.equity)
```

Every `collect()` call has an event limit and timeout, and the outer loop has a finite target. For a
long-running service, keep the cleanup pattern but replace the target with your own cancellation
signal.
