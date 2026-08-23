"""Backtide.

Author: Mavs
Description: Abstract base class for custom experiment metrics.

"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import pandas as pd
    import polars as pl


class BaseMetric(ABC):
    """Abstract base class for custom experiment metrics.

    A metric receives completed, caller-owned result tables and returns one
    finite scalar. Backtide calls it once for every strategy run.

    Attributes
    ----------
    percentage : bool, default=False
        Whether the returned fraction should be displayed as a percentage.

    greater_is_better : bool, default=True
        Whether larger values should rank ahead of smaller values.

    Examples
    --------
    ```python
    from backtide.metrics import BaseMetric

    class GainToPain(BaseMetric):
        '''Return gross gains divided by gross losses.'''

        percentage = False
        greater_is_better = True

        def compute(self, equity_curve, trades):
            pnl = trades["pnl"]
            gains = pnl[pnl > 0].sum()
            losses = abs(pnl[pnl < 0].sum())
            return float(gains / losses) if losses else 0.0
    ```

    """

    percentage = False
    greater_is_better = True
    # Compatibility alias for custom metrics created before ``greater_is_better``.
    higher_is_better = True

    @abstractmethod
    def compute(
        self,
        equity_curve: pd.DataFrame | pl.DataFrame,
        trades: pd.DataFrame | pl.DataFrame,
    ) -> float:
        """Compute one scalar for a completed strategy run.

        Parameters
        ----------
        equity_curve : pd.DataFrame | pl.DataFrame
            Chronological samples with `timestamp`, `equity`, and `drawdown` columns.

        trades : pd.DataFrame | pl.DataFrame
            Completed trades with symbol, quantity, timestamps, prices, and `pnl`.

        Returns
        -------
        float
            Finite metric value.

        """
        ...
