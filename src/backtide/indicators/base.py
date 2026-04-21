"""Backtide.

Author: Mavs
Description: Abstract base class for indicators.

"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING


if TYPE_CHECKING:
    import numpy as np
    import pandas as pd
    import polars as pl


class BaseIndicator(ABC):
    """Abstract base class for all indicators.

    Subclass this to create a custom indicator.

    Examples
    --------
    ```python
    from backtide.indicators import BaseIndicator

    class MyMomentum(BaseIndicator):
        def __init__(self, period: int = 10):
            self.period = period

        def compute(self, data):
            return data["close"].diff(self.period)
    ```

    """

    @abstractmethod
    def compute(self, data: np.ndarray | pd.DataFrame | pl.DataFrame) -> Any:
        """Compute the indicator values.

        Parameters
        ----------
        data : np.array | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pd.DataFrame | pl.Series | pl.DataFrame
            Single series for one-output indicators, or 2d for multi-output
            indicators (e.g., Bollinger Bands upper/lower).

        """
        ...
