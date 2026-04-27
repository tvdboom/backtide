"""Backtide.

Author: Mavs
Description: Module containing the dividend history chart for data analysis.

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
from backtide.utils.utils import _to_pandas

cfg = get_config()


@overload
def plot_dividends(
    data: DataFrameLike,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_dividends(
    data: DataFrameLike,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_dividends(
    data: DataFrameLike,
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
    data : pd.DataFrame | pl.DataFrame
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
    - backtide.analysis:plot_drawdown
    - backtide.analysis:plot_price
    - backtide.analysis:plot_returns

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_dividends
    from backtide.analysis import plot_dividends

    df = query_dividends(["AAPL", "MSFT"])
    df["dt"] = pd.to_datetime(df["ex_date"], unit="s", utc=True)

    plot_dividends(df)
    ```

    """
    data = _resolve_dt(_to_pandas(data))
    _check_columns(data, ["symbol", "dt", "amount"], "plot_dividends")

    fig = go.Figure()
    currency = _get_currency_symbol(data)

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        # Stem lines from zero to each dividend amount
        for _, row in subset.iterrows():
            fig.add_trace(
                go.Scatter(
                    x=[row["dt"], row["dt"]],
                    y=[0, row["amount"]],
                    mode="lines",
                    line={"color": color, "width": 1.5},
                    showlegend=False,
                    hoverinfo="skip",
                )
            )

        # Markers at the dividend amounts
        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=subset["amount"],
                name=symbol,
                mode="markers",
                marker={"color": color, "size": 8, "symbol": "circle"},
                opacity=0.9,
                hovertemplate="%{x}<br>Dividend: $%{y:.2f}<extra>" + symbol + "</extra>",
            )
        )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Ex-Dividend Date",
        ylabel=f"Dividend ({currency.symbol})" if currency else "Dividend",
        figsize=figsize,
        filename=filename,
        display=display,
    )
