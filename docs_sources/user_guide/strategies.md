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

- `data` — OHLCV data available up to the current bar.
- `portfolio` — the current [portfolio] (cash, positions and open orders).
- `state` — the current [state] (timestamp, bar index, warmup flag, etc...).
- `indicators` — pre-computed [indicator values][indicators] (only up to the current bar).

The method should return a list of [orders] to execute on the current bar.

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
| **`Cancel`**            | Cancels a pending order. Set `id` to the ID of the order to cancel.                                                                                                                                               | —                                             |

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
orders.append(Order(
    id="my-limit",
    order_type="cancel",
    symbol="AAPL",
    quantity=0,
))
```

If you didn't assign a custom `id` when submitting the order, the engine
auto-generates one. You can retrieve it from the portfolio:

```python
from backtide.backtest import OrderType

# Cancel all pending stop-loss orders for AAPL
for pending in portfolio.orders:
    if pending.symbol == "AAPL" and pending.order_type == OrderType.StopLoss:
        orders.append(Order(
            id=pending.id,
            order_type="cancel",
            symbol="AAPL",
            quantity=0,
        ))
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
    from math import floor

    from backtide.backtest import Order
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
                    entry_candidates.append((symbol, float(current["close"])))

            # Second pass: size entries from currently available cash.
            if entry_candidates:
                available_cash = sum(portfolio.cash.values()) * self.cash_fraction
                cash_per_trade = available_cash / len(entry_candidates)

                for symbol, close in entry_candidates:
                    qty = floor(cash_per_trade / close)
                    if qty > 0:
                        orders.append(Order(symbol=symbol, order_type="market", quantity=qty))

            return orders


    InsideBarBreakout()
    ```

Custom strategies can either compute a numeric quantity for every order or attach
a [sizer][sizers] directly to an `Order` by passing it as `quantity`. Attached sizers
are resolved by the engine just before the order is queued. The engine converts
current portfolio equity into the order instrument's quote currency.

### Performance

Backtide is fast because the hot path is deliberately kept out of Python. The
experiment engine, order matching, portfolio accounting, currency conversion,
metrics and built-in strategies are implemented in Rust.

A custom strategy's performance is mostly determined by what happens inside
`evaluate()`, because that method is called once per bar. Recommended patterns
are:

| Do                                                                                            | Avoid                                                                      |
|-----------------------------------------------------------------------------------------------|----------------------------------------------------------------------------|
| Declare expensive rolling features in `required_indicators()`.                                | Recomputing SMA/RSI/ATR/rolling statistics inside `evaluate()`.            |
| Keep state on the strategy object for incremental logic.                                      | Rebuilding large temporary lists, dicts or dataframes every bar.           |
| Use built-in indicators and built-in strategies when they match your idea.                    | Reimplementing existing Rust-backed functionality in Python.               |
| Vectorize heavy array calculations with NumPy, Polars or pandas outside the hot loop.         | Python `for` loops over long histories inside `evaluate()`.                |
| Use `numba.njit` for expensive custom numeric kernels, and compile them outside `evaluate()`. | Decorating/compiling functions dynamically inside `evaluate()`.            |
| Return only the orders you actually want to place.                                            | Returning duplicate orders every bar when a position/order already exists. |

!!! tip
    If a custom strategy is still slow, profile the `evaluate()` method first.
    In most cases, the fix is to move historical calculations into an indicator,
    replace Python loops with vectorized operations, or precompile numeric
    helpers with [Numba].

??? example
    The strategy below computes a custom z-score momentum signal that would be
    slow in a plain Python loop. The heavy lifting is offloaded to a `@njit`
    compiled helper that is defined and compiled once at import time, outside
    `evaluate()`. Make sure to have [numba] installed in your environment to use
    this example.

    ```python title="Z-score momentum strategy with Numba"
    from math import floor

    import numpy as np
    from numba import njit

    from backtide.backtest import Order
    from backtide.strategies import BaseStrategy


    @njit
    def zscore_momentum(closes: np.ndarray, lookback: int, threshold: float):
        """Return 1 (buy), -1 (sell) or 0 (hold) based on z-score momentum.

        The z-score measures how far the latest close deviates from the
        rolling mean of the last `lookback` closes, expressed in standard
        deviations. A reading above `+threshold` suggests unusual upward
        momentum (buy signal); below `-threshold` suggests the opposite.

        Parameters
        ----------
        closes : np.ndarray
            1-D float64 array of close prices up to the current bar.

        lookback : int
            Rolling window length for mean and standard deviation.

        threshold : float
            Number of standard deviations required to trigger a signal.

        Returns
        -------
        int
            1 for buy, -1 for sell, 0 for hold.

        """
        n = closes.shape[0]
        if n < lookback:
            return 0

        window = closes[-lookback:]

        total = 0.0
        for i in range(lookback):
            total += window[i]
        mean = total / lookback

        var = 0.0
        for i in range(lookback):
            diff = window[i] - mean
            var += diff * diff
        std = (var / lookback) ** 0.5

        if std == 0.0:
            return 0

        zscore = (closes[-1] - mean) / std
        if zscore > threshold:
            return 1
        elif zscore < -threshold:
            return -1
        return 0


    class ZScoreMomentum(BaseStrategy):
        """Long/flat strategy driven by z-score momentum.

        Uses a Numba-compiled kernel to compute a rolling z-score of close
        prices. When the z-score exceeds the upper threshold, the strategy
        goes long with a fixed fraction of available cash. When it drops
        below the lower threshold, existing positions are closed.

        """

        def __init__(self, lookback=20, threshold=1.5, cash_fraction=0.95):
            self.lookback = lookback
            self.threshold = threshold
            self.cash_fraction = cash_fraction

        def evaluate(self, data, portfolio, state, indicators):
            orders = []

            for symbol, df in data.items():
                closes = df["close"].to_numpy(dtype=np.float64)

                signal = zscore_momentum(closes, self.lookback, self.threshold)
                current_qty = portfolio.positions.get(symbol, 0)

                if signal == 1 and current_qty <= 0:
                    available = sum(portfolio.cash.values()) * self.cash_fraction
                    price = closes[-1]
                    qty = floor(available / price)
                    if qty > 0:
                        orders.append(Order(symbol=symbol, order_type="market", quantity=qty))

                elif signal == -1 and current_qty > 0:
                    orders.append(Order(symbol=symbol, order_type="market", quantity=-current_qty))

            return orders


    ZScoreMomentum()
    ```

    The first call to `zscore_momentum` triggers Numba's JIT compilation (a few
    hundred milliseconds). Every subsequent call runs at machine-code speed, often
    10–100x faster than the equivalent pure-Python loop.

<br>

## Built-in strategies

All built-in strategies are implemented in Rust and exposed to Python. They
are divided into **single-asset** strategies (operating on one instrument)
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

| Strategy                   | Description                                                    |
|----------------------------|----------------------------------------------------------------|
| [`MultiBollingerRotation`] | Rotates into stocks crossing above their upper Bollinger Band. |
| [`RocRotation`]            | Rotates into the top K stocks by Rate of Change.               |
| [`RsrsRotation`]           | Rotates into stocks with highest RSRS values.                  |
| [`TripleRsiRotation`]      | Rotates based on composite long/medium/short RSI scores.       |
