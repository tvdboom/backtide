# Live and paper trading
------------------------

Backtide can apply the same strategy objects used by the backtest engine to live
exchange candles. Market data arrives over public provider WebSockets and orders
are matched by a local paper-trading engine. No brokerage account is connected,
no credentials are required, and no real orders are submitted.

Paper trading is useful for checking how a strategy behaves as bars arrive, but
its fills remain a simulation. Network delay, outages, exchange liquidity, queue
position, and the difference between candle prices and executable quotes can all
make real execution differ from the paper result.

<br>

## Provider support

| Provider | Live candles | Notes |
|---|---|---|
| **Binance** | Yes | Public spot kline WebSocket at the intervals supported by Backtide. |
| **Kraken** | Yes | Public Spot WebSocket v2 OHLC feed. |
| **Coinbase** | Yes | The public candles channel emits five-minute candles only. |
| **Yahoo Finance** | No | Yahoo does not publish an official market-data WebSocket. Use an exchange provider for live mode. |

Historical downloads still support every provider described in the [data guide][data].
Live support is checked separately with [`provider_live_support`].

<br>

## Using the application

Start the packaged web application as usual:

```console
backtide launch
```

Open **Paper trading** under **Live**, then:

1. Choose Binance, Kraken, or Coinbase and a supported candle interval.
2. Add one or more canonical symbols, such as `BTC-USDT`.
3. Select a saved strategy and configure starting cash, commission, slippage,
   short selling, margin, and partial-candle behavior.
4. Start the session. The equity chart, latest prices, positions, fills, and
   market-event feed update while the session is running.
5. Stop the session before changing its configuration.

The session is deliberately local and in-memory. Its bounded event history keeps
long-running browser sessions from growing without limit. Stopping the process
closes the current feed and discards the paper account unless you export the
values yourself.

<br>

## Using Python

[`PaperTradingSession`] is deterministic when you feed it explicit
[`MarketUpdate`] objects. That makes the engine suitable for unit tests and
recorded replays without a network connection:

```python
from backtide.live import MarketUpdate, PaperTradingConfig, PaperTradingSession
from backtide.strategies import BuyAndHold

session = PaperTradingSession(
    PaperTradingConfig(initial_cash=25_000, commission_pct=0.1),
    strategy=BuyAndHold(),
)

transition = session.on_bar(
    MarketUpdate(
        symbol="BTC-USDT",
        interval="1m",
        open_ts=1_800_000_000,
        close_ts=1_800_000_059,
        open=100_000,
        high=100_100,
        low=99_900,
        close=100_050,
        volume=12.5,
    )
)

print(transition.snapshot.equity)
```

For a bounded batch from a real provider, use [`collect_market_updates`]:

```python
from backtide.live import collect_market_updates, PaperTradingSession
from backtide.strategies import BuyAndHold

session = PaperTradingSession(strategy=BuyAndHold())

updates = collect_market_updates(  # norun
    "binance",
    ["BTC-USDT"],
    interval="1m",
    max_events=20,
    timeout_seconds=30,
)
for market in updates:
    session.on_bar(market)
```

The collector always has both an event limit and a timeout. A timeout returns the
events received so far instead of leaving the caller blocked indefinitely. For a
longer-lived worker, create a [`LiveMarketFeed`], call `collect` in bounded batches,
and call `cancel` during shutdown. The feed reconnects transient disconnections with
bounded exponential backoff.

<br>

## Candle and fill semantics

- Provider payloads are normalized to Backtide's canonical symbols and Unix
  timestamps in seconds before they reach the paper engine.
- Final candles are processed by default. Set `trade_on_partial=True` only when
  a strategy is designed to evaluate repeated updates to the same candle.
- The session ignores stale or duplicate completed candles so a reconnect cannot
  trade the same bar twice.
- Market orders are paper-filled from the current candle with configured slippage
  and commission. Cash, positions, realized PnL, unrealized PnL, and equity are
  updated together.
- `allow_short` and `allow_margin` are off by default. Orders that violate the
  configured account rules are rejected with a reason in [`PaperFill`].
- `max_history` bounds the bars retained per symbol for strategy evaluation.

Use [`PaperTradingSession.snapshot`](../api/models/live/papertradingsession.md) whenever you need
the latest read-only account view without processing another market event. It returns a
[`PaperTradingSnapshot`](../api/models/live/papertradingsnapshot.md).
