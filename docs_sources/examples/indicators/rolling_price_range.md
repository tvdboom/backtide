# Rolling price-range indicator

This parameterized indicator measures the difference between the highest high and lowest low in a
rolling window. The initial `period - 1` values are missing because the window is incomplete.

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
