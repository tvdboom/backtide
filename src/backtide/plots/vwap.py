"""Backtide.

Author: Mavs
Description: Module containing the VWAP chart function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pandas as pd
import plotly.graph_objects as go

from backtide.config import get_config
from backtide.plots.utils import _get_currency_symbol, _plot

cfg = get_config()


def plot_vwap(
    data: pd.DataFrame,
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a VWAP (Volume-Weighted Average Price) chart.

    Displays the cumulative VWAP alongside the closing price for one or
    more symbols. VWAP is a key benchmark used to assess whether a security
    was bought or sold at a favorable price relative to volume.

    Parameters
    ----------
    data : pd.DataFrame
        Input data containing columns `symbol`, `close`, `high`, `low`,
        `volume` and `dt` with the datetime.

    title : str | dict | None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, [title configuration][parameters].

    legend : str | dict | None, default="upper left"
        Legend for the plot. See the [user guide][parameters] for an extended
        description of the choices.

        * If None: No legend is shown.
        * If str: Position to display the legend.
        * If dict: Legend configuration.

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
    - backtide.plots:plot_candlestick
    - backtide.plots:plot_price
    - backtide.plots:plot_volume

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.plots import plot_vwap

    df = query_bars(["AAPL", "MSFT"], "1d")
    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)

    plot_vwap(df)
    ```

    """
    fig = go.Figure()

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt").copy()
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        # Compute typical price and cumulative VWAP
        typical_price = (subset["high"] + subset["low"] + subset["close"]) / 3
        cum_vol = subset["volume"].cumsum()
        cum_tp_vol = (typical_price * subset["volume"]).cumsum()
        vwap = cum_tp_vol / cum_vol

        # Close price as a thin line
        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=subset["close"],
                mode="lines",
                name=f"{symbol} Close",
                line={"color": color, "width": 1, "dash": "dot"},
                opacity=0.5,
                legendgroup=symbol,
                hovertemplate="%{x}<br>Close: %{y:.2f}<extra>" + symbol + "</extra>",
            )
        )

        # VWAP as a bold line
        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=vwap,
                mode="lines",
                name=f"{symbol} VWAP",
                line={"color": color, "width": 2.5},
                legendgroup=symbol,
                hovertemplate="%{x}<br>VWAP: %{y:.2f}<extra>" + symbol + "</extra>",
            )
        )

    _cs = _get_currency_symbol(data)

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=f"Price ({_cs})" if _cs else "Price",
        figsize=figsize,
        filename=filename,
        display=display,
    )

