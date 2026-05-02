"""Backtide.

Author: Mavs
Description: Module containing the MAE/MFE scatter chart.

"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import _plot, _resolve_runs_currency
from backtide.config import get_config
from backtide.storage import query_bars
from backtide.utils.utils import _format_price

if TYPE_CHECKING:
    from collections.abc import Iterable
    from pathlib import Path

    from backtide.backtest import RunResult

cfg = get_config()


@overload
def plot_mae_mfe(
    run: RunResult,
    *,
    interval: str | None = ...,
    symbols: Iterable[str] | None = ...,
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
    symbols: Iterable[str] | None = ...,
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
    symbols: Iterable[str] | None = None,
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

    symbols : Iterable[str] | None, default=None
        Restrict the chart to trades on these symbols.

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
    ccy = _resolve_runs_currency([run])

    fig = go.Figure()

    trades = list(getattr(run, "trades", None) or [])
    if symbols is not None:
        wanted = set(symbols)
        trades = [t for t in trades if getattr(t, "symbol", "") in wanted]

    # Cache bars per symbol so we only hit storage once per traded symbol.
    bars_cache: dict[str, pd.DataFrame] = {}
    win_mae: list[float] = []
    win_mfe: list[float] = []
    win_text: list[str] = []
    loss_mae: list[float] = []
    loss_mfe: list[float] = []
    loss_text: list[str] = []

    for t in trades:
        sym = t.symbol
        if sym not in bars_cache:
            try:
                df = query_bars(symbol=sym, interval=interval)
            except Exception:  # noqa: BLE001
                df = pd.DataFrame()
            bars_cache[sym] = df

        df = bars_cache[sym]
        if df.empty or "open_ts" not in df.columns:
            continue

        entry_ts = int(t.entry_ts)
        exit_ts = int(t.exit_ts)
        mask = (df["open_ts"] >= entry_ts) & (df["open_ts"] <= exit_ts)
        window = df.loc[mask]
        if window.empty:
            continue

        entry_price = float(t.entry_price)
        qty = int(getattr(t, "quantity", 0))
        if qty >= 0:
            # Long: gain on highs, loss on lows.
            mfe = max(0.0, float(window["high"].max()) - entry_price)
            mae = max(0.0, entry_price - float(window["low"].min()))
        else:
            # Short: gain when price drops, loss when price rises.
            mfe = max(0.0, entry_price - float(window["low"].min()))
            mae = max(0.0, float(window["high"].max()) - entry_price)

        label = (
            f"{sym}<br>Entry: {pd.to_datetime(entry_ts, unit='s'):%Y-%m-%d}<br>"
            f"Exit: {pd.to_datetime(exit_ts, unit='s'):%Y-%m-%d}<br>"
            f"PnL: {_format_price(float(t.pnl), currency=ccy)}<br>"
            f"MAE: {_format_price(mae, currency=ccy)}<br>"
            f"MFE: {_format_price(mfe, currency=ccy)}"
        )
        if float(t.pnl) >= 0:
            win_mae.append(mae)
            win_mfe.append(mfe)
            win_text.append(label)
        else:
            loss_mae.append(mae)
            loss_mfe.append(mfe)
            loss_text.append(label)

    max_axis = max(win_mae + win_mfe + loss_mae + loss_mfe + [1.0])
    fig.add_trace(
        go.Scatter(
            x=[0, max_axis],
            y=[0, max_axis],
            mode="lines",
            line={"color": "rgba(128,128,128,0.5)", "dash": "dash", "width": cfg.plots.line_width / 2},
            name="MFE = MAE",
            hoverinfo="skip",
            showlegend=False,
        )
    )

    if win_mae:
        fig.add_trace(
            go.Scatter(
                x=win_mae,
                y=win_mfe,
                mode="markers",
                name="Winners",
                marker={"color": "#2ecc71", "size": cfg.plots.marker_size, "line": {"width": 0}},
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
                marker={"color": "#e74c3c", "size": cfg.plots.marker_size, "line": {"width": 0}},
                customdata=loss_text,
                hovertemplate="%{customdata}<extra></extra>",
                showlegend=False,
            )
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
