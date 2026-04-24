"""Backtide.

Author: Mavs
Description: Module containing summary statistics computation for data analysis.

"""

from __future__ import annotations

import numpy as np
import pandas as pd


def compute_summary_stats(
    data: pd.DataFrame,
    price_col: str = "adj_close",
    *,
    risk_free_rate: float = 0.0,
    periods_per_year: int | None = None,
) -> pd.DataFrame:
    """Compute per-symbol summary statistics.

    Calculates key performance and risk metrics for each symbol in the
    dataset. All metrics are annualized based on the detected or
    specified trading frequency.

    Parameters
    ----------
    data : pd.DataFrame
        Input data containing columns `symbol`, the column specified by
        `price_col`, and `dt` with the datetime.

    price_col : str, default="adj_close"
        Column name used to compute returns.

    risk_free_rate : float, default=0.0
        Annualized risk-free rate used in Sharpe and Sortino ratio
        calculations.

    periods_per_year : int | None, default=None
        Number of trading periods per year for annualization. If None,
        it is estimated from the median time delta between bars
        (e.g. 252 for daily data).

    Returns
    -------
    pd.DataFrame
        DataFrame indexed by symbol with columns for each metric.

    """
    records = []

    for symbol in sorted(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        prices = subset[price_col].values
        returns = pd.Series(prices).pct_change().dropna()

        if len(returns) < 2:
            continue

        # Estimate annualization factor
        if periods_per_year is not None:
            ann = periods_per_year
        else:
            dts = subset["dt"].sort_values()
            median_delta = dts.diff().dropna().median()
            seconds = median_delta.total_seconds()
            ann = max(1, int(round(365.25 * 86400 / seconds)))

        # Annualized return (geometric)
        total_return = prices[-1] / prices[0]
        n_years = len(returns) / ann
        ann_return = (total_return ** (1 / n_years) - 1) * 100 if n_years > 0 else 0.0

        # Annualized volatility
        ann_vol = returns.std() * np.sqrt(ann) * 100

        # Sharpe ratio
        excess = returns.mean() - risk_free_rate / ann
        sharpe = (excess / returns.std() * np.sqrt(ann)) if returns.std() > 0 else 0.0

        # Sortino ratio
        downside = returns[returns < 0]
        downside_std = downside.std() if len(downside) > 1 else 0.0
        sortino = (excess / downside_std * np.sqrt(ann)) if downside_std > 0 else 0.0

        # Max drawdown
        cumulative = (1 + returns).cumprod()
        running_max = cumulative.cummax()
        drawdowns = (cumulative - running_max) / running_max
        max_dd = drawdowns.min() * 100

        # Win rate
        win_rate = (returns > 0).sum() / len(returns) * 100

        records.append(
            {
                "Symbol": symbol,
                "Ann. Return": ann_return,
                "Ann. Volatility": ann_vol,
                "Sharpe Ratio": sharpe,
                "Sortino Ratio": sortino,
                "Max Drawdown": max_dd,
                "Win Rate": win_rate,
                "Total Bars": len(subset),
            }
        )

    return pd.DataFrame(records)

