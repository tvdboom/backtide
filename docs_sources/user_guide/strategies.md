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

| Strategy | Category | Description                                                        |
|----------|----------|--------------------------------------------------------------------|
| [`AdaptiveRsi`] | Momentum | RSI with dynamic period adapting to volatility.                    |
| [`AlphaRsiPro`] | Momentum | Advanced RSI with adaptive levels and trend bias filtering.        |
| [`BollingerMeanReversion`] | Mean Reversion | Buys at the lower band, sells at the upper band.                   |
| [`BuyAndHold`] | Baseline | Buys on the first day and holds to the end.                        |
| [`DoubleTop`] | Pattern | Buys on breakout after a double-top pattern.                       |
| [`HybridAlphaRsi`] | Momentum | Combines adaptive period, adaptive levels, and trend confirmation. |
| [`Macd`] | Trend | Buys on MACD golden cross, sells on death cross.                   |
| [`Momentum`] | Trend | Buys when momentum turns positive, exits on MA filter.             |
| [`RiskAverse`] | Breakout | Buys low-volatility stocks making new highs on volume.             |
| [`Roc`] | Momentum | Buys on high Rate of Change, sells on low.                         |
| [`Rsi`] | Momentum | Combines RSI and Bollinger Bands for dual confirmation.            |
| [`Rsrs`] | Trend | Uses regression of high/low prices for support detection.          |
| [`SmaCrossover`] | Trend | Golden cross / death cross with two moving averages.               |
| [`SmaNaive`] | Trend | Buys above MA, sells below.                                        |
| [`TurtleTrading`] | Trend | Channel breakout trend-following with equal-weight entries.        |
| [`Vcp`] | Breakout | Volatility Contraction Pattern breakout.                           |

### Portfolio-rotation strategies

| Strategy | Description                                                    |
|----------|----------------------------------------------------------------|
| [`MultiBollingerRotation`] | Rotates into stocks crossing above their upper Bollinger Band. |
| [`RocRotation`] | Rotates into the top K stocks by Rate of Change.               |
| [`RsrsRotation`] | Rotates into stocks with highest RSRS values.                  |
| [`TripleRsiRotation`] | Rotates based on composite long/medium/short RSI scores.       |
