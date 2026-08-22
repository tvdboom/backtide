 Strategies
-----------

Strategies are the decision-making logic that determines when to buy, sell, or
hold positions during a backtest. Each strategy receives market data, portfolio
state, and pre-computed indicator values, and returns a list of orders to
execute. Backtide provides a set of built-in strategies as well as a framework
for creating custom strategies.

<br>

## How they work

Every strategy inherits from [`BaseStrategy`] and implements a `evaluate` method
that receives data, state, and indicators, and returns a list of orders:

- **Single-asset** strategies operate on one instrument at a time, making
  buy/sell decisions based on that instrument's data and indicators.
- **Portfolio-rotation** strategies operate across multiple instruments,
  periodically ranking and rotating the portfolio into the top performers.

When running a backtest, the strategy's `evaluate` method is called on every
bar. It receives:

- `data` — `dict[str, pandas.DataFrame | polars.DataFrame]`, keyed by symbol.
  For example, `data["AAPL"]["close"]` is AAPL's close-price history through
  the current bar.
- `portfolio` — [`backtide.backtest.Portfolio`][portfolio], containing the current cash,
  positions and open orders. For example,
  `portfolio.positions.get("AAPL", 0.0)` reads AAPL's signed quantity.
- `state` — [`backtide.backtest.State`][state], containing the timestamp, bar index,
  total bar count and warmup flag. For example, `state.is_warmup` tells the
  strategy whether orders are currently suppressed.
- `indicators` —
  `dict[str, dict[str, pandas.Series | pandas.DataFrame | polars.Series | polars.DataFrame]] | None`.
  The outer key is the indicator's deterministic name and the inner key is the
  symbol. For example, `indicators["SMA_20"]["AAPL"]` is AAPL's visible 20-bar
  SMA history.

The method should return a list of [orders] to execute on the current bar.
See [`BaseStrategy.evaluate`](../../api/models/strategies/basestrategy.md#basestrategy-evaluate)
for the complete API contract and per-parameter examples.

<br>

## Orders

An order is a trade instruction. Each order is an [`Order`] object with a symbol,
a signed quantity, an order type, and — depending on the type — one or two price fields.

```python
from backtide.backtest import Order

# Buy 50 shares of AAPL at market price
Order(symbol="AAPL", order_type="market", quantity=50)

# Sell 20 shares with a limit at $185
Order(symbol="AAPL", order_type="limit", quantity=-20, price=185.0)
```

### Order types

The `order_type` field determines when and how the order is filled. You can pass
an [`OrderType`] instance or a string. Strings are parsed flexibly: PascalCase
(`"StopLoss"`) and snake\_case (`"stop_loss"`) are both accepted, case-insensitively.
Only order types listed in [`ExchangeExpConfig.allowed_order_types`][ExchangeExpConfig]
are accepted; others are rejected immediately.

| Type                    | Fills when…                                                                                                                                                                                                       | Price fields                                  |
|-------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------|
| **`Market`**            | Immediately at the next bar's open (or current close if `trade_on_close` is enabled).                                                                                                                             | —                                             |
| **`Limit`**             | The market reaches the limit price *or better*. A buy limit fills at-or-below `price`; a sell limit fills at-or-above `price`.                                                                                    | `price` = limit                               |
| **`StopLoss`**          | The market moves *against* you past the stop. A sell stop triggers when the price falls to `price`; a buy stop triggers on a rise. Once triggered, fills like a market order.                                     | `price` = stop                                |
| **`TakeProfit`**        | The market moves *in your favour* past the target. Execution semantics are identical to a limit order.                                                                                                            | `price` = target                              |
| **`StopLossLimit`**     | Same trigger as `StopLoss`, but once triggered the order converts to a **limit** resting at `limit_price` instead of filling at market.                                                                           | `price` = stop, `limit_price` = limit         |
| **`TakeProfitLimit`**   | Same trigger as `TakeProfit`, but converts to a **limit** at `limit_price`.                                                                                                                                       | `price` = target, `limit_price` = limit       |
| **`TrailingStop`**      | A stop that follows the market. The engine tracks the running high (for sells) or running low (for buys). The stop triggers when the price reverses by `price` units from the extreme. Fills like a market order. | `price` = trail amount                        |
| **`TrailingStopLimit`** | Same as `TrailingStop`, but converts to a **limit** at `limit_price` instead of filling at market.                                                                                                                | `price` = trail amount, `limit_price` = limit |
| **`SettlePosition`**    | Closes the entire open position in the symbol at a market price. Quantity is computed by the engine.                                                                                                              | —                                             |
| **`Cancel`**            | Cancels a pending order. Set `id` to the ID of the order to cancel. Other fields (`symbol`, `quantity`, `price`, `limit_price`) are ignored.                                                                   | —                                             |

!!! note
    Limit-style orders are protected against slippage: a buy limit will never fill
    above the limit price, and a sell limit will never fill below it, even after the
    configured slippage percentage is applied.

!!! warning
    Every pending order must have a unique `id`. If you submit an order whose
    `id` matches one already in the order book, the duplicate is immediately
    rejected. When you omit the `id` parameter, the engine auto-generates a
    unique one.

### Examples

```python title="Bracket order: entry with stop-loss and take-profit"
def evaluate(self, data, portfolio, state, indicators):
    orders = []
    for symbol, df in data.items():
        close = df["close"].iloc[-1]
        qty = portfolio.positions.get(symbol, 0)

        if qty == 0:
            # Enter long at market
            orders.append(Order(
                symbol=symbol,
                order_type="market",
                quantity=100,
            ))

            # Attach a stop-loss 5% below entry
            orders.append(Order(
                symbol=symbol,
                order_type="stop_loss",
                quantity=-100,
                price=close * 0.95,
            ))

            # Attach a take-profit 10% above entry
            orders.append(Order(
                symbol=symbol,
                order_type="take_profit",
                quantity=-100,
                price=close * 1.10,
            ))

    return orders
```

```python title="Trailing stop that locks in gains"
# Trail the high by $2. If the stock rises from 100 to 120 and then
# drops back to 118, the trailing stop triggers at 118.
Order(
    symbol="AAPL",
    order_type="trailing_stop",
    quantity=-100,
    price=2.0,   # Trail amount in price units
)
```

### Cancelling orders

Pending orders (limit, stop, trailing) stay in the order book until they are
filled, canceled or expire at the end of the simulation. You can inspect
currently open orders via `portfolio.orders` — each entry is an [`Order`]
object whose `id` attribute uniquely identifies it.

To cancel a specific order, submit a `Cancel` whose `id` matches the
target:

```python
# Place a limit order with a known ID
orders.append(Order(
    id="my-limit",
    symbol="AAPL",
    order_type="limit",
    quantity=50,
    price=150.0,
))

# On a later bar, cancel it
orders.append(Order(id="my-limit", order_type="cancel"))
```

!!! info
    For `Cancel` orders, only the `id` field matters. You can omit `symbol`,
    `quantity`, `price` and `limit_price` since the engine ignores them.

If you didn't assign a custom `id` when submitting the order, the engine
auto-generates one. You can retrieve it from the portfolio:

```python
from backtide.backtest import OrderType

# Cancel all pending stop-loss orders for AAPL
for pending in portfolio.orders:
    if pending.symbol == "AAPL" and pending.order_type == OrderType.StopLoss:
        orders.append(Order(id=pending.id, order_type="cancel"))
```

Alternatively, enable [`EngineExpConfig.exclusive_orders`][EngineExpConfig] to
have the engine automatically cancel all pending orders whenever a new order is
submitted. This is convenient for strategies that should only have one active
order at a time.

### Sizing

Instead of computing a numeric quantity yourself, you can pass a sizer as
`quantity`. The engine resolves the sizer into a concrete number of units just
before the order is queued.

```python
from backtide.sizers import EqualWeight, FixedFractional

# Allocate an equal slice of equity to this position
Order(symbol="AAPL", order_type="market", quantity=EqualWeight())

# Risk 2% of equity per trade
Order(symbol="AAPL", order_type="market", quantity=FixedFractional(0.02))
```

See [Sizers][sizers] for the full list of built-in sizers and how to create
custom ones.

<br>

## Auto-injected indicators

Most built-in strategies depend on a handful of indicators (e.g., SMA Crossover
needs two SMAs, BB Mean Reversion needs Bollinger Bands, etc...). To save you from
having to add those manually on every experiment, the engine auto-injects them for
you.

Auto-injected indicators behave exactly like user-selected ones — they are
computed once over the full dataset before the simulation starts and are then
sliced per bar for the strategy. They are de-duplicated across strategies, so
two strategies asking for the same `SMA(20)` only compute it once.

<br>

## Custom strategies

You can create your own strategies by subclassing `BaseStrategy`. Custom
strategies can be written directly in the [application's][application] code
editor or uploaded as `.py` files.

??? example
    ```python title="Inside-bar breakout strategy"
    from backtide.backtest import Order
    from backtide.sizers import EqualWeight
    from backtide.strategies import BaseStrategy


    class InsideBarBreakout(BaseStrategy):
        """Long-only inside-bar breakout strategy.

        Entry:
          - Previous bar is an inside bar relative to the bar before it.
          - Current close breaks above the inside bar high.

        Exit:
          - Current close falls below the inside bar low.

        """

        def __init__(self, cash_fraction=0.95):
            self.cash_fraction = cash_fraction

        def evaluate(self, data, portfolio, state, indicators):
            orders = []
            entry_candidates = []

            # First pass: determine exits and potential entries.
            for symbol, df in data.items():
                # Need at least 3 bars:
                # bar[-3] = "mother bar", bar[-2] = "inside bar", bar[-1] = current
                if len(df) < 3:
                    continue

                mother = df.iloc[-3]
                inside = df.iloc[-2]
                current = df.iloc[-1]

                current_qty = portfolio.positions.get(symbol, 0)

                is_inside_bar = inside["high"] < mother["high"] and inside["low"] > mother["low"]
                breakout_up = current["close"] > inside["high"]
                breakdown_down = current["close"] < inside["low"]

                # Exit existing long on downside break.
                if current_qty > 0 and breakdown_down:
                    orders.append(Order(symbol=symbol, order_type="market", quantity=-current_qty))
                    continue

                # Track new long entries.
                if current_qty <= 0 and is_inside_bar and breakout_up:
                    entry_candidates.append(symbol)

            # Second pass: hand sizing off to the built-in EqualWeight sizer
            # (scaled down by `cash_fraction` so the strategy keeps a cash buffer).
            if entry_candidates:
                sizer = EqualWeight(n_positions=int(len(entry_candidates) / self.cash_fraction))
                for symbol in entry_candidates:
                    orders.append(Order(symbol=symbol, order_type="market", quantity=sizer))

            return orders


    InsideBarBreakout()
    ```

Custom strategies can either compute a numeric quantity for every order or attach
a [sizer][sizers] directly to an `Order` by passing it as `quantity`. Attached sizers
are resolved by the engine just before the order is queued. The engine converts
current portfolio equity into the order instrument's quote currency.

See the [daily reversal](../../examples/strategies/daily_reversal.md) and
[moving-average trend](../../examples/strategies/moving_average_trend.md) pages for complete,
copy-ready implementations. General optimization guidance lives on the
[Performance](performance.md) page.

<br>

## Built-in strategies

Built-in strategies are divided into **single-asset** strategies (operating on one instrument)
and **portfolio-rotation** strategies (ranking and rotating across multiple
instruments). See the API reference for full details on each strategy's
parameters, attributes, and logic.

### Position sizing

Backtide uses [sizers] to turn a trading signal into an order quantity:

- **Signal-following strategies** size buys with [`FixedNotional`]: the strategy
  computes a target cash allocation for the symbol, then converts that notional
  into units at the latest known close. Sells use [`FixedQuantity`] to close the
  current position.
- **Equal-weight entries and rotation strategies** use [`EqualWeight`]: selected
  symbols receive an equal slice of current equity/cash. Rotation strategies
  liquidate symbols that leave the selected set and rebalance into the current
  winners.
- **[`BuyAndHold`]** enters each symbol once, as soon as that symbol has data, and
  does not resize afterward. If a single benchmark symbol is configured, it only
  buys that symbol.

### Single-asset strategies

| Strategy                   | Category       | Description                                                        |
|----------------------------|----------------|--------------------------------------------------------------------|
| [`AdaptiveRsi`]            | Momentum       | RSI with dynamic period adapting to volatility.                    |
| [`AlphaRsiPro`]            | Momentum       | Advanced RSI with adaptive levels and trend bias filtering.        |
| [`BollingerMeanReversion`] | Mean Reversion | Buys at the lower band, sells at the upper band.                   |
| [`BuyAndHold`]             | Baseline       | Buys on the first day and holds to the end.                        |
| [`DoubleTop`]              | Pattern        | Buys on breakout after a double-top pattern.                       |
| [`HybridAlphaRsi`]         | Momentum       | Combines adaptive period, adaptive levels, and trend confirmation. |
| [`Macd`]                   | Trend          | Buys on MACD golden cross, sells on death cross.                   |
| [`Momentum`]               | Trend          | Buys when momentum turns positive, exits on MA filter.             |
| [`RiskAverse`]             | Breakout       | Buys low-volatility stocks making new highs on volume.             |
| [`Roc`]                    | Momentum       | Buys on high Rate of Change, sells on low.                         |
| [`Rsi`]                    | Momentum       | Combines RSI and Bollinger Bands for dual confirmation.            |
| [`Rsrs`]                   | Trend          | Uses regression of high/low prices for support detection.          |
| [`SmaCrossover`]           | Trend          | Golden cross / death cross with two moving averages.               |
| [`SmaNaive`]               | Trend          | Buys above MA, sells below.                                        |
| [`TurtleTrading`]          | Trend          | Channel breakout trend-following with equal-weight entries.        |
| [`Vcp`]                    | Breakout       | Volatility Contraction Pattern breakout.                           |

### Portfolio-rotation strategies

| Strategy                   | Description                                                         |
|----------------------------|---------------------------------------------------------------------|
| [`MultiBollingerRotation`] | Rotates into instruments crossing above their upper Bollinger Band. |
| [`RocRotation`]            | Rotates into the top K instruments by Rate of Change.               |
| [`RsrsRotation`]           | Rotates into instruments with highest RSRS values.                  |
| [`TripleRsiRotation`]      | Rotates based on composite long/medium/short RSI scores.            |
