# Ulcer-index metric

The ulcer index measures the depth and duration of drawdowns through their root mean square. Lower
values are better.

```python
import numpy as np

from backtide.metrics import BaseMetric


class UlcerIndex(BaseMetric):
    """Return the root mean square percentage drawdown."""

    percentage = True
    higher_is_better = False

    def compute(self, equity_curve, trades):
        del trades
        equity = np.asarray(equity_curve["equity"], dtype=float)
        if equity.size == 0:
            return 0.0
        peaks = np.maximum.accumulate(equity)
        drawdowns = np.divide(equity, peaks, out=np.ones_like(equity), where=peaks != 0) - 1.0
        return float(np.sqrt(np.mean(drawdowns**2)))


UlcerIndex()
```

The returned fraction is displayed as a percentage because `percentage` is `True`.
