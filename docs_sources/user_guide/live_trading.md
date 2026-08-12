# Paper trading
---------------

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
The application checks live support before starting a session and only offers intervals
available from the selected exchange.

<br>

## Using the application

Start the packaged web application as usual:

```console
backtide launch
```

Open **Paper trading** under **Trading**. The setup is divided into five focused steps:

1. **Market** selects the provider, interval, symbols, historical warm-up, and bounded
   strategy history.
2. **Strategy & monitoring** selects zero or more strategies, optional dashboard
   indicators, live-compatible metrics, and partial-candle behavior. Strategy-required
   indicators are always included automatically.
3. **Account & execution** configures starting cash, fees, slippage, allowed order
   types, and optional candle-volume participation.
4. **Risk** configures short selling, position concentration, drawdown halts, leverage,
   initial and maintenance margin, margin interest, and short borrow cost.
5. **Review** summarizes the complete session before connection.

While a session is active, the dashboard shows account performance, exposure, leverage,
buying power, drawdown, costs, selected metrics, latest indicator values, fills, prices,
and connection diagnostics. You can pause strategy evaluation, resume it, cancel resting
orders, request a complete flatten, or stop the session.

When multiple strategies are selected, each receives an independent paper account with the
configured starting cash. Orders, fills, snapshots, and metrics remain attributed to that
strategy; the headline account cards show the sum of those isolated accounts. This avoids one
strategy's orders changing another strategy's decisions while still making side-by-side forward
testing possible.

Every session is persisted beneath the configured Backtide storage directory. The
**Session history** page lists completed sessions and can replay their recorded normalized
market events through a new paper engine. Browser event buffers remain bounded, while the
on-disk journal retains the complete recorded session. The table shows both the start and finish
timestamps. **Live paper** means the journal came from a real-time provider feed; **Replay** means
the same normalized events were subsequently processed from disk. Both modes use simulated fills
and never submit broker orders. Replays preserve event ordering and reuse the saved session
settings, but process the journal as quickly as the engine allows rather than reconnecting to the
provider or waiting for the original wall-clock intervals. Use **Go live** instead to reconnect to
the provider immediately with the session's saved market, strategy, account, execution, and risk
settings.

### Margin behavior

`allow_margin=True` enables bounded borrowing; it no longer means unlimited negative cash.
Exposure-increasing orders must satisfy both `max_leverage` and `initial_margin`, as well as
the per-symbol `max_position_size`. Financing costs accrue from event timestamps. When the
equity-to-gross-exposure ratio falls below `maintenance_margin`, the paper broker halts new
exposure and liquidates marked positions deterministically.

This is Backtide's generic cross-margin simulation. It does not claim to duplicate an
exchange's product-specific liquidation engine, insurance fund, or order-book execution.

<br>

## Using the command line

Start a paper-trading session from TOML, YAML, or JSON with
`backtide start-live-session`. For example, save this as `live.toml`:

```toml
provider = "kraken"
symbols = ["BTC-USD"]
interval = "1m"
strategy = "my-saved-strategy"
batch_size = 10
timeout_seconds = 5

[paper]
initial_cash = 25000
commission_pct = 0.1
slippage = 0.05
```

The optional `strategy` value names a strategy saved in the application's
**Library**. Omit it to monitor the feed and paper account without generating
orders. Every field accepted by [`PaperTradingConfig`] can be placed under
`paper`.

Start the session and press Ctrl+C when you want to stop:

```console
backtide start-live-session live.toml
```

The command validates provider support before connecting, prints processed
candles with current equity and fill counts, and closes the WebSocket during
shutdown.

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
events received so far instead of leaving the caller blocked indefinitely.

To run continuously from Python, compose [`LiveMarketFeed`] with
[`PaperTradingSession`]. This is the same public API used by the CLI and the
application:

```python
from backtide.live import LiveMarketFeed, PaperTradingSession
from backtide.strategies import BuyAndHold

feed = LiveMarketFeed("kraken", ["BTC-USD"], interval="1m")
session = PaperTradingSession(strategy=BuyAndHold())

try:  # norun
    while True:
        for market in feed.collect(max_events=10, timeout_seconds=5):
            transition = session.on_bar(market)
            if transition.processed:
                print(market.symbol, transition.snapshot.equity)
except KeyboardInterrupt:
    pass
finally:
    feed.cancel()

final_snapshot = session.snapshot()
feed.cancel()  # hide
```

Each `collect` call remains bounded, while the outer loop keeps the session
running until interrupted. `cancel` closes the retained WebSocket during
shutdown. The feed reconnects transient disconnections with bounded exponential
backoff.

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
- When margin is disabled, a strategy-generated buy is reduced when necessary to
  leave room for slippage and commission. An explicitly submitted oversized order
  is still rejected as `insufficient cash` instead of being changed silently.
- `allow_short` and `allow_margin` are off by default. Orders that violate the
  configured account rules are rejected with a reason in [`PaperFill`].
- `allowed_order_types` controls which market, limit, stop, trailing, settlement,
  and cancellation requests the session accepts.
- With `partial_fills=True`, a fill is capped at `max_volume_participation` of the
  current candle volume. The remainder stays open under the same order identifier and can fill
  on later candles or be canceled. Candle volume remains a simplified liquidity proxy.
- Selected built-in metrics are computed from the bounded authoritative equity and
  completed-trade history. Benchmark-relative metrics require a separate synchronized
  benchmark and therefore are not offered by the live setup.
- `max_history` bounds the bars retained per symbol for strategy evaluation.

Use [`PaperTradingSession.snapshot`](../api/live/papertradingsession.md) whenever you need
the latest read-only account view without processing another market event. It returns a
[`PaperTradingSnapshot`](../api/models/live/papertradingsnapshot.md).
