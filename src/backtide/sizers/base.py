"""Backtide.

Author: Mavs
Description: Abstract base class for sizers.

"""

from __future__ import annotations

from abc import ABC, abstractmethod


class BaseSizer(ABC):
    """Abstract base class for all position sizers.

    Subclass it to create a custom sizer.

    Examples
    --------
    ```python
    from backtide.sizers import BaseSizer

    class MySizer(BaseSizer):
        def __init__(self, param1=0.02):
            self.param1 = param1

        def calculate(self, equity, price, stop_distance=None, atr=None):
            # Your sizing logic here
            return quantity
    ```
    """

    @abstractmethod
    def calculate(
        self,
        equity: float,
        price: float,
        stop_distance: float | None = None,
        atr: float | None = None,
    ) -> float:
        """Calculate the quantity to trade.

        Parameters
        ----------
        equity : float
            Current portfolio equity in the same currency as ``price``. When a
            sizer is attached to an order, the engine passes equity converted
            to that instrument's quote currency.

        price : float
            Current market price of the instrument.

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
            If required parameters are missing or invalid.

        """
        ...
