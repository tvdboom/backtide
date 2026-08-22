# Inspect a live market feed

Use a feed without a paper session when you only want to inspect normalized candle updates.

```python
from backtide.live import LiveMarketFeed

feed = LiveMarketFeed(
    provider="binance",
    symbols=["BTC-USDT", "ETH-USDT"],
    interval="1m",
    include_partial=True,
)

try:
    for update in feed.collect(max_events=20, timeout_seconds=30):
        state = "closed" if update.is_final else "partial"
        print(update.symbol, update.close, update.volume, state)
finally:
    feed.cancel()
```

Set `include_partial=False` when downstream logic should see completed candles only. A
`PaperTradingSession` already avoids strategy decisions on partial candles unless
`trade_on_partial=True` is configured.
