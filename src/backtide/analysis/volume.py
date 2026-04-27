"""Backtide.

Author: Mavs
Description: Module containing the volume bar chart function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any, overload

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

cfg = get_config()


@overload
def plot_volume(
    data: DataFrameLike,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_volume(
    data: DataFrameLike,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_volume(
    data: DataFrameLike,
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a trading volume bar chart.

    Displays trading volume over time for one or more symbols. Each symbol
    is rendered as a separate bar trace.

    Parameters
    ----------
    data : pd.DataFrame | pl.DataFrame
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
    - backtide.analysis:plot_candlestick
    - backtide.analysis:plot_price
    - backtide.analysis:plot_vwap

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.analysis import plot_volume

    df = query_bars("AAPL", "1d")
    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)

    # Plot raw share volume
    plot_volume(df)

    # Plot price x share (dollar volume)
    df_vol = df.copy()
    df["volume"] = df["volume"] * df["close"]
    df["currency"] = "USD"  # Add currency to format labels
    plot_volume(df, title="Dollar volume for AAPL")
    ```

    """
    data = _resolve_dt(_to_pandas(data))
    _check_columns(data, ["symbol", "volume", "dt"], "plot_volume")

    fig = go.Figure()
    currency = _get_currency_symbol(data)

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol]
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
                customdata=[
                    _format_price(x["volume"], decimals=0, currency=x.get("currency"))
                    for _, x in subset.iterrows()
                ],
                hovertemplate=f"%{{x}}<br>Volume: %{{customdata}}<extra>{symbol}</extra>",
            )
        )

    # Compact SI notation for y-axis ticks (e.g. 1M, 200k)
    fig.update_yaxes(tickformat="~s")

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=f"Volume ({currency.symbol})" if currency else "Volume",
        figsize=figsize,
        filename=filename,
        display=display,
    )
