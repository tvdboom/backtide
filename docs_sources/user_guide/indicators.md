# Indicators
-----------

Indicators are mathematical calculations applied to historical price and volume
data. They help traders and analysts identify trends, momentum, volatility and
potential reversal points. Backtide provides a set of built-in indicators
implemented in Rust for maximum performance, as well as a framework for creating
custom indicators in Python.

<br>

## How they work

Every indicator inherits from `BaseIndicator` and implements a `compute` method
that receives an OHLCV dataframe and returns one or more series of computed values:

- **Single-output** indicators (e.g., SMA, RSI) return one column — typically
  plotted as a line overlay on the price chart.
- **Multi-output** indicators (e.g., Bollinger Bands, MACD) return two or
  more columns — plotted as bands, dual lines, or signal pairs.

When running a backtest, indicators listed in the experiment configuration are
computed up front over the entire price history before the simulation begins.
The values are then passed to the strategy function through its `indicators`
parameter on every bar, so the strategy can use them to make investment decisions
without recomputing anything. Only values up to the current bar's timestamp are
made available — no future information is leaked, ensuring the backtest remains
free of lookahead bias.

```python
from backtide.indicators import SimpleMovingAverage

sma = SimpleMovingAverage(period=20)
result = sma.compute(df)  # Returns a single-column result
```

<br>

## Custom indicators

You can create your own indicators by subclassing `BaseIndicator`. Custom
indicators can be written directly in the [application's][application] code
editor or uploaded as `.py` files.

```python
from backtide.indicators import BaseIndicator


class MyIndicator(BaseIndicator):
    def compute(self, data):
        return data[["close"]].rolling(10).mean()


MyIndicator()
```

<br>

## Built-in indicators

All built-in indicators are implemented in Rust and exposed to Python. They
accept OHLCV data in any configured [`DataFrameLibrary`] format and return
results in that same format. See the API reference for full details on each
indicator's parameters, attributes, and formulas.

| Indicator | Acronym | Category | Description                                     |
|-----------|---------|----------|-------------------------------------------------|
| [`AverageDirectionalIndex`] | ADX | Trend | Trend strength (0–100) regardless of direction. |
| [`AverageTrueRange`] | ATR | Volatility | Average of the true range over a period.        |
| [`BollingerBands`] | BB | Volatility | Volatility bands around an SMA.                 |
| [`CommodityChannelIndex`] | CCI | Momentum | Deviation of typical price from its mean.       |
| [`ExponentialMovingAverage`] | EMA | Trend | Exponentially weighted moving average.          |
| [`MovingAverageConvergenceDivergence`] | MACD | Momentum | Trend-following momentum from two EMAs.         |
| [`OnBalanceVolume`] | OBV | Volume | Cumulative volume confirming price trends.      |
| [`RelativeStrengthIndex`] | RSI | Momentum | Overbought/oversold oscillator (0–100).         |
| [`SimpleMovingAverage`] | SMA | Trend | Arithmetic mean of the last N closing prices.   |
| [`StochasticOscillator`] | STOCH | Momentum | Closing price relative to high-low range.       |
| [`VolumeWeightedAveragePrice`] | VWAP | Volume | Cumulative average price weighted by volume.    |
| [`WeightedMovingAverage`] | WMA | Trend | Linearly weighted moving average.               |
