 Strategies
-----------

Strategies are the decision-making logic that determines when to buy, sell, or
hold positions during a backtest. Each strategy receives market data, portfolio
state, and pre-computed indicator values, and returns a list of orders to
execute. Backtide provides a set of built-in strategies as well as a framework
for creating custom strategies.

<br>

## How they work

Every strategy inherits from `BaseStrategy` and implements an `evaluate` method
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

```python
from backtide.strategies import SmaCrossover

strategy = SmaCrossover(fast_period=20, slow_period=50)
orders = strategy.evaluate(data, portfolio, state, indicators)
```

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

You don't need to think about this for built-in strategies. For [custom strategies](#custom-strategies),
you can declare auto-included indicators yourself by overriding the `required_indicators`
method on your subclass (note the `__auto_` prefix to avoid naming conflicts with
user-defined indicators). The engine will then compute and inject those indicators
into your strategy's `evaluate` method just like it does for built-in ones.:

```python
from backtide.indicators import SimpleMovingAverage
from backtide.strategies import BaseStrategy


class MyStrategy(BaseStrategy):
    def __init__(self, period=20):
        self.period = period

    def required_indicators(self):
        return [SimpleMovingAverage(self.period)]

    def evaluate(self, data, portfolio, state, indicators):
        # Read the auto-injected SMA via its deterministic key.
        sma = indicators[f"__auto_SMA_{self.period}"]
        ...
```

<br>

## Custom strategies

You can create your own strategies by subclassing `BaseStrategy`. Custom
strategies can be written directly in the [application's][application] code
editor or uploaded as `.py` files.

```python
from backtide.strategies import BaseStrategy


class MyStrategy(BaseStrategy):
    def evaluate(self, data, portfolio, state, indicators):
        orders = []

        # Your logic here ...

        return orders


MyStrategy()
```

<br>

## Built-in strategies

All built-in strategies are implemented in Rust and exposed to Python. They
are divided into **single-asset** strategies (operating on one instrument)
and **portfolio-rotation** strategies (ranking and rotating across multiple
instruments). See the API reference for full details on each strategy's
parameters, attributes, and logic.

### Position sizing

Built-in strategies don't take a `quantity` parameter — they size every order
automatically from the portfolio's current cash, so the same strategy works
unchanged across different starting capitals and prices. The exact rule depends
on the strategy family:

- **Single-asset strategies.** When a buy signal fires for a symbol, the
  strategy allocates an equal-weight slice of the *current* cash balance to it:
  `cash / N`, where `N` is the number of symbols configured in the experiment.
  That cash is divided by the next-bar fill price to obtain the integer share
  quantity. Symbols enter independently — a slow-history symbol that becomes
  available later still gets its `1 / N` slice of whatever cash is left at that
  point. Sell signals always close the full position for the symbol.

  - [`BuyAndHold`] is the only exception: it buys once on each symbol's first
    available bar and never re-sizes.
  - [`TurtleTrading`] sizes by **risk parity** instead: the per-trade quantity
    is `(risk_per_trade × equity) / (ATR × price_per_unit)`, capping
    volatile instruments and scaling up calmer ones to the same dollar risk.

- **Portfolio-rotation strategies.** On each rebalance, the strategy ranks the
  universe, picks the top `K` symbols, and targets an equal-weight allocation
  of `equity / K` per slot. Existing positions outside the new top-`K` are
  fully liquidated; remaining positions are resized up or down to match the
  new target weight, so the portfolio is always close to fully invested across
  the current `K` winners.

In every case, if the next-bar fill price plus slippage and commission would
push an order over the available cash, the engine **auto-shrinks** the
quantity to whatever fits rather than rejecting it outright. This keeps
equal-weight strategies from silently dropping their last leg when fees nibble
into the budget.

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
| [`TurtleTrading`] | Trend | Breakout-based trend-following with ATR position sizing.           |
| [`Vcp`] | Breakout | Volatility Contraction Pattern breakout.                           |

### Portfolio-rotation strategies

| Strategy | Description                                                    |
|----------|----------------------------------------------------------------|
| [`MultiBollingerRotation`] | Rotates into stocks crossing above their upper Bollinger Band. |
| [`RocRotation`] | Rotates into the top K stocks by Rate of Change.               |
| [`RsrsRotation`] | Rotates into stocks with highest RSRS values.                  |
| [`TripleRsiRotation`] | Rotates based on composite long/medium/short RSI scores.       |
