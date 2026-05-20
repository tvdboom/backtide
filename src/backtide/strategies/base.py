"""Backtide.

Author: Mavs
Description: Abstract base class for strategies.

"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import pandas as pd
    import polars as pl

    from backtide.backtest import Order, Portfolio, State


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

        def evaluate(self, data, portfolio, state, indicators):
            orders = []
            # Your logic here ...
            return orders
    ```

    """

    @staticmethod
    def log(message: str, level: str = "info"):
        """Write a message to the experiment log.

        Messages appear in the live log viewer while the experiment
        runs and are persisted to the experiment's ``logs.txt`` file.

        Parameters
        ----------
        message : str
            The message to log.

        level : str | [LogLevel], default="info"
            Tracing log level. Choose from: "error", "warn", "info", "debug".

        Examples
        --------
        ```python
        def evaluate(self, data, portfolio, state, indicators):
            self.log(f"Bar {state.bar_index}: evaluating...")
            ...
        ```

        """
        from backtide.backtest import experiment_log

        experiment_log(message, level)

    @abstractmethod
    def evaluate(
        self,
        data: dict[str, pd.DataFrame | pl.DataFrame],
        portfolio: Portfolio,
        state: State,
        indicators: dict[str, dict[str, pd.DataFrame | pl.DataFrame]] | None,
    ) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
        ...
