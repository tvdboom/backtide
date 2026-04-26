"""Backtide.

Author: Mavs
Description: Module containing the drawdown chart function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.config import get_config
from backtide.analysis.utils import _check_columns, _plot

cfg = get_config()


@overload
def plot_drawdown(
    data: pd.DataFrame,
    price_col: str = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_drawdown(
    data: pd.DataFrame,
    price_col: str = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_drawdown(
    data: pd.DataFrame,
    price_col: str = "adj_close",
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "lower left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a drawdown chart.

    Plots the percentage drawdown from the running peak over time for
    one or more symbols. Drawdown measures the decline from a historical
    peak in price and is a key risk metric.

    Parameters
    ----------
    data : pd.DataFrame
        Input data containing columns `symbol`, the column specified by
        `price_col`, and `dt` with the datetime.

    price_col : str, default="adj_close"
        Column name used to compute the drawdown.

    title : str | dict | None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, [title configuration][parameters].

    legend : str | dict | None, default="lower left"
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
    - backtide.analysis:plot_correlation
    - backtide.analysis:plot_price
    - backtide.analysis:plot_returns

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.analysis import plot_drawdown

    df = query_bars(["AAPL", "MSFT"], "1d")
    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)

    plot_drawdown(df)
    ```

    """
    _check_columns(data, ["symbol", price_col, "dt"], "plot_drawdown")

    fig = go.Figure()

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        prices = subset[price_col]
        cummax = prices.cummax()
        drawdown = (prices - cummax) / cummax * 100
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=drawdown,
                mode="lines",
                name=symbol,
                line={"color": color, "width": 2},
                fill="tozeroy",
                fillcolor=f"rgba{color[3:-1]}, 0.15)",
            )
        )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel="Drawdown (%)",
        figsize=figsize,
        filename=filename,
        display=display,
    )
