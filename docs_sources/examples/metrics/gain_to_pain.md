# Gain-to-pain metric

Gain to pain divides gross winning trade PnL by absolute gross losing trade PnL.

```python
from backtide.metrics import BaseMetric


class GainToPain(BaseMetric):
    """Return gross winning PnL divided by gross losing PnL."""

    percentage = False
    greater_is_better = True

    def compute(self, equity_curve, trades):
        del equity_curve
        pnl = trades["pnl"]
        gains = pnl[pnl > 0].sum()
        losses = abs(pnl[pnl < 0].sum())
        return float(gains / losses) if losses else 0.0


GainToPain()
```

The result is a ratio, so `percentage` remains `False`.
