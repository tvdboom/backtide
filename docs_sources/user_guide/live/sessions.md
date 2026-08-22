# Live simulation
---------------

Backtide can apply the same strategy objects used by the backtest engine to live
exchange candles. Market data arrives over public provider WebSockets and orders
are matched by a local live-session engine. No brokerage account is connected,
no credentials are required, and no real orders are submitted.

Live simulation is useful for checking how a strategy behaves as bars arrive, but
its fills remain a simulation. Network delay, outages, exchange liquidity, queue
position, and the difference between candle prices and executable quotes can all
make real execution differ from the simulated result.

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

Open **Live simulation** under **Live**. The setup is divided into seven focused steps:

1. **Market data** selects the provider, interval, and symbols.
2. **Portfolio** configures starting cash and the reporting currency.
3. **Strategy** selects one or more strategies and optional dashboard indicators.
4. **Metrics** selects live-compatible performance measures.
5. **Execution** configures fees, slippage, allowed order types, and optional
   candle-volume participation.
6. **Risk** configures short selling, position concentration, drawdown halts, leverage,
   initial and maintenance margin, margin interest, and short borrow cost.
7. **Engine** configures the risk-free rate, historical warm-up, bounded strategy
   history, and partial-candle behavior.

While a session is active, the dashboard shows account performance, exposure, leverage,
buying power, drawdown, costs, selected metrics, latest indicator values, fills, prices,
and connection diagnostics. You can pause strategy evaluation, resume it, cancel resting
orders, request a complete flatten, or stop the session.

When multiple strategies are selected, each receives an independent simulated account with the
configured starting cash. Orders, fills, snapshots, and metrics remain attributed to that
strategy; the headline account cards show the sum of those isolated accounts. This avoids one
strategy's orders changing another strategy's decisions while still making side-by-side forward
testing possible.

Every session is persisted in the configured Backtide DuckDB database. The **Session history**
page lists the start and finish time, status, strategies, starting equity, and final P&L. Browser
event buffers remain bounded, while the database retains the complete event journal and the exact
warm-up stream used by the session.

The `live_sessions` table stores session metadata and the latest snapshot, the ordered
`live_session_events` table stores normalized live events, and `live_session_warmup` stores the
warm-up bars separately. Snapshot metrics remain JSON objects, so custom metric names do not
require fixed database columns.

### Margin behavior

`allow_margin=True` enables bounded borrowing; it no longer means unlimited negative cash.
Exposure-increasing orders must satisfy both `max_leverage` and `initial_margin`, as well as
the per-symbol `max_position_size`. Financing costs accrue from event timestamps. When the
equity-to-gross-exposure ratio falls below `maintenance_margin`, the simulation broker halts new
exposure and liquidates marked positions deterministically.

This is Backtide's generic cross-margin simulation. It does not claim to duplicate an
exchange's product-specific liquidation engine, insurance fund, or order-book execution.

<br>

## Replays

A **replay** runs a saved live-session event stream through a new simulation engine. It re-evaluates
the strategies, indicators, sizers, metrics, order rules, and account configuration instead of
showing previously saved snapshots. No provider connection is opened, and every order remains
simulated.

### How a replay works

1. A live session records normalized market updates, receipt timestamps, exchange-rate updates,
   and its warm-up bars in the local database.
2. On **Session history**, choose 1×, 2×, 5×, 10×, or **Maximum**, then select **Replay** for a
   completed session.
3. Backtide creates a new session from the source session's saved market, strategy, portfolio,
   execution, risk, and engine settings. Recorded warm-up bars are applied before the first event
   so rolling indicators and strategies start with the same price context.
4. Events are processed in their original order. Timed modes divide the recorded delay between
   events by the selected speed; **Maximum** removes those delays. Pausing freezes the playback
   clock without consuming or discarding events, and resuming continues from the same point.
5. The replay is saved as a separate child of its source session. The live page reports event
   progress, source duration, speed, and warm-up provenance. Expand the replay count in session
   history to compare final P&L with the original.

Replays are most useful when you want to:

- reproduce a strategy decision or investigate a particular order, fill, or risk halt;
- run a long recorded session quickly without waiting for the market to produce new candles;
- compare a strategy or engine change against the same ordered market events; or
- check whether a completed session remains deterministic under the same code and configuration.

A replay is not a test of WebSocket reliability, reconnection behavior, current market latency,
or executable liquidity because it does not contact the provider. Use **Go live** to reconnect
with the saved setup when those conditions matter. Exact agreement with the original also depends
on using the same strategy definitions and Backtide version; changing code intentionally changes
what the fresh simulation engine can produce.

<br>

## Using the command line

Start a live session from TOML, YAML, or JSON with
`backtide start-live-session`. For example, save this as `live.toml`:

```toml
provider = "kraken"
symbols = ["BTC-USD"]
interval = "1m"
strategy = "my-saved-strategy"
batch_size = 10
timeout_seconds = 5

[session]
initial_cash = 25000
commission_pct = 0.1
slippage = 0.05
```

The optional `strategy` value names a strategy saved in the application's
**Library**. Omit it to monitor the feed and simulated account without generating
orders. Every field accepted by [`SessionConfig`] can be placed under
`session`.

Start the session and press Ctrl+C when you want to stop:

```console
backtide start-live-session live.toml
```

The command validates provider support before connecting, prints processed
candles with current equity and fill counts, and closes the WebSocket during
shutdown.

<br>

## Using Python

[`Session`] is deterministic when you feed it explicit
[`MarketUpdate`] objects. That makes the engine suitable for unit tests and
recorded replays without a network connection:

`SessionConfig` contains account, execution, risk, and metric settings. Its single `metrics` list
accepts both exact built-in string keys and custom Python metric objects. Strategy and indicator
instances are runtime dependencies passed to `Session`. [`Experiment`] follows the same
config-first class pattern, and its metrics likewise live only in `ExperimentConfig.metrics`.

```python
from backtide.live import MarketUpdate, Session, SessionConfig
from backtide.strategies import BuyAndHold

session = Session(
    SessionConfig(
        initial_cash=25_000,
        commission_pct=0.1,
        metrics=["pnl", "sharpe"],
    ),
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
from backtide.live import collect_market_updates, Session
from backtide.strategies import BuyAndHold

session = Session(strategy=BuyAndHold())

updates = collect_market_updates(
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
[`Session`]. This is the same public API used by the CLI and the
application:

```python
from backtide.live import LiveMarketFeed, Session
from backtide.strategies import BuyAndHold

feed = LiveMarketFeed("kraken", ["BTC-USD"], interval="1m")
session = Session(strategy=BuyAndHold())

try:
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
  timestamps in seconds before they reach the simulation engine.
- Final candles are processed by default. Set `trade_on_partial=True` only when
  a strategy is designed to evaluate repeated updates to the same candle.
- The session ignores stale or duplicate completed candles so a reconnect cannot
  trade the same bar twice.
- Market orders are simulated from the current candle with configured slippage
  and commission. Cash, positions, realized PnL, unrealized PnL, and equity are
  updated together.
- When margin is disabled, a strategy-generated buy is reduced when necessary to
  leave room for slippage and commission. An explicitly submitted oversized order
  is still rejected as `insufficient cash` instead of being changed silently.
- `allow_short` and `allow_margin` are off by default. Orders that violate the
  configured account rules are rejected with a reason in [`SessionFill`].
- `allowed_order_types` controls which market, limit, stop, trailing, settlement,
  and cancellation requests the session accepts.
- With `partial_fills=True`, a fill is capped at `max_volume_participation` of the
  current candle volume. The remainder stays open under the same order identifier and can fill
  on later candles or be canceled. Candle volume remains a simplified liquidity proxy.
- Selected built-in metrics are computed from the bounded authoritative equity and
  completed-trade history. Benchmark-relative metrics require a separate synchronized
  benchmark and therefore are not offered by the live setup.
- `max_history` bounds the bars retained per symbol for strategy evaluation.

Use [`Session.snapshot`](../../api/live/session.md) whenever you need
the latest read-only account view without processing another market event. It returns a
[`SessionSnapshot`](../../api/models/live/sessionsnapshot.md).
