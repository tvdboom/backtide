# Performance

Start with clear, correct custom objects and measure them on representative data. Vectorized
pandas or Polars expressions are usually the best first choice because they already run their
numeric work in optimized code. Avoid Python loops over dataframe rows and avoid converting the
same columns repeatedly.

[Numba](https://numba.readthedocs.io/) can help when a custom indicator or metric needs a numeric
loop that cannot be expressed efficiently with dataframe operations. It is optional and must be
installed separately. Numba compiles NumPy-oriented functions, so keep dataframe conversion and
Backtide objects outside the compiled function. The first call includes compilation time; warm the
function before benchmarking and define it once at module scope.

## Strategy performance

Strategy orchestration stays in Python, but a large numeric decision kernel can be compiled. Pass
plain NumPy arrays into that kernel and construct [`Order`] objects after it returns:

```python
import numpy as np
from numba import njit


@njit
def crossed_above(close, fast, slow):
    if close.size < 2:
        return False
    fast_now = close[-fast:].mean()
    slow_now = close[-slow:].mean()
    fast_before = close[-fast - 1:-1].mean()
    slow_before = close[-slow - 1:-1].mean()
    return fast_before <= slow_before and fast_now > slow_now


# Inside evaluate():
close = np.asarray(data[symbol]["close"], dtype=np.float64)
if crossed_above(close, 20, 50):
    orders.append(Order(symbol=symbol, order_type="market", quantity=100))
```

Do not compile `evaluate()` itself: it receives dataframes and Backtide model objects that Numba
cannot use in nopython mode. Also avoid recomputing rolling indicators in every call; declare them
through `required_indicators()` instead.

## Indicator performance

Indicators process a full history at once and are often the strongest Numba candidates. Return the
same dataframe-family object that the rest of your configuration expects:

```python
import numpy as np
import pandas as pd
import polars as pl
from numba import njit

from backtide.indicators import BaseIndicator


@njit
def rolling_mean(values, period):
    output = np.full(values.size, np.nan)
    for index in range(period - 1, values.size):
        output[index] = values[index - period + 1:index + 1].mean()
    return output


class FastMovingAverage(BaseIndicator):
    def __init__(self, period=20):
        self.period = period

    def compute(self, data):
        values = np.asarray(data["close"], dtype=np.float64)
        output = rolling_mean(values, self.period)
        if isinstance(data, pl.DataFrame):
            return pl.Series(f"fast_ma_{self.period}", output)
        return pd.Series(output, index=data.index, name=f"fast_ma_{self.period}")
```

For standard rolling, expanding, or group operations, benchmark this against native pandas or
Polars first; Numba is not automatically faster after conversion and compilation costs.

## Metric performance

Metrics also receive complete result tables, making array kernels useful for path-dependent
statistics. Keep the public `compute()` method small and return a finite Python `float`:

```python
import numpy as np
from numba import njit

from backtide.metrics import BaseMetric


@njit
def ulcer(values):
    peak = values[0]
    total = 0.0
    for value in values:
        peak = max(peak, value)
        drawdown = value / peak - 1.0
        total += drawdown * drawdown
    return (total / values.size) ** 0.5


class UlcerIndex(BaseMetric):
    greater_is_better = False

    def compute(self, equity_curve, trades):
        del trades
        values = np.asarray(equity_curve["equity"], dtype=np.float64)
        return float(ulcer(values)) if values.size else 0.0
```

## Sizer performance

A sizer normally performs a few scalar operations once per order. Compilation overhead and the
Python boundary usually cost more than the arithmetic, so a direct implementation is preferable:

```python
from backtide.sizers import BaseSizer


class CappedAllocation(BaseSizer):
    def __init__(self, fraction=0.1, cap=1_000):
        self.fraction = fraction
        self.cap = cap

    def calculate(self, equity, price, stop_distance=None, atr=None):
        del stop_distance, atr
        return min(self.cap, equity * self.fraction / price)
```

Optimize a sizer only after profiling shows it matters. Validation, finite-value checks, and clear
units are more important than accelerating a handful of arithmetic operations.
