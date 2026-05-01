"""Backtide.

Author: Mavs
Description: Module containing the VWAP chart function.

"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, overload

import plotly.graph_objects as go

from backtide.analysis.utils import (
    DataFrameLike,
    _check_columns,
    _get_currency_symbol,
    _plot,
    _resolve_dt,
)
from backtide.config import get_config
from backtide.utils.utils import _format_price, _to_pandas

if TYPE_CHECKING:
    from pathlib import Path

cfg = get_config()


@overload
def plot_vwap(
    data: DataFrameLike,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_vwap(
    data: DataFrameLike,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_vwap(
    data: DataFrameLike,
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
    data : pd.DataFrame | pl.DataFrame
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
    - backtide.analysis:plot_candlestick
    - backtide.analysis:plot_price
    - backtide.analysis:plot_volume

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.analysis import plot_vwap

    df = query_bars(["AAPL", "MSFT"], "1d")
    plot_vwap(df)
    ```

    """
    data = _resolve_dt(_to_pandas(data))
    _check_columns(data, ["symbol", "dt", "high", "low", "close", "volume"], "plot_vwap")

    fig = go.Figure()
    currency = _get_currency_symbol(data)
    intraday = data["dt"].dt.date.duplicated(keep=False).any()

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        # Compute typical price and VWAP
        typical_price = (subset["high"] + subset["low"] + subset["close"]) / 3
        tp_vol = typical_price * subset["volume"]

        if intraday:
            # Reset VWAP daily for intraday data
            day = subset["dt"].dt.date
            vwap = tp_vol.groupby(day).cumsum() / subset["volume"].groupby(day).cumsum()
        else:
            vwap = tp_vol.cumsum() / subset["volume"].cumsum()

        # Close price as a thin line
        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=subset["close"],
                mode="lines",
                name="Close",
                line={"color": color, "width": 2, "dash": "dot"},
                opacity=0.8,
                legendgroup=symbol,
                legendgrouptitle_text=symbol,
                customdata=[
                    _format_price(x["close"], currency=x.get("currency"))
                    for _, x in subset.iterrows()
                ],
                hovertemplate=f"%{{x}}<br>Close: %{{customdata}}<extra>{symbol}</extra>",
            )
        )

        # VWAP as a bold line
        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=vwap,
                mode="lines",
                name="VWAP",
                line={"color": color, "width": 2},
                legendgroup=symbol,
                customdata=[
                    _format_price(vwap[i], currency=x.get("currency"))
                    for i, x in subset.iterrows()
                ],
                hovertemplate=f"%{{x}}<br>VWAP: %{{customdata}}<extra>{symbol}</extra>",
            )
        )

    return _plot(
        fig,
        groupclick="togglegroup",
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=f"Price ({currency.symbol})" if currency else "Price",
        figsize=figsize,
        filename=filename,
        display=display,
    )
