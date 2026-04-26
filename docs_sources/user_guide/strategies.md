# Strategies
-----------

Strategies are the decision-making logic that determines when to buy, sell, or
hold positions during a backtest. Each strategy receives market data, portfolio
state, and pre-computed indicator values, and returns a list of orders to
execute. Backtide provides a set of built-in strategies implemented in Rust
for maximum performance, as well as a framework for creating custom strategies
in Python.

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
- `state` — the current portfolio state (positions, cash, etc.).
- `indicators` — pre-computed indicator values keyed by symbol and name
  (only values up to the current bar are available — no lookahead bias).

```python
from backtide.strategies import SmaCrossover

strategy = SmaCrossover(fast_period=20, slow_period=50)
orders = strategy.evaluate(data, state, indicators)
```

<br>

## Custom strategies

You can create your own strategies by subclassing `BaseStrategy`. Custom
strategies can be written directly in the [application's][application] code
editor or uploaded as `.py` files.

```python
from backtide.strategies import BaseStrategy


class MyStrategy(BaseStrategy):
    def evaluate(self, data, state, indicators):
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

### Single-asset strategies

| Strategy | Category | Description |
|----------|----------|-------------|
| [`AdaptiveRsi`] | Momentum | RSI with dynamic period (8–28) adapting to volatility |
| [`AlphaRsiPro`] | Momentum | Advanced RSI with adaptive levels and trend bias filtering |
| [`BollingerMeanReversion`] | Mean Reversion | Buys at the lower band, sells at the upper band |
| [`BuyAndHold`] | Baseline | Buys on the first day and holds to the end |
| [`DoubleTop`] | Pattern | Buys on breakout after a double-top pattern |
| [`HybridAlphaRsi`] | Momentum | Combines adaptive period, adaptive levels, and trend confirmation |
| [`Macd`] | Trend | Buys on MACD golden cross, sells on death cross |
| [`Momentum`] | Trend | Buys when momentum turns positive, exits on MA filter |
| [`RiskAverse`] | Breakout | Buys low-volatility stocks making new highs on volume |
| [`Roc`] | Momentum | Buys on high Rate of Change, sells on low |
| [`Rsi`] | Momentum | Combines RSI and Bollinger Bands for dual confirmation |
| [`Rsrs`] | Trend | Uses regression of high/low prices for support detection |
| [`SmaCrossover`] | Trend | Golden cross / death cross with two moving averages |
| [`SmaNaive`] | Trend | Buys above MA, sells below |
| [`TurtleTrading`] | Trend | Breakout-based trend-following with ATR position sizing |
| [`Vcp`] | Breakout | Volatility Contraction Pattern breakout |

### Portfolio-rotation strategies

| Strategy | Description |
|----------|-------------|
| [`MultiBollingerRotation`] | Rotates into stocks crossing above their upper Bollinger Band |
| [`RocRotation`] | Rotates into the top K stocks by Rate of Change |
| [`RsrsRotation`] | Rotates into stocks with highest RSRS values |
| [`TripleRsiRotation`] | Rotates based on composite long/medium/short RSI scores |
