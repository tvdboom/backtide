# Strategy examples
-------------------

These complete custom strategies can be pasted into Backtide's strategy editor
or saved in a `.py` file. The final expression instantiates the class, which is
required when loading custom strategy source through the application.

Both examples read price and indicator series through a small helper so they
work with either the pandas or Polars dataframe backend.

## Daily two-percent reversal

This long-only strategy buys after a daily fall of at least two percent and
closes the position after a daily rise of at least two percent. It ignores
warmup bars, invalid prices and symbols that already have a pending order.

```python
from math import isfinite

from backtide.backtest import Order
from backtide.strategies import BaseStrategy


class DailyTwoPercentReversal(BaseStrategy):
    """Buy after a 2% daily fall and sell after a 2% daily rise."""

    def __init__(self, quantity=100, threshold=0.02):
        self.quantity = quantity
        self.threshold = threshold

    @staticmethod
    def _close_at(frame, index):
        """Read a close value from either pandas or Polars."""
        closes = frame["close"]
        if hasattr(closes, "iloc"):
            return float(closes.iloc[index])
        return float(closes[index])

    def evaluate(self, data, portfolio, state, indicators):
        orders = []

        if state.is_warmup:
            return orders

        pending_symbols = {order.symbol for order in portfolio.orders}

        for symbol, frame in data.items():
            if len(frame) < 2 or symbol in pending_symbols:
                continue

            previous_close = self._close_at(frame, -2)
            current_close = self._close_at(frame, -1)

            if (
                not isfinite(previous_close)
                or not isfinite(current_close)
                or previous_close <= 0
            ):
                continue

            daily_change = current_close / previous_close - 1
            current_quantity = portfolio.positions.get(symbol, 0)

            # Sell the complete position after a daily increase of at least 2%.
            if current_quantity > 0 and daily_change >= self.threshold:
                orders.append(
                    Order(
                        symbol=symbol,
                        order_type="market",
                        quantity=-current_quantity,
                    )
                )

            # Buy 100 shares after a daily decrease of at least 2%.
            elif current_quantity == 0 and daily_change <= -self.threshold:
                orders.append(
                    Order(
                        symbol=symbol,
                        order_type="market",
                        quantity=self.quantity,
                    )
                )

        return orders


DailyTwoPercentReversal()
```

## Moving-average trend

This strategy declares its moving average in `required_indicators()`. Backtide
computes it once, injects it under the deterministic name `SMA_50`, and passes
only values through the current bar to `evaluate()`.

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
        """Declare indicators that Backtide should pre-compute."""
        return [SimpleMovingAverage(self.period)]

    @staticmethod
    def _last(values):
        """Read the latest value from either pandas or Polars."""
        if hasattr(values, "iloc"):
            return float(values.iloc[-1])
        return float(values[-1])

    def evaluate(self, data, portfolio, state, indicators):
        orders = []

        if state.is_warmup or indicators is None:
            return orders

        averages = indicators.get(f"SMA_{self.period}", {})
        pending_symbols = {order.symbol for order in portfolio.orders}

        for symbol, frame in data.items():
            moving_average = averages.get(symbol)
            if not len(frame) or moving_average is None or not len(moving_average):
                continue
            if symbol in pending_symbols:
                continue

            close = self._last(frame["close"])
            average = self._last(moving_average)
            if not isfinite(close) or not isfinite(average):
                continue

            current_quantity = portfolio.positions.get(symbol, 0)
            if current_quantity == 0 and close > average:
                orders.append(
                    Order(
                        symbol=symbol,
                        order_type="market",
                        quantity=self.quantity,
                    )
                )
            elif current_quantity > 0 and close < average:
                orders.append(
                    Order(
                        symbol=symbol,
                        order_type="market",
                        quantity=-current_quantity,
                    )
                )

        return orders


MovingAverageTrend()
```
