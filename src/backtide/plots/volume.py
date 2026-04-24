"""Backtide.

Author: Mavs
Description: Module containing the volume bar chart function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd
import plotly.graph_objects as go

from backtide.config import get_config
from backtide.plots.utils import _get_currency_symbol, _plot
from backtide.utils.utils import _format_number

cfg = get_config()


def plot_volume(
    data: pd.DataFrame,
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a volume bar chart.

    Displays trading volume over time for one or more symbols. Each symbol
    is rendered as a separate bar trace with its own color.

    Parameters
    ----------
    data : pd.DataFrame
        Input data containing columns `symbol`, `volume` and `dt` with the
        datetime.

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
    - backtide.plots:plot_vwap

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.plots import plot_volume

    df = query_bars("AAPL", "1d")
    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)
    df["currency"] = "USD"

    plot_volume(df)
    ```

    """
    fig = go.Figure()

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=subset["volume"],
                name=symbol,
                mode="lines",
                line={"width": 0.5, "color": color},
                fill="tozeroy",
                fillcolor=f"rgba({color[4:-1]}, 0.4)",
                opacity=0.85,
                hovertemplate="%{x}<br>Volume: %{y:,.0f}<extra>" + symbol + "</extra>",
            )
        )

    # Format y-axis ticks with compact notation (e.g., 1.5M, 200k)
    all_volumes = data["volume"].dropna()
    if not all_volumes.empty:
        max_vol = all_volumes.max()
        tick_step = 10 ** int(np.log10(max(max_vol, 1)))
        if max_vol / tick_step < 3:
            tick_step //= 2
        tick_vals = list(range(0, int(max_vol + tick_step), int(tick_step)))
        fig.update_yaxes(
            tickmode="array",
            tickvals=tick_vals,
            ticktext=[_format_number(v) for v in tick_vals],
        )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=f"Volume ({cs})" if (cs := _get_currency_symbol(data)) else "Volume (shares)",
        figsize=figsize,
        filename=filename,
        display=display,
    )
