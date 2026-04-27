"""Type stubs for `backtide.core.analysis` (auto-generated)."""

__all__ = ["compute_statistics"]

import numpy as np
import pandas as pd
import polars as pl

def compute_statistics(
    data,
    *,
    price_col="adj_close",
    risk_free_rate=0.0,
    periods_per_year=None,
) -> np.ndarray | pd.DataFrame | pl.DataFrame:
    """Compute per-symbol summary statistics.

    Calculates key performance and risk metrics for each symbol in `data`.
    All metrics are annualized based on the detected or specified trading
    frequency.

    Parameters
    ----------
    data : pd.DataFrame | pl.DataFrame
        Input data containing columns `symbol`, the column specified by
        `price_col`, and `dt` with the datetime.

    price_col : str, default="adj_close"
        Column name used to compute returns.

    risk_free_rate : float, default=0.0
        Annualized risk-free rate used in Sharpe and Sortino ratio
        calculations.

    periods_per_year : int | None, default=None
        Number of trading periods per year for annualization. If `None`,
        it is estimated from the median time delta between bars (e.g., 252
        for daily data).

    Returns
    -------
    np.ndarray | pd.DataFrame | pl.DataFrame
        Dataset with one row per symbol and columns for each metric.

    See Also
    --------
    backtide.analysis:plot_returns
    backtide.analysis:plot_drawdown

    Examples
    --------
    ```pycon
    from backtide.storage import query_bars
    from backtide.analysis import compute_statistics

    df = query_bars(["AAPL", "MSFT"], "1d")
    stats = compute_statistics(df)
    print(stats.head())
    ```

    """
