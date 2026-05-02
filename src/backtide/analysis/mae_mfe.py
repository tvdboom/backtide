"""Backtide.

Author: Mavs
Description: Module containing the MAE/MFE scatter chart.

"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import GREEN, RED, REFERENCE_LINE, _plot, _resolve_runs_currency
from backtide.config import get_config
from backtide.core.data import Interval
from backtide.storage import query_bars
from backtide.utils.utils import _format_price, _moment_to_strftime

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.backtest import RunResult

cfg = get_config()


@overload
def plot_mae_mfe(
    run: RunResult,
    *,
    interval: str | None = ...,
    symbols: list[str] | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_mae_mfe(
    run: RunResult,
    *,
    interval: str | None = ...,
    symbols: list[str] | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_mae_mfe(
    run: RunResult,
    *,
    interval: str | None = None,
    symbols: list[str] | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Plot Maximum Adverse Excursion vs Maximum Favourable Excursion per trade.

    For each closed trade in `run`, compute the maximum unrealized loss (MAE)
    and gain (MFE) versus the entry price. Markers are colored by final PnL sign
    (green = winner, red = loser). The diagonal reference line marks `mfe == mae`.

    Parameters
    ----------
    run : [RunResult]
        The strategy run whose trades will be analyzed.

    interval : str | [Interval] | None, default=None
        Bar interval to load (e.g., `1d`, `1h`). When `None`, the
        function lets `query_bars` pick whatever is available.

    symbols : list[str] | None, default=None
        List of symbols to include in the plot. If `None` or empty,
        all traded symbols are included.

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
    - backtide.analysis:plot_trade_pnl
    - backtide.backtest:Trade
    - backtide.storage:query_bars

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_mae_mfe
    from backtide.storage import query_experiments, query_strategy_runs

    exp = query_experiments()[0]
    runs = query_strategy_runs(exp.id)
    plot_mae_mfe(runs[0], interval="1d")
    ```

    """
    cache = {}  # Cache bars per symbol so we only hit storage once per traded symbol.

    fig = go.Figure()
    ccy = _resolve_runs_currency([run])

    if Interval(interval).is_intraday():
        dt_fmt = _moment_to_strftime(cfg.display.datetime_format())
    else:
        dt_fmt = _moment_to_strftime(cfg.display.date_format)

    win_mae, win_mfe, win_text = [], [], []
    loss_mae, loss_mfe, loss_text = [], [], []
    for t in run.trades:
        # Filter by symbols if specified
        if symbols and t.symbol not in symbols:
            continue

        if t.symbol not in cache:
            cache[t.symbol] = query_bars(symbol=t.symbol, interval=interval)
        df = cache[t.symbol]

        window = df.loc[(df["open_ts"] >= t.entry_ts) & (df["open_ts"] <= t.exit_ts)]
        if window.empty:
            continue

        if t.quantity >= 0:
            # Long: gain on highs, loss on lows.
            mfe = max(0.0, window["high"].max() - t.entry_price)
            mae = max(0.0, t.entry_price - window["low"].min())
        else:
            # Short: gain when price drops, loss when price rises.
            mfe = max(0.0, t.entry_price - window["low"].min())
            mae = max(0.0, window["high"].max() - t.entry_price)

        label = (
            f"<b>{t.symbol}</b><br>"
            f"Entry: {pd.to_datetime(t.entry_ts, unit='s'):{dt_fmt}}<br>"
            f"Exit: {pd.to_datetime(t.exit_ts, unit='s'):{dt_fmt}}<br>"
            f"PnL: {_format_price(t.pnl, signed=True, currency=ccy)}<br>"
            f"MAE: {_format_price(mae, currency=ccy)}<br>"
            f"MFE: {_format_price(mfe, currency=ccy)}"
        )

        if t.pnl >= 0:
            win_mae.append(mae)
            win_mfe.append(mfe)
            win_text.append(label)
        else:
            loss_mae.append(mae)
            loss_mfe.append(mfe)
            loss_text.append(label)

    if win_mae:
        fig.add_trace(
            go.Scatter(
                x=win_mae,
                y=win_mfe,
                mode="markers",
                name="Winners",
                marker={"color": GREEN, "size": cfg.plots.marker_size},
                customdata=win_text,
                hovertemplate="%{customdata}<extra></extra>",
                showlegend=False,
            )
        )

    if loss_mae:
        fig.add_trace(
            go.Scatter(
                x=loss_mae,
                y=loss_mfe,
                mode="markers",
                name="Losers",
                marker={"color": RED, "size": cfg.plots.marker_size},
                customdata=loss_text,
                hovertemplate="%{customdata}<extra></extra>",
                showlegend=False,
            )
        )

    # Add a diagonal line
    all_mae = [*win_mae, *loss_mae]
    all_mfe = [*win_mfe, *loss_mfe]
    if all_mae and all_mfe:
        lo = min(min(all_mae), min(all_mfe))
        hi = max(max(all_mae), max(all_mfe))
        fig.add_shape(
            type="line",
            x0=lo,
            y0=lo,
            x1=hi,
            y1=hi,
            line=REFERENCE_LINE,
        )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel=f"MAE per share{f' ({ccy.symbol})' if ccy else ''}",
        ylabel=f"MFE per share{f' ({ccy.symbol})' if ccy else ''}",
        figsize=figsize,
        filename=filename,
        display=display,
    )
