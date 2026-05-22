"""Type stubs for `backtide.core.sizers` (auto-generated)."""

__all__ = [
    "EqualWeight",
    "FixedFractional",
    "FixedNotional",
    "FixedQuantity",
    "KellyCriterion",
    "RiskBased",
    "VolatilityScaled",
]

from typing import Any

class EqualWeight:
    """Divide current equity equally across a fixed number of positions.

    Computes `quantity = (equity / n_positions) / price`. Useful for
    portfolio-level rotation strategies where every selected symbol gets
    the same allocation regardless of price or volatility.

    Parameters
    ----------
    n_positions : int
        Number of concurrent positions to split the equity across.

    See Also
    --------
    - backtide.sizers:BaseSizer
    - backtide.sizers:FixedFractional
    - backtide.sizers:FixedNotional

    """

    n_positions: Any

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def calculate(self, equity, price, stop_distance=None, atr=None) -> int | float:
        """Calculate the order quantity for this sizer.

        Parameters
        ----------
        equity : float
            Current portfolio equity in the same currency as `price`.
            When a sizer is attached to an order, the engine passes
            equity converted to that instrument's quote currency.

        price : float
            Reference price of the instrument.

        stop_distance : float | None, default=None
            Distance from entry to stop loss, in price units.

        atr : float | None, default=None
            Current ATR value. Required for volatility-based sizers.

        Returns
        -------
        int | float
            The number of units to trade. Positive for buys, negative for sells.

        Raises
        ------
        ValueError
            If a required input is missing or invalid.

        """

class FixedFractional:
    """Allocate a fixed percentage of total current equity.

    Computes `quantity = (equity * fraction) / price`. The position size
    scales with the portfolio: as equity grows, allocations grow, and
    vice versa. This is the most common sizing rule.

    Parameters
    ----------
    fraction : float
        Fraction of equity to allocate per trade. Must be in `(0, 1]`.

    See Also
    --------
    - backtide.sizers:BaseSizer
    - backtide.sizers:EqualWeight
    - backtide.sizers:FixedNotional

    """

    fraction: Any

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def calculate(self, equity, price, stop_distance=None, atr=None) -> int | float:
        """Calculate the order quantity for this sizer.

        Parameters
        ----------
        equity : float
            Current portfolio equity in the same currency as `price`.
            When a sizer is attached to an order, the engine passes
            equity converted to that instrument's quote currency.

        price : float
            Reference price of the instrument.

        stop_distance : float | None, default=None
            Distance from entry to stop loss, in price units.

        atr : float | None, default=None
            Current ATR value. Required for volatility-based sizers.

        Returns
        -------
        int | float
            The number of units to trade. Positive for buys, negative for sells.

        Raises
        ------
        ValueError
            If a required input is missing or invalid.

        """

class FixedNotional:
    """Buy a fixed amount of currency worth of the asset.

    Computes `quantity = amount / price`. Keeps cash exposure consistent
    across symbols regardless of price level, but ignores portfolio size.

    Parameters
    ----------
    amount : float
        Cash amount to spend per trade, in the instrument's quote currency.

    See Also
    --------
    - backtide.sizers:BaseSizer
    - backtide.sizers:FixedFractional
    - backtide.sizers:FixedQuantity

    """

    amount: Any

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def calculate(self, equity, price, stop_distance=None, atr=None) -> int | float:
        """Calculate the order quantity for this sizer.

        Parameters
        ----------
        equity : float
            Current portfolio equity in the same currency as `price`.
            When a sizer is attached to an order, the engine passes
            equity converted to that instrument's quote currency.

        price : float
            Reference price of the instrument.

        stop_distance : float | None, default=None
            Distance from entry to stop loss, in price units.

        atr : float | None, default=None
            Current ATR value. Required for volatility-based sizers.

        Returns
        -------
        int | float
            The number of units to trade. Positive for buys, negative for sells.

        Raises
        ------
        ValueError
            If a required input is missing or invalid.

        """

class FixedQuantity:
    """Buy exactly N units.

    Returns the configured `quantity` regardless of price or equity. Simple,
    price-naive sizing — appropriate for crypto base units or quick prototyping.

    Parameters
    ----------
    quantity : int | float
        The exact number of units to trade per order.

    See Also
    --------
    - backtide.sizers:BaseSizer
    - backtide.sizers:FixedFractional
    - backtide.sizers:FixedNotional

    """

    quantity: Any

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def calculate(self, equity, price, stop_distance=None, atr=None) -> int | float:
        """Calculate the order quantity for this sizer.

        Parameters
        ----------
        equity : float
            Current portfolio equity in the same currency as `price`.
            When a sizer is attached to an order, the engine passes
            equity converted to that instrument's quote currency.

        price : float
            Reference price of the instrument.

        stop_distance : float | None, default=None
            Distance from entry to stop loss, in price units.

        atr : float | None, default=None
            Current ATR value. Required for volatility-based sizers.

        Returns
        -------
        int | float
            The number of units to trade. Positive for buys, negative for sells.

        Raises
        ------
        ValueError
            If a required input is missing or invalid.

        """

class KellyCriterion:
    """Kelly Criterion sizing.

    Computes the theoretically optimal fraction of capital to risk for long-run
    geometric growth: `kelly_pct = win_rate - ((1 - win_rate) / (avg_win / avg_loss))`,
    then `quantity = (equity * kelly_pct * fraction) / price`. The `fraction`
    multiplier (e.g., 0.25 for "quarter Kelly") tames drawdowns at the cost of
    slower growth.

    Parameters
    ----------
    win_rate : float
        Historical fraction of winning trades, in `[0, 1]`.

    avg_win : float
        Average profit of winning trades. Must be positive.

    avg_loss : float
        Average loss of losing trades, expressed as a positive number.

    fraction : float
        Kelly multiplier (typically 0.25–0.5 for half/quarter Kelly).

    See Also
    --------
    - backtide.sizers:BaseSizer
    - backtide.sizers:FixedFractional
    - backtide.sizers:RiskBased

    """

    avg_loss: Any
    avg_win: Any
    fraction: Any
    win_rate: Any

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def calculate(self, equity, price, stop_distance=None, atr=None) -> int | float:
        """Calculate the order quantity for this sizer.

        Parameters
        ----------
        equity : float
            Current portfolio equity in the same currency as `price`.
            When a sizer is attached to an order, the engine passes
            equity converted to that instrument's quote currency.

        price : float
            Reference price of the instrument.

        stop_distance : float | None, default=None
            Distance from entry to stop loss, in price units.

        atr : float | None, default=None
            Current ATR value. Required for volatility-based sizers.

        Returns
        -------
        int | float
            The number of units to trade. Positive for buys, negative for sells.

        Raises
        ------
        ValueError
            If a required input is missing or invalid.

        """

class RiskBased:
    """Size based on acceptable risk and stop loss distance.

    Computes `quantity = (equity * risk_pct) / stop_distance`. Industry standard
    approach: you define how much equity you're willing to lose and the distance
    to your stop, and the sizer works backwards. Requires `stop_distance` to be
    passed to `calculate()`.

    Parameters
    ----------
    risk_pct : float
        Fraction of equity at risk per trade (e.g. `0.01` for 1%).

    See Also
    --------
    - backtide.sizers:BaseSizer
    - backtide.sizers:KellyCriterion
    - backtide.sizers:VolatilityScaled

    """

    risk_pct: Any

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def calculate(self, equity, price, stop_distance=None, atr=None) -> int | float:
        """Calculate the order quantity for this sizer.

        Parameters
        ----------
        equity : float
            Current portfolio equity in the same currency as `price`.
            When a sizer is attached to an order, the engine passes
            equity converted to that instrument's quote currency.

        price : float
            Reference price of the instrument.

        stop_distance : float | None, default=None
            Distance from entry to stop loss, in price units.

        atr : float | None, default=None
            Current ATR value. Required for volatility-based sizers.

        Returns
        -------
        int | float
            The number of units to trade. Positive for buys, negative for sells.

        Raises
        ------
        ValueError
            If a required input is missing or invalid.

        """

class VolatilityScaled:
    """Size based on volatility (ATR).

    Computes `quantity = (equity * risk_pct) / atr`. Like [`RiskBased`] but uses
    the instrument's Average True Range as a proxy for stop distance, automatically
    shrinking positions on volatile assets and growing them on calm ones. Requires
    `atr` to be passed to `calculate()`.

    Parameters
    ----------
    risk_pct : float
        Fraction of equity to risk per trade (e.g., `0.02` for 2%).

    See Also
    --------
    - backtide.sizers:BaseSizer
    - backtide.sizers:FixedFractional
    - backtide.sizers:RiskBased

    """

    risk_pct: Any

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def calculate(self, equity, price, stop_distance=None, atr=None) -> int | float:
        """Calculate the order quantity for this sizer.

        Parameters
        ----------
        equity : float
            Current portfolio equity in the same currency as `price`.
            When a sizer is attached to an order, the engine passes
            equity converted to that instrument's quote currency.

        price : float
            Reference price of the instrument.

        stop_distance : float | None, default=None
            Distance from entry to stop loss, in price units.

        atr : float | None, default=None
            Current ATR value. Required for volatility-based sizers.

        Returns
        -------
        int | float
            The number of units to trade. Positive for buys, negative for sells.

        Raises
        ------
        ValueError
            If a required input is missing or invalid.

        """
