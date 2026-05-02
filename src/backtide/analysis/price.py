"""Backtide.

Author: Mavs
Description: Module containing the price line chart function.

"""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import (
    GREEN,
    RED,
    _check_columns,
    _get_currency_symbol,
    _plot,
    _resolve_dt,
)
from backtide.config import get_config
from backtide.utils.types import DataFrameLike
from backtide.utils.utils import _format_price, _to_list, _to_pandas

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.backtest import RunResult
    from backtide.indicators import BaseIndicator


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
    run: RunResult | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_price(
    data: DataFrameLike,
    price_col: str = ...,
    *,
    indicators: BaseIndicator | Sequence[BaseIndicator] | dict[str, BaseIndicator] | None = ...,
    strategy_run: RunResult | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_price(
    data: DataFrameLike,
    price_col: str = "adj_close",
    *,
    indicators: BaseIndicator | Sequence[BaseIndicator] | dict[str, BaseIndicator] | None = None,
    run: RunResult | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] = (900, 600),
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

    run : [RunResult] | None, default=None
        When provided, overlays entry/exit markers from the run's trades on top
        of the price line. Triangles mark entries (up for long, down for short)
        and crosses mark exits (green for winners, red for losers). Trades whose
        symbols are not present in `data` are silently skipped.

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
    ccy = getattr(run, "base_currency", None) or _get_currency_symbol(data)

    ind_dict = None
    if indicators is not None:
        if isinstance(indicators, dict):
            ind_dict = indicators
        else:
            ind_dict = {x.__class__.__name__: x for x in _to_list(indicators)}

    symbols = data["symbol"].unique()
    for idx, symbol in enumerate(symbols):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=subset[price_col],
                mode="lines",
                name="Price" if ind_dict else symbol,
                line={"color": color, "width": cfg.plots.line_width},
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
                            y=(y := values.iloc[:, 0]),
                            mode="lines",
                            line={
                                "color": f"rgba{color[3:-1]}, 0.7)",
                                "width": cfg.plots.line_width,
                            },
                            name=name,
                            legendgroup=symbol,
                            customdata=[_format_price(v, currency=ccy) for v in y],
                            hovertemplate=(
                                f"%{{x}}<br>{name}: %{{customdata}}<extra>{symbol}</extra>"
                            ),
                        )
                    )
                else:
                    fig.add_traces(
                        [
                            go.Scatter(
                                x=subset["dt"],
                                y=(y := values.iloc[:, 0]),
                                mode="lines",
                                line={"width": cfg.plots.line_width / 2, "color": color},
                                customdata=[_format_price(v, currency=ccy) for v in y],
                                hovertemplate=(
                                    f"%{{x}}<br>{name} (upper): %{{customdata}}"
                                    f"<extra>{symbol}</extra>"
                                ),
                                name=name,
                                legendgroup=symbol,
                                showlegend=False,
                            ),
                            go.Scatter(
                                x=subset["dt"],
                                y=(y := values.iloc[:, 1]),
                                mode="lines",
                                line={"width": cfg.plots.line_width / 2, "color": color},
                                fill="tonexty",
                                fillcolor=f"rgba{color[3:-1]}, 0.2)",
                                customdata=[_format_price(v, currency=ccy) for v in y],
                                hovertemplate=(
                                    f"%{{x}}<br>{name} (lower): %{{customdata}}"
                                    f"<extra>{symbol}</extra>"
                                ),
                                name=name,
                                legendgroup=symbol,
                                showlegend=True,
                            ),
                        ]
                    )

    if run:

        def _hover_data(sym: str, px: float, qty: float, pnl: float) -> tuple[str, str, str, str]:
            """Convert the data for the hovertemplate to nicely formatted strings."""
            return (
                _format_price(px, currency=ccy),
                f"{qty:+,}",
                _format_price(pnl, signed=True, currency=ccy),
                sym,
            )

        long_x, long_y, short_x, short_y = [], [], [], []
        win_x, win_y, loss_x, loss_y = [], [], [], []
        long_data, short_data, win_data, loss_data = [], [], [], []
        for t in run.trades:
            sym = str(t.symbol)
            if sym not in symbols:
                continue

            if t.quantity >= 0:
                long_x.append(pd.to_datetime(t.entry_ts, unit="s"))
                long_y.append(t.entry_price)
                long_data.append(_hover_data(sym, t.entry_price, t.quantity, t.pnl))
            else:
                short_x.append(pd.to_datetime(t.entry_ts, unit="s"))
                short_y.append(t.entry_price)
                short_data.append(_hover_data(sym, t.entry_price, t.quantity, t.pnl))

            if t.pnl >= 0:
                win_x.append(pd.to_datetime(t.exit_ts, unit="s"))
                win_y.append(t.exit_price)
                win_data.append(_hover_data(sym, t.exit_price, t.quantity, t.pnl))
            else:
                loss_x.append(pd.to_datetime(t.exit_ts, unit="s"))
                loss_y.append(t.exit_price)
                loss_data.append(_hover_data(sym, t.exit_price, t.quantity, t.pnl))

        if long_x:
            fig.add_trace(
                go.Scatter(
                    x=long_x,
                    y=long_y,
                    mode="markers",
                    name="Long entry",
                    marker={
                        "symbol": "triangle-up",
                        "color": "#2ecc71",
                        "size": cfg.plots.marker_size + 3,
                    },
                    legendgroup="trades",
                    customdata=long_data,
                    hovertemplate=(
                        "%{x}<br>Price: %{customdata[0]}<br>Qty: %{customdata[1]}"
                        "<extra>%{customdata[3]}</extra>"
                    ),
                    showlegend=False,
                )
            )

        if short_x:
            fig.add_trace(
                go.Scatter(
                    x=short_x,
                    y=short_y,
                    mode="markers",
                    name="Short entry",
                    marker={
                        "symbol": "triangle-down",
                        "color": "#3498db",
                        "size": cfg.plots.marker_size + 3,
                    },
                    legendgroup="trades",
                    customdata=short_data,
                    hovertemplate=(
                        "%{x}<br>Price: %{customdata[0]}<br>Qty: %{customdata[1]}"
                        "<extra>%{customdata[3]}</extra>"
                    ),
                    showlegend=False,
                )
            )

        if win_x:
            fig.add_trace(
                go.Scatter(
                    x=win_x,
                    y=win_y,
                    mode="markers",
                    name="Exit (win)",
                    marker={
                        "symbol": "x",
                        "color": GREEN,
                        "size": cfg.plots.marker_size + 2,
                        "line": {"width": cfg.plots.line_width},
                    },
                    legendgroup="trades",
                    customdata=win_data,
                    hovertemplate=(
                        "%{x}<br>Price: %{customdata[0]}<br>"
                        "Qty: %{customdata[1]}<br>PnL: %{customdata[2]}"
                        "<extra>%{customdata[3]}</extra>"
                    ),
                    showlegend=False,
                )
            )

        if loss_x:
            fig.add_trace(
                go.Scatter(
                    x=loss_x,
                    y=loss_y,
                    mode="markers",
                    name="Exit (loss)",
                    marker={
                        "symbol": "x",
                        "color": RED,
                        "size": cfg.plots.marker_size + 2,
                        "line": {"width": cfg.plots.line_width},
                    },
                    legendgroup="trades",
                    customdata=loss_data,
                    hovertemplate=(
                        "%{x}<br>Price: %{customdata[0]}<br>"
                        "Qty: %{customdata[1]}<br>PnL: %{customdata[2]}"
                        "<extra>%{customdata[3]}</extra>"
                    ),
                    showlegend=False,
                )
            )

    return _plot(
        fig,
        groupclick="togglegroup",
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=f"Price ({ccy.symbol})" if ccy else "Price",
        figsize=figsize,
        filename=filename,
        display=display,
    )
