"""Backtide.

Author: Mavs
Description: Module containing the rolling volatility chart.

"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, overload

import plotly.graph_objects as go

from backtide.analysis.utils import _check_columns, _plot, _resolve_dt
from backtide.config import get_config
from backtide.utils.utils import _to_pandas

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.utils.types import DataFrameLike

cfg = get_config()


@overload
def plot_volatility(
    data: DataFrameLike,
    price_col: str = ...,
    window: int = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_volatility(
    data: DataFrameLike,
    price_col: str = ...,
    window: int = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_volatility(
    data: DataFrameLike,
    price_col: str = "adj_close",
    window: int = 21,
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a rolling volatility chart.

    Plots the rolling standard deviation of percentage returns over a
    configurable window for one or more symbols. Useful for tracking how
    risk evolves over time and comparing volatility regimes.

    Parameters
    ----------
    data : pd.DataFrame | pl.DataFrame
        Input data containing columns `symbol`, the column specified by
        `price_col`, and `dt` with the datetime.

    price_col : str, default="adj_close"
        Column name used to compute returns.

    window : int, default=21
        Rolling window size (number of bars) for computing volatility.

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

    figsize : tuple[int, int], default=(900, 600)
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
    - backtide.analysis:plot_drawdown
    - backtide.analysis:plot_returns
    - backtide.analysis:plot_seasonality

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.analysis import plot_volatility

    df = query_bars("AAPL", "1d")
    plot_volatility(df, window=21)
    ```

    """
    data = _resolve_dt(_to_pandas(data))
    _check_columns(data, ["symbol", price_col, "dt"], "plot_volatility")

    fig = go.Figure()

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        returns = subset[price_col].pct_change() * 100
        rolling_vol = returns.rolling(window=window).std()
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=rolling_vol,
                mode="lines",
                name=symbol,
                line={"color": color, "width": cfg.plots.line_width},
                hovertemplate=f"%{{x}}<br>Volatility: %{{y:.2f}}%<extra>{symbol}</extra>",
            )
        )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel="Volatility (%)",
        figsize=figsize,
        filename=filename,
        display=display,
    )
