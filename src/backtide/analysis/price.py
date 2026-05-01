"""Backtide.

Author: Mavs
Description: Module containing the price line chart function.

"""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path
from typing import TYPE_CHECKING, Any, overload

import pandas as pd
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


if TYPE_CHECKING:
    from backtide.backtest import StrategyRunResult


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
    run: StrategyRunResult | None = ...,
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
    strategy_run: StrategyRunResult | None = ...,
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
    run: StrategyRunResult | None = None,
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

    run : [StrategyRunResult] | None, default=None
        When provided, overlays entry/exit markers from the strategy run's
        trades on top of the price line. Triangles mark entries (up for
        long, down for short) and crosses mark exits (green for winners,
        red for losers). Trades whose symbols are not present in `data`
        are silently skipped.

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
    from backtide.storage import query_bars
    from backtide.analysis import plot_price
    from backtide.indicators import BollingerBands, SimpleMovingAverage

    df = query_bars(["AAPL", "MSFT"], "1d")

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

    if run:
        available = set(data["symbol"].unique())
        long_x: list[Any] = []
        long_y: list[float] = []
        short_x: list[Any] = []
        short_y: list[float] = []
        win_x: list[Any] = []
        win_y: list[float] = []
        loss_x: list[Any] = []
        loss_y: list[float] = []
        long_text: list[str] = []
        short_text: list[str] = []
        win_text: list[str] = []
        loss_text: list[str] = []

        for t in getattr(run, "trades", None) or []:
            sym = str(getattr(t, "symbol", ""))
            if sym not in available:
                continue
            entry_dt = pd.to_datetime(int(t.entry_ts), unit="s")
            exit_dt = pd.to_datetime(int(t.exit_ts), unit="s")
            entry_price = float(t.entry_price)
            exit_price = float(t.exit_price)
            qty = int(getattr(t, "quantity", 0))
            pnl = float(t.pnl)
            label = f"{sym}<br>Qty: {qty:+,}<br>PnL: {pnl:+,.2f}"

            if qty >= 0:
                long_x.append(entry_dt)
                long_y.append(entry_price)
                long_text.append(label)
            else:
                short_x.append(entry_dt)
                short_y.append(entry_price)
                short_text.append(label)

            if pnl >= 0:
                win_x.append(exit_dt)
                win_y.append(exit_price)
                win_text.append(label)
            else:
                loss_x.append(exit_dt)
                loss_y.append(exit_price)
                loss_text.append(label)

        if long_x:
            fig.add_trace(
                go.Scatter(
                    x=long_x,
                    y=long_y,
                    mode="markers",
                    name="Long entry",
                    marker={"symbol": "triangle-up", "color": "#2ecc71", "size": 11},
                    legendgroup="trades",
                    customdata=long_text,
                    hovertemplate="%{customdata}<extra>long entry</extra>",
                )
            )
        if short_x:
            fig.add_trace(
                go.Scatter(
                    x=short_x,
                    y=short_y,
                    mode="markers",
                    name="Short entry",
                    marker={"symbol": "triangle-down", "color": "#3498db", "size": 11},
                    legendgroup="trades",
                    customdata=short_text,
                    hovertemplate="%{customdata}<extra>short entry</extra>",
                )
            )
        if win_x:
            fig.add_trace(
                go.Scatter(
                    x=win_x,
                    y=win_y,
                    mode="markers",
                    name="Exit (win)",
                    marker={"symbol": "x", "color": "#27ae60", "size": 10, "line": {"width": 2}},
                    legendgroup="trades",
                    customdata=win_text,
                    hovertemplate="%{customdata}<extra>exit</extra>",
                )
            )
        if loss_x:
            fig.add_trace(
                go.Scatter(
                    x=loss_x,
                    y=loss_y,
                    mode="markers",
                    name="Exit (loss)",
                    marker={"symbol": "x", "color": "#e74c3c", "size": 10, "line": {"width": 2}},
                    legendgroup="trades",
                    customdata=loss_text,
                    hovertemplate="%{customdata}<extra>exit</extra>",
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
