Sizers
------

Position sizing determines how many units a strategy buys or sells when it
emits a trading signal. In Backtide, this logic is represented by sizers:
small objects that calculate an order quantity from portfolio equity, price,
and optional risk inputs. Backtide provides a set of built-in sizers as well
as a framework for creating custom sizers.

<br>

## How they work

Every sizer inherits from [`BaseSizer`] and implements a `calculate` method
that receives equity, price, stop_distance and atr, and returns a quantity:

- `equity` — current portfolio equity in the same currency as `price`.
- `price` — current instrument price.
- `stop_distance` — optional distance from entry to stop, in price units.
- `atr` — optional Average True Range value, in price units.

The output is a number of units. Most built-in sizers return a positive entry
quantity; exits are usually submitted with a numeric negative quantity or a
[`SettlePosition`][ordertype] order.

When you attach a sizer directly to an `Order`, the engine resolves it just
before the order is queued. It computes portfolio equity, converts that equity
to the target instrument's quote currency, reads the symbol's latest close, and
then calls `calculate(...)`.

If the portfolio base currency differs from the instrument quote currency, the
engine converts equity first. For example, in a EUR-based portfolio trading a
USD-quoted stock, the sizer receives equity in USD.

!!! note
    If you call `calculate()` manually inside a custom strategy, you are
    responsible for passing the arguments in consistent currencies.

<br>

## Built-in sizers

| Sizer                | Description                                                              |
|----------------------|--------------------------------------------------------------------------|
| [`EqualWeight`]      | Splits equity equally across a fixed number of positions.                |
| [`FixedFractional`]  | Allocates a fixed percentage of current equity per trade.                |
| [`FixedNotional`]    | Spends a fixed cash amount per trade, independent of portfolio size.     |
| [`FixedQuantity`]    | Trades an exact number of units regardless of equity or price.           |
| [`KellyCriterion`]   | Sizes from win rate, average win/loss and a fractional Kelly multiplier. |
| [`RiskBased`]        | Risks a fixed fraction of equity based on distance to a stop level.      |
| [`VolatilityScaled`] | Risks a fixed fraction of equity using ATR as the risk unit.             |

<br>

## Using sizers in custom strategies

Custom strategies can use sizers in two ways.

### Attach a sizer to an order

This is the simplest option. The engine supplies current equity and price, then
resolves the concrete quantity before validation and fill processing.

```python
from backtide.backtest import Order
from backtide.sizers import FixedFractional
from backtide.strategies import BaseStrategy


class FractionalTrend(BaseStrategy):
    def evaluate(self, data, portfolio, state, indicators):
        symbol = "AAPL"
        if symbol not in data:
            return []

        close = data[symbol]["close"].iloc[-1]
        previous = data[symbol]["close"].iloc[-2]
        current_qty = portfolio.positions.get(symbol, 0.0)

        if close > previous and current_qty <= 0:
            return [Order(symbol=symbol, quantity=FixedFractional(0.10))]

        if close < previous and current_qty > 0:
            return [Order(symbol=symbol, quantity=-current_qty)]

        return []
```

For [`RiskBased`], set the order's `price` to your stop level. The engine derives
`stop_distance = abs(current_close - price)` and passes it to the sizer.

```python
from backtide.backtest import Order
from backtide.sizers import RiskBased

entry = Order(
    symbol="AAPL",
    quantity=RiskBased(0.01),
    price=close * 0.95,
)
```

### Calculate the quantity yourself

Use this option when the sizer needs inputs the engine cannot infer, such as
`atr` for [`VolatilityScaled`], or when you need custom rounding before creating
the order.

```python
from math import floor

from backtide.backtest import Order
from backtide.indicators import AverageTrueRange
from backtide.sizers import VolatilityScaled
from backtide.strategies import BaseStrategy


class AtrSizedBreakout(BaseStrategy):
    def required_indicators(self):
        return [AverageTrueRange(14)]

    def evaluate(self, data, portfolio, state, indicators):
        symbol = "AAPL"
        if symbol not in data:
            return []

        close = data[symbol]["close"].iloc[-1]
        atr = indicators["ATR_14"][symbol].iloc[-1]
        if atr <= 0:
            return []

        # Manual examples should keep equity and price in the same currency.
        # For same-currency portfolios, summing cash is often enough for a
        # simple example; production strategies may want full mark-to-market
        # equity including open positions.
        equity = sum(portfolio.cash.values())
        quantity = floor(VolatilityScaled(0.01).calculate(equity=equity, price=close, atr=atr))

        if quantity <= 0:
            return []

        return [Order(symbol=symbol, order_type="market", quantity=quantity)]
```

<br>

## Custom sizers

A custom sizer only needs a `calculate` method. Subclass [`BaseSizer`] for a clear
interface.

```python
from backtide.sizers import BaseSizer


class HalfCashSizer(BaseSizer):
    def calculate(self, equity, price, stop_distance=None, atr=None):
        if equity <= 0 or price <= 0:
            return 0.0
        return (equity * 0.5) / price
```

You can attach it to an order exactly like a built-in sizer:

```python
Order(symbol="AAPL", quantity=HalfCashSizer())
```

In all cases, test sizing parameters carefully. A strategy's entries and exits
determine *when* you trade; the sizer determines *how much* risk each trade adds
to the portfolio.

!!! tip
    Only cryptos accept non-integer quantities. Make sure to return whole units for
    all other instrument types to avoid unexpected sizes.
