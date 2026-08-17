# Indicator examples
--------------------

These complete custom indicators can be pasted into Backtide's indicator editor
or saved in a `.py` file. The final expression instantiates the class, which is
required when loading custom indicator source through the application.

Each implementation works with either the pandas or Polars dataframe backend
and returns one series with the same row order as the input OHLCV data.

## Typical price

Typical price averages each bar's high, low and close. It is a useful building
block for indicators that need a less noisy price input than the close alone.

```python
from backtide.indicators import BaseIndicator


class TypicalPrice(BaseIndicator):
    """Calculate the average of each bar's high, low and close."""

    acronym = "TYP"

    def compute(self, data):
        high = data["high"]
        low = data["low"]
        close = data["close"]
        return (high + low + close) / 3.0


TypicalPrice()
```

## Rolling price range

This parameterized indicator measures the distance between the highest high and
lowest low in a rolling window. The initial `period - 1` values are missing
because a complete window is not available yet.

```python
from backtide.indicators import BaseIndicator


class RollingPriceRange(BaseIndicator):
    """Calculate the rolling highest-high minus lowest-low range."""

    acronym = "RANGE"

    def __init__(self, period=20):
        self.period = period

    def compute(self, data):
        high = data["high"]
        low = data["low"]

        if hasattr(high, "rolling"):
            highest = high.rolling(self.period).max()
            lowest = low.rolling(self.period).min()
        else:
            highest = high.rolling_max(window_size=self.period)
            lowest = low.rolling_min(window_size=self.period)

        return highest - lowest


RollingPriceRange(period=20)
```
