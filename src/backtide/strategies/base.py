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
        indicators: dict[
            str,
            dict[str, pd.Series | pd.DataFrame | pl.Series | pl.DataFrame],
        ]
        | None,
    ) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, pandas.DataFrame | polars.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar. For example,
            `data["AAPL"]["close"]` selects AAPL's visible close-price history.

        portfolio : [backtide.backtest.Portfolio][portfolio]
            Current portfolio holdings (cash, positions and open orders). For
            example, `portfolio.positions.get("AAPL", 0.0)` returns the current
            signed quantity, while `portfolio.orders` contains pending orders.

        state : [backtide.backtest.State][state]
            Current simulation state. For example, use `state.is_warmup` to
            suppress orders during warmup and `state.datetime` to read the
            current bar's timezone-aware timestamp.

        indicators : dict[str,dict[str,pd.Series | pd.DataFrame | pl.Series | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            histories available up to the current bar. For example,
            `indicators["SMA_20"]["AAPL"]` selects AAPL's visible 20-bar SMA
            history. `None` is permitted when no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
        ...
