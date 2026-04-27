"""Backtide.

Author: Mavs
Description: Module containing the price line chart function for data analysis.

"""

from __future__ import annotations

from collections.abc import Sequence
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
from backtide.indicators import BaseIndicator
from backtide.utils.utils import _format_price, _to_list, _to_pandas

# Supported price columns and their display labels.
PRICE_COLUMNS = {
    "open": "Open",
    "high": "High",
    "low": "Low",
    "close": "Close",
    "adj_close": "Adj. Close",
}


cfg = get_config()


@overload
def plot_price(
    data: DataFrameLike,
    price_col: str = ...,
    *,
    indicators: BaseIndicator | Sequence[BaseIndicator] | dict[str, BaseIndicator] | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_price(
    data: DataFrameLike,
    price_col: str = ...,
    *,
    indicators: BaseIndicator | Sequence[BaseIndicator] | dict[str, BaseIndicator] | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_price(
    data: DataFrameLike,
    price_col: str = "adj_close",
    *,
    indicators: BaseIndicator | Sequence[BaseIndicator] | dict[str, BaseIndicator] | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a price line chart.

    Optionally, overlay the prices with indicators.

    Parameters
    ----------
    data : pd.DataFrame | pl.DataFrame
        Input data containing columns `symbol`, `open`, `high`, `low`, `close`
        and `dt` with the datetime.

    price_col : str, default="adj_close"
        Column name in `data` to plot on the y-axis.

    indicators : [BaseIndicator] | Sequence[[BaseIndicator]] | dict[str, [BaseIndicator]] or None, default=None
        Indicators to overlay on the price chart. If dict, it must map a name
        (used in the legend) to an indicator instance.

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
    - backtide.analysis:plot_volume
    - backtide.analysis:plot_vwap

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.analysis import plot_price
    from backtide.indicators import BollingerBands, SimpleMovingAverage

    df = query_bars(["AAPL", "MSFT"], "1d")
    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)

    # Compare the price of two symbols
    plot_price(df)

    # Add a line indicator to the price chart
    aapl = df[df["symbol"] == "AAPL"]
    plot_price(aapl, indicators=SimpleMovingAverage())

    # Add a band indicator to the price chart
    plot_price(aapl, indicators=BollingerBands())
    ```

    """
    data = _resolve_dt(_to_pandas(data))
    _check_columns(data, ["symbol", price_col, "dt"], "plot_price")

    fig = go.Figure()
    currency = _get_currency_symbol(data)

    ind_dict = None
    if indicators is not None:
        if isinstance(indicators, dict):
            ind_dict = indicators
        else:
            ind_dict = {x.__class__.__name__: x for x in _to_list(indicators)}

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=subset[price_col],
                mode="lines",
                name="Price" if ind_dict else symbol,
                line={"color": color, "width": 2},
                legendgroup=symbol,
                legendgrouptitle_text=symbol if ind_dict else None,
                customdata=[
                    _format_price(x[price_col], currency=x.get("currency"))
                    for _, x in subset.iterrows()
                ],
                hovertemplate=f"%{{x}}<br>Price: %{{customdata}}<extra>{symbol}</extra>",
            )
        )

        if ind_dict:
            for name, ind in ind_dict.items():
                values = _to_pandas(ind.compute(subset))  # ty: ignore[unresolved-attribute]

                if values.shape[1] == 1:
                    fig.add_trace(
                        go.Scatter(
                            x=subset["dt"],
                            y=values.iloc[:, 0],
                            mode="lines",
                            line={"color": f"rgba{color[3:-1]}, 0.7)", "width": 1.5},
                            name=name,
                            legendgroup=symbol,
                        )
                    )
                else:
                    fig.add_traces(
                        [
                            go.Scatter(
                                x=subset["dt"],
                                y=values.iloc[:, 0],
                                mode="lines",
                                line={"width": 1, "color": color},
                                hovertemplate="%{y}<extra>upper bound</extra>",
                                name=name,
                                legendgroup=symbol,
                                showlegend=False,
                            ),
                            go.Scatter(
                                x=subset["dt"],
                                y=values.iloc[:, 1],
                                mode="lines",
                                line={"width": 1, "color": color},
                                fill="tonexty",
                                fillcolor=f"rgba{color[3:-1]}, 0.2)",
                                hovertemplate="%{y}<extra>lower bound</extra>",
                                name=name,
                                legendgroup=symbol,
                                showlegend=True,
                            ),
                        ]
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
