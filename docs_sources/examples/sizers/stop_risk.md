# Stop-risk sizer

This sizer divides a fixed risk budget by the distance between entry and stop. It rejects calls
that omit the required distance.

```python
from math import isfinite

from backtide.sizers import BaseSizer


class StopRiskSizer(BaseSizer):
    """Risk a fixed fraction of equity at the stop price."""

    def __init__(self, risk_fraction=0.01):
        self.risk_fraction = risk_fraction

    def calculate(self, equity, price, stop_distance=None, atr=None):
        del atr
        if not isfinite(equity) or not isfinite(price) or equity <= 0 or price <= 0:
            return 0.0
        if stop_distance is None or not isfinite(stop_distance) or stop_distance <= 0:
            raise ValueError("stop_distance must be finite and positive")
        return equity * self.risk_fraction / stop_distance


StopRiskSizer(risk_fraction=0.01)
```

When attached to an order with a stop price, Backtide passes the absolute entry-to-stop distance as
`stop_distance`.
