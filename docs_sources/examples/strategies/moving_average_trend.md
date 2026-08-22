# Moving-average trend strategy

This strategy declares its moving average in `required_indicators()`. Backtide computes it once and
passes it to `evaluate()` under the indicator-name-first mapping.

```python
from math import isfinite

from backtide.backtest import Order
from backtide.indicators import SimpleMovingAverage
from backtide.strategies import BaseStrategy


class MovingAverageTrend(BaseStrategy):
    """Buy above a moving average and exit below it."""

    def __init__(self, period=50, quantity=100):
        self.period = period
        self.quantity = quantity

    def required_indicators(self):
        return [SimpleMovingAverage(self.period)]

    @staticmethod
    def _last(values):
        if hasattr(values, "iloc"):
            return float(values.iloc[-1])
        return float(values[-1])

    def evaluate(self, data, portfolio, state, indicators):
        if state.is_warmup or indicators is None:
            return []

        orders = []
        averages = indicators.get(f"SMA_{self.period}", {})
        pending_symbols = {order.symbol for order in portfolio.orders}
        for symbol, frame in data.items():
            average_history = averages.get(symbol)
            if symbol in pending_symbols or average_history is None or not len(average_history):
                continue

            close = self._last(frame["close"])
            average = self._last(average_history)
            if not isfinite(close) or not isfinite(average):
                continue

            quantity = portfolio.positions.get(symbol, 0.0)
            if quantity == 0 and close > average:
                orders.append(
                    Order(symbol=symbol, order_type="market", quantity=self.quantity)
                )
            elif quantity > 0 and close < average:
                orders.append(Order(symbol=symbol, order_type="market", quantity=-quantity))

        return orders


MovingAverageTrend()
```

The lookup is always `indicators[indicator_name][symbol]`; here that is
`indicators["SMA_50"]["AAPL"]` for the default period and symbol.
