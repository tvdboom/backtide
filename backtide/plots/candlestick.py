"""Backtide.

Author: Mavs
Description: Module containing the candlestick chart function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pandas as pd
import plotly.graph_objects as go

from backtide.plots.utils import (
    _plot,
)


def plot_candlestick(
    df: pd.DataFrame,
    *,
    rangeslider: bool = True,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper right",
    figsize: tuple[int, int] | None = None,
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a candlestick chart using Plotly.

    Visualizes OHLC (Open-High-Low-Close) price data over time as
    candlestick bars — the standard chart type used in financial
    technical analysis. When the dataframe contains multiple symbols,
    each symbol gets its own color-coded candlestick trace with a
    matching close-price line overlay for readability.

    Parameters
    ----------
    df : pd.DataFrame
        Input data containing columns `open`, `high`, `low`, `close`
        and `dt` with the datetime.

    rangeslider : bool, default=True
        Whether to show the range slider below the chart.

    title : str, dict or None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, custom title configuration.

    legend : str, dict or None, default="upper right"
        Legend for the plot.

        - If None: no legend is shown.
        - If str: named position (e.g., `"lower left"`).
        - If dict: legend configuration.

    figsize : tuple[int, int] or None, default=None
        Figure size in pixels as `(width, height)`. If None, defaults to
        `(900, 600)`.

    filename : str, Path or None, default=None
        Save the plot to this path. If None, the plot is not saved.

    display : bool or None, default=True
        Whether to render the plot. If None, return the figure.

    Returns
    -------
    go.Figure or None
        The Plotly figure object. Only returned if `display=None`.

    Examples
    --------
    ```pycon
    from backtide.storage import query_bars
    from backtide.plots import plot_candlestick

    df = query_bars("AAPL", "1d")
    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)
    fig = plot_candlestick(df, title="AAPL Daily", display=None)
    ```

    """
    df = df.sort_values("dt")

    fig = go.Figure()

    # Default candlestick colors
    inc = "#26A69A"  # Teal (bullish)
    dec = "#EF5350"  # Red (bearish)

    fig.add_trace(
        go.Candlestick(
            x=df["dt"],
            open=df["open"],
            high=df["high"],
            low=df["low"],
            close=df["close"],
            whiskerwidth=0.2,
            increasing={"line": {"color": inc}, "fillcolor": inc},
            decreasing={"line": {"color": dec}, "fillcolor": dec},
            showlegend=False,
        )
    )

    # Default visible range: last month (user can zoom out to see all)
    x_end = df["dt"].max()
    x_start = x_end - pd.DateOffset(months=1)

    fig.update_layout(
        xaxis=dict(
            rangeslider_visible=rangeslider,
            range=[x_start, x_end],
            type="date",
        ),
        yaxis=dict(autorange=True, fixedrange=False),
        uirevision="constant",
    )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel="Price",
        figsize=figsize,
        filename=filename,
        display=display,
    )
