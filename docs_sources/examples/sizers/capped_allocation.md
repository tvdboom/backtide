# Capped-allocation sizer

This sizer invests a fraction of equity while limiting the order to a fixed number of units.

```python
from math import isfinite

from backtide.sizers import BaseSizer


class CappedAllocation(BaseSizer):
    """Allocate part of equity without exceeding a unit cap."""

    def __init__(self, fraction=0.10, max_units=1_000):
        self.fraction = fraction
        self.max_units = max_units

    def calculate(self, equity, price, stop_distance=None, atr=None):
        del stop_distance, atr
        if not isfinite(equity) or not isfinite(price) or equity <= 0 or price <= 0:
            return 0.0
        return min(self.max_units, equity * self.fraction / price)


CappedAllocation(fraction=0.10, max_units=1_000)
```

Attach the instance to `Order.quantity`; Backtide supplies current equity and price when resolving
the order.
