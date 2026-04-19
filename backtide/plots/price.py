"""Backtide.

Author: Mavs
Description: Module containing the price line chart function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pandas as pd
import plotly.graph_objects as go

from backtide.plots.utils import PALETTE, _plot

# Supported price columns and their display labels.
PRICE_COLUMNS: dict[str, str] = {
    "open": "Open",
    "high": "High",
    "low": "Low",
    "close": "Close",
    "adj_close": "Adj. Close",
}


def plot_price(
    df: pd.DataFrame,
    *,
    price_col: str = "adj_close",
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = None,
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a price line chart.

    Parameters
    ----------
    df : pd.DataFrame
        Input data containing a `dt` datetime column, a `symbol`
        column, and at least the column named by `price_col`.

    price_col : str, default="adj_close"
        Column name to plot on the y-axis. Must be one of `open`, `high`,
        `low`, `close`, or `adj_close`.

    title : str, dict or None, default=None
        Title for the plot.

    legend : str, dict or None, default="upper left"
        Legend for the plot.

    figsize : tuple[int, int] or None, default=None
        Figure size in pixels as ``(width, height)``.

    filename : str, Path or None, default=None
        Save the plot to this path.

    display : bool or None, default=True
        Whether to render the plot. If None, return the figure.

    Returns
    -------
    go.Figure or None
        The Plotly figure object. Only returned if ``display=None``.

    """
    fig = go.Figure()

    symbols = sorted(df["symbol"].unique())

    for idx, symbol in enumerate(symbols):
        subset = df[df["symbol"] == symbol].sort_values("dt")
        color = PALETTE[idx % len(PALETTE)]

        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=subset[price_col],
                mode="lines",
                name=symbol,
                line={"color": color, "width": 1.5},
            )
        )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=PRICE_COLUMNS[price_col],
        figsize=figsize,
        filename=filename,
        display=display,
    )
