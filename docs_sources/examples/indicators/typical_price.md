# Typical price indicator

Typical price averages each bar's high, low, and close. The arithmetic works unchanged with pandas
and Polars series.

```python
from backtide.indicators import BaseIndicator


class TypicalPrice(BaseIndicator):
    """Calculate the average of each bar's high, low, and close."""

    acronym = "TYP"

    def compute(self, data):
        high = data["high"]
        low = data["low"]
        close = data["close"]
        return (high + low + close) / 3.0


TypicalPrice()
```

`data` is the complete OHLCV dataframe. The returned series has one value per input row.
