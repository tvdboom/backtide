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
    _check_columns,
    _get_currency_symbol,
    _plot,
)


def plot_candlestick(
    data: pd.DataFrame,
    *,
    rangeslider: bool = True,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a candlestick chart.

    Visualizes OHLC (Open-High-Low-Close) price data over time as
    candlestick bars — the standard chart type used in financial
    technical analysis. When the dataframe contains multiple symbols,
    each symbol gets its own color-coded candlestick trace with a
    matching close-price line overlay for readability.

    Parameters
    ----------
    data : pd.DataFrame
        Input data containing columns `open`, `high`, `low`, `close`
        and `dt` with the datetime.

    rangeslider : bool, default=True
        Whether to show the range slider below the chart.

    title : str | dict | None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, [title configuration][parameters].

    legend : str | dict | None, default="upper left"
        Legend for the plot. See the [user guide][parameters] for an extended
        description of the choices.

        - If None: No legend is shown.
        - If str: Position to display the legend.
        - If dict: Legend configuration.

    figsize : tuple[int, int] | None, default=(900, 600)
        Figure's size in pixels, format as (x, y).

    filename : str | Path | None, default=None
        Save the plot using this name. The type of the file depends on the
        provided name (`.html`, `.png`, `.pdf`, etc...). If `filename` has no
        file type, the plot is saved as `.html`. If `None`, the plot isn't saved.

    display : bool | None, default=True
        Whether to render the plot. If `None`, it returns the figure.

    Returns
    -------
    go.Figure | None
        The Plotly figure object. Only returned if `display=None`.

    See Also
    --------
    - backtide.plots:plot_price
    - backtide.plots:plot_volume
    - backtide.plots:plot_vwap

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.plots import plot_candlestick

    df = query_bars("AAPL", "1d")

    # Show only the last 30 days
    df = df.sort_values("open_ts").iloc[-30:]

    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)
    plot_candlestick(df, title="AAPL Daily")
    ```

    """
    _check_columns(data, ["symbol", "dt", "open", "high", "low", "close"], "plot_candlestick")

    fig = go.Figure()

    # Default candlestick colors
    inc = "#26A69A"  # Teal (bullish)
    dec = "#EF5350"  # Red (bearish)

    fig.add_trace(
        go.Candlestick(
            x=data["dt"],
            open=data["open"],
            high=data["high"],
            low=data["low"],
            close=data["close"],
            whiskerwidth=0.2,
            name=data["symbol"].iloc[0],
            increasing={"line": {"color": inc}, "fillcolor": inc},
            decreasing={"line": {"color": dec}, "fillcolor": dec},
            showlegend=False,
        )
    )

    fig.update_layout(
        xaxis={"rangeslider_visible": rangeslider, "type": "date"},
        yaxis={"autorange": True, "fixedrange": False},
        uirevision="constant",
    )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=f"Price ({cs})" if (cs := _get_currency_symbol(data)) else "Price",
        figsize=figsize,
        filename=filename,
        display=display,
    )
