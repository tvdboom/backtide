# Daily reversal strategy

This long-only strategy buys after a daily fall of at least two percent and closes the position
after a daily rise of at least two percent. It works with pandas and Polars dataframes.

```python
from math import isfinite

from backtide.backtest import Order
from backtide.strategies import BaseStrategy


class DailyReversal(BaseStrategy):
    """Buy after a large daily fall and sell after a large daily rise."""

    def __init__(self, quantity=100, threshold=0.02):
        self.quantity = quantity
        self.threshold = threshold

    @staticmethod
    def _close_at(frame, index):
        closes = frame["close"]
        if hasattr(closes, "iloc"):
            return float(closes.iloc[index])
        return float(closes[index])

    def evaluate(self, data, portfolio, state, indicators):
        del indicators
        if state.is_warmup:
            return []

        orders = []
        pending_symbols = {order.symbol for order in portfolio.orders}
        for symbol, frame in data.items():
            if len(frame) < 2 or symbol in pending_symbols:
                continue

            previous = self._close_at(frame, -2)
            current = self._close_at(frame, -1)
            if not isfinite(previous) or not isfinite(current) or previous <= 0:
                continue

            change = current / previous - 1.0
            quantity = portfolio.positions.get(symbol, 0.0)
            if quantity > 0 and change >= self.threshold:
                orders.append(Order(symbol=symbol, order_type="market", quantity=-quantity))
            elif quantity == 0 and change <= -self.threshold:
                orders.append(
                    Order(symbol=symbol, order_type="market", quantity=self.quantity)
                )

        return orders


DailyReversal()
```

The final expression is required when the source is loaded through the application library.
