"""Backtide.

Author: Mavs
Description: Module containing the dividend history chart for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pandas as pd
import plotly.graph_objects as go

from backtide.config import get_config
from backtide.plots.utils import _plot

cfg = get_config()


def plot_dividends(
    data: pd.DataFrame,
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a dividend history chart.

    Displays dividend payments over time for one or more symbols as a bar
    chart with markers, making it easy to compare payout history and
    identify trends.

    Parameters
    ----------
    data : pd.DataFrame
        Input data containing columns `symbol`, `ex_date` (unix timestamp
        or datetime) and `amount` with the dividend amount.

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
    - backtide.plots:plot_drawdown
    - backtide.plots:plot_price
    - backtide.plots:plot_returns

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_dividends
    from backtide.plots import plot_dividends

    df = query_dividends(["AAPL", "MSFT"])
    df["dt"] = pd.to_datetime(df["ex_date"], unit="s", utc=True)

    plot_dividends(df)
    ```

    """
    fig = go.Figure()

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        fig.add_trace(
            go.Bar(
                x=subset["dt"],
                y=subset["amount"],
                name=symbol,
                marker_color=color,
                marker_line_width=0,
                opacity=0.85,
                hovertemplate="%{x}<br>Dividend: $%{y:.4f}<extra>" + symbol + "</extra>",
            )
        )

    fig.update_layout(
        barmode="group",
        bargap=0.15,
    )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Ex-Dividend Date",
        ylabel="Dividend ($)",
        figsize=figsize,
        filename=filename,
        display=display,
    )

