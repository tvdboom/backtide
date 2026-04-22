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
results in that same format.

<br>

### Simple Moving Average (SMA)

The arithmetic mean of the last $n$ closing prices. Used to smooth short-term
fluctuations and identify the direction of a trend.

**When to use:** Trend identification, support/resistance levels, crossover
strategies (e.g., golden cross / death cross).

$$
\text{SMA}_t = \frac{1}{n} \sum_{i=0}^{n-1} C_{t-i}
$$

where $C_t$ is the closing price at time $t$ and $n$ is the period.

| Parameter | Default | Description |
|-----------|---------|-------------|
| `period` | 14 | Number of bars in the window |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Moving_average#Simple_moving_average)

<br>

### Exponential Moving Average (EMA)

A weighted moving average that gives exponentially more weight to recent prices,
making it more responsive to new information than the SMA.

**When to use:** Faster trend detection, reducing lag in crossover systems,
building block for other indicators (MACD, ADX).

$$
\text{EMA}_t = \alpha \cdot C_t + (1 - \alpha) \cdot \text{EMA}_{t-1}
\qquad \text{where} \quad \alpha = \frac{2}{n + 1}
$$

| Parameter | Default | Description |
|-----------|---------|-------------|
| `period` | 14 | Number of bars in the window |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Moving_average#Exponential_moving_average)

<br>

### Weighted Moving Average (WMA)

A moving average where each price is multiplied by a linearly decreasing weight,
placing more emphasis on recent data than the SMA but with a different weighting
scheme than the EMA.

**When to use:** Similar to EMA but with a linear instead of exponential decay
— useful when you want recent prices to matter more without the recursive smoothing
of EMA.

$$
\text{WMA}_t = \frac{\sum_{i=0}^{n-1} (n - i) \cdot C_{t-i}}{\sum_{i=1}^{n} i}
$$

| Parameter | Default | Description |
|-----------|---------|-------------|
| `period` | 14 | Number of bars in the window |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Moving_average#Weighted_moving_average)

<br>

### Relative Strength Index (RSI)

A momentum oscillator that measures the speed and magnitude of recent price changes
on a scale of 0 to 100. Values above 70 are typically considered overbought; below
30, oversold.

**When to use:** Identifying overbought/oversold conditions, spotting divergences,
confirming trend strength.

$$
\text{RSI} = 100 - \frac{100}{1 + RS}
\qquad \text{where} \quad RS = \frac{\text{avg gain over } n}{\text{avg loss over } n}
$$

| Parameter | Default | Description |
|-----------|---------|-------------|
| `period` | 14 | Lookback period |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Relative_strength_index)

<br>

### Moving Average Convergence Divergence (MACD)

A trend-following momentum indicator that shows the relationship between two EMAs.
The MACD line is the difference between a fast and slow EMA; the signal line is an
EMA of the MACD line itself.

**When to use:** Trend direction and momentum, signal line crossovers for entry/exit
timing, histogram divergence analysis.

$$
\text{MACD}_t = \text{EMA}_{\text{fast}}(C_t) - \text{EMA}_{\text{slow}}(C_t)
$$

$$
\text{Signal}_t = \text{EMA}_{\text{signal}}(\text{MACD}_t)
$$

| Parameter | Default | Description |
|-----------|---------|-------------|
| `fast_period` | 12 | Fast EMA period |
| `slow_period` | 26 | Slow EMA period |
| `signal_period` | 9 | Signal line EMA period |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/MACD)

<br>

### Bollinger Bands (BB)

Volatility bands placed $k$ standard deviations above and below an $n$-period SMA.
The bands widen during high volatility and contract during low volatility.

**When to use:** Volatility assessment, mean-reversion strategies, breakout
detection when price moves outside the bands.

$$
\text{Upper}_t = \text{SMA}_t + k \cdot \sigma_t
$$

$$
\text{Lower}_t = \text{SMA}_t - k \cdot \sigma_t
$$

where $\sigma_t$ is the rolling standard deviation over $n$ periods.

| Parameter | Default | Description |
|-----------|---------|-------------|
| `period` | 20 | Number of bars for the moving average |
| `std_dev` | 2.0 | Number of standard deviations |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Bollinger_Bands)

<br>

### Average True Range (ATR)

Measures market volatility by calculating the average of the true range over a
period. The true range accounts for gaps between sessions.

**When to use:** Position sizing, setting stop-loss levels, comparing volatility
across instruments.

$$
\text{TR}_t = \max\!\big(H_t - L_t,\; |H_t - C_{t-1}|,\; |L_t - C_{t-1}|\big)
$$

$$
\text{ATR}_t = \frac{1}{n} \sum_{i=0}^{n-1} \text{TR}_{t-i}
$$

| Parameter | Default | Description |
|-----------|---------|-------------|
| `period` | 14 | Lookback period |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Average_true_range)

<br>

### Average Directional Index (ADX)

Quantifies trend strength on a scale of 0 to 100, regardless of direction. Values
above 25 generally indicate a strong trend; below 20, a weak or ranging market.

**When to use:** Determining whether a market is trending or ranging before
applying trend-following or mean-reversion strategies.

$$
+DI_t = 100 \cdot \frac{\text{Smoothed } +DM_t}{\text{ATR}_t}
\qquad
-DI_t = 100 \cdot \frac{\text{Smoothed } -DM_t}{\text{ATR}_t}
$$

$$
DX_t = 100 \cdot \frac{|+DI_t - {-DI_t}|}{+DI_t + {-DI_t}}
\qquad
\text{ADX}_t = \text{EMA}_n(DX_t)
$$

| Parameter | Default | Description |
|-----------|---------|-------------|
| `period` | 14 | Lookback period |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Average_directional_movement_index)

<br>

### Commodity Channel Index (CCI)

Measures how far the typical price deviates from its statistical mean, identifying
cyclical trends. Values above +100 suggest overbought conditions; below −100, oversold.

**When to use:** Identifying cyclical price patterns, spotting divergences, timing
entries in commodities and equities.

$$
\text{TP}_t = \frac{H_t + L_t + C_t}{3}
$$

$$
\text{CCI}_t = \frac{\text{TP}_t - \text{SMA}_n(\text{TP}_t)}{0.015 \cdot \text{MD}_t}
$$

where $\text{MD}_t$ is the mean absolute deviation of $\text{TP} over $n$ periods.

| Parameter | Default | Description |
|-----------|---------|-------------|
| `period` | 20 | Lookback period |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Commodity_channel_index)

<br>

### Stochastic Oscillator (STOCH)

Compares the closing price to the high-low range over a period, producing
a %K line and a smoothed %D signal line. Both oscillate between 0 and 100.

**When to use:** Overbought/oversold signals, %K/%D crossovers for
entry/exit timing, divergence analysis.

$$
\%K_t = 100 \cdot \frac{C_t - L_n}{H_n - L_n}
$$

$$
\%D_t = \text{SMA}_d(\%K_t)
$$

where $H_n$ and $L_n$ are the highest high and lowest low over $n$ periods.

| Parameter | Default | Description |
|-----------|---------|-------------|
| `k_period` | 14 | %K lookback period |
| `d_period` | 3 | %D smoothing period |

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Stochastic_oscillator)

<br>

### On-Balance Volume (OBV)

A cumulative volume indicator that adds volume on up-close days and subtracts it
on down-close days. Rising OBV confirms an uptrend; falling OBV confirms a downtrend.

**When to use:** Confirming price trends with volume, spotting divergences between
price and volume momentum.

$$
\text{OBV}_t = \text{OBV}_{t-1} +
\begin{cases}
V_t & \text{if } C_t > C_{t-1} \\
-V_t & \text{if } C_t < C_{t-1} \\
0 & \text{otherwise}
\end{cases}
$$

:material-link: [Wikipedia](https://en.wikipedia.org/wiki/On-balance_volume)

<br>

### Volume-Weighted Average Price (VWAP)

The cumulative average price weighted by volume. Institutional traders use VWAP
as a benchmark: buying below VWAP is considered favorable, selling above it likewise.

**When to use:** Intraday trading benchmark, assessing execution quality, dynamic
support/resistance.

$$
\text{VWAP}_t = \frac{\sum_{i=1}^{t} \text{TP}_i \cdot V_i}{\sum_{i=1}^{t} V_i}
\qquad \text{where} \quad \text{TP}_i = \frac{H_i + L_i + C_i}{3}
$$


:material-link: [Wikipedia](https://en.wikipedia.org/wiki/Volume-weighted_average_price)
