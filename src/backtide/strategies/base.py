"""Backtide.

Author: Mavs
Description: Abstract base class for strategies.

"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from backtide.backtest import Order, State


class BaseStrategy(ABC):
    """Abstract base class for all strategies.

    Subclass it to create a custom strategy.

    Examples
    --------
    ```python
    from backtide.strategies import BaseStrategy

    class MyStrategy(BaseStrategy):
        def __init__(self, threshold=0.02):
            self.threshold = threshold

        def evaluate(self, data, state, indicators):
            orders = []
            # Your logic here ...
            return orders
    ```

    """

    @abstractmethod
    def evaluate(
        self,
        data: Any,
        state: State,
        indicators: Any,
    ) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : np.array | pd.DataFrame | pl.DataFrame
            Historical OHLCV data available up to the current bar.

        state : State
            Current simulation state (portfolio, timestamp).

        indicators : np.array | pd.DataFrame | pl.DataFrame | None
            Pre-computed indicator values. None if no indicators were
            selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
        ...
