"""Backtide.

Author: Mavs
Description: Module containing the candlestick chart function for data analysis.

"""

from __future__ import annotations

from typing import Any

import pandas as pd
import plotly.graph_objects as go


def plot_candlestick(
    df: pd.DataFrame,
    *,
    datetime_col: str = "datetime",
    open_col: str = "open",
    high_col: str = "high",
    low_col: str = "low",
    close_col: str = "close",
    group_by: str | None = None,
    title: str | None = None,
    xlabel: str | None = None,
    ylabel: str | None = None,
    showlegend: bool = True,
    increasing_color: str | None = None,
    decreasing_color: str | None = None,
    line_width: int = 1,
    width: int = 900,
    height: int = 600,
    template: str = "plotly_dark",
    rangeslider: bool = True,
    layout_kwargs: dict[str, Any] | None = None,
    trace_kwargs: dict[str, Any] | None = None,
) -> go.Figure:
    """Create a candlestick chart using Plotly.

    Visualizes OHLC (Open-High-Low-Close) price data over time as
    candlestick bars — the standard chart type used in financial
    technical analysis. Each candlestick represents one bar interval
    and encodes the open, high, low, and close prices.

    Parameters
    ----------
    df : pd.DataFrame
        Input data containing OHLC columns and a datetime column.

    datetime_col : str, default="datetime"
        Name of the column containing datetime values for the x-axis.

    open_col : str, default="open"
        Name of the column containing open prices.

    high_col : str, default="high"
        Name of the column containing high prices.

    low_col : str, default="low"
        Name of the column containing low prices.

    close_col : str, default="close"
        Name of the column containing close prices.

    group_by : str or None, default=None
        Column name to split data into separate candlestick traces
        (e.g., by symbol). When None, a single trace is drawn.

    title : str or None, default=None
        Title for the plot. None means no title.

    xlabel : str or None, default=None
        Label for the x-axis. None defaults to "Date".

    ylabel : str or None, default=None
        Label for the y-axis. None defaults to "Price".

    showlegend : bool, default=True
        Whether to display the legend.

    increasing_color : str or None, default=None
        Color for bullish (close > open) candlesticks.

    decreasing_color : str or None, default=None
        Color for bearish (close < open) candlesticks.

    line_width : int, default=1
        Width of the candlestick border lines.

    width : int, default=900
        Figure width in pixels.

    height : int, default=600
        Figure height in pixels.

    template : str, default="plotly_dark"
        Plotly template name for the figure styling.

    rangeslider : bool, default=True
        Whether to show the range slider below the chart.

    layout_kwargs : dict or None, default=None
        Extra keyword arguments forwarded to ``fig.update_layout``.

    trace_kwargs : dict or None, default=None
        Extra keyword arguments forwarded to every ``go.Candlestick`` trace.

    Returns
    -------
    go.Figure
        The Plotly figure object, ready to be displayed or saved.

    Examples
    --------
    ```python
    from backtide.storage import query_bars
    from backtide.plots import plot_candlestick

    df = query_bars("AAPL", "1d")
    df["datetime"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)
    fig = plot_candlestick(df, title="AAPL Daily")
    fig.show()
    ```

    """
    fig = go.Figure()

    if group_by is not None and group_by in df.columns:
        groups = df[group_by].unique()
    else:
        groups = [None]

    for grp in groups:
        subset = df[df[group_by] == grp] if grp is not None else df

        candle_kwargs: dict[str, Any] = {
            "x": subset[datetime_col],
            "open": subset[open_col],
            "high": subset[high_col],
            "low": subset[low_col],
            "close": subset[close_col],
            "name": str(grp) if grp is not None else "Price",
        }

        if increasing_color:
            candle_kwargs["increasing"] = {"line": {"color": increasing_color, "width": line_width}}
        if decreasing_color:
            candle_kwargs["decreasing"] = {"line": {"color": decreasing_color, "width": line_width}}

        if trace_kwargs:
            candle_kwargs.update(trace_kwargs)

        fig.add_trace(go.Candlestick(**candle_kwargs))

    # Layout
    default_layout: dict[str, Any] = {
        "template": template,
        "width": width,
        "height": height,
        "showlegend": showlegend,
        "xaxis_title": xlabel or "Date",
        "yaxis_title": ylabel or "Price",
        "xaxis_rangeslider_visible": rangeslider,
    }

    if title:
        default_layout["title"] = {
            "text": title,
            "x": 0.5,
            "xanchor": "center",
            "font_size": 20,
        }

    if layout_kwargs:
        default_layout.update(layout_kwargs)

    fig.update_layout(**default_layout)

    return fig

