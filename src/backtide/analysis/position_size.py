"""Backtide.

Author: Mavs
Description: Module containing the position-size-over-time chart.

"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import _plot
from backtide.config import get_config

if TYPE_CHECKING:
    from collections.abc import Iterable
    from pathlib import Path

    from backtide.backtest import RunResult

cfg = get_config()


@overload
def plot_position_size(
    run: RunResult,
    *,
    symbols: Iterable[str] | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_position_size(
    run: RunResult,
    *,
    symbols: Iterable[str] | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_position_size(
    run: RunResult,
    *,
    symbols: Iterable[str] | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a position-size-over-time chart for a single strategy run.

    Reconstructs the held quantity per symbol from the run's filled
    orders and renders one step line per symbol. Positive values are
    long positions, negative values are short.

    Parameters
    ----------
    run : [RunResult]
        The strategy run to plot.

    symbols : Iterable[str] | None, default=None
        Restrict the chart to these symbols. When `None`, every symbol
        traded by the strategy is plotted.

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
    - backtide.analysis:plot_pnl
    - backtide.analysis:plot_price
    - backtide.backtest:OrderRecord

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_position_size
    from backtide.storage import query_experiments, query_strategy_runs

    exp = query_experiments()[0]
    runs = query_strategy_runs(exp.id)
    plot_position_size(runs[0])
    ```

    """
    fig = go.Figure()

    orders = getattr(run, "orders", None) or []
    fills = [o for o in orders if getattr(o, "status", "") == "filled"]
    if not fills:
        fig.add_annotation(
            text="No filled orders to plot.",
            xref="paper",
            yref="paper",
            x=0.5,
            y=0.5,
            showarrow=False,
        )
        return _plot(
            fig,
            title=title,
            legend=legend,
            xlabel="Date",
            ylabel="Position size",
            figsize=figsize,
            filename=filename,
            display=display,
        )

    by_symbol: dict[str, list[tuple[int, int]]] = {}
    for o in fills:
        sym = getattr(o.order, "symbol", "")
        qty = int(getattr(o.order, "quantity", 0))
        ts = int(getattr(o, "timestamp", 0))
        by_symbol.setdefault(sym, []).append((ts, qty))

    if symbols is not None:
        wanted = set(symbols)
        by_symbol = {s: rows for s, rows in by_symbol.items() if s in wanted}

    fig.add_hline(y=0, line_width=1, line_dash="dot", line_color="rgba(128,128,128,0.6)")

    for idx, (sym, rows) in enumerate(sorted(by_symbol.items())):
        rows.sort(key=lambda r: r[0])
        ts_list = [r[0] for r in rows]
        qty_running: list[int] = []
        running = 0
        for _, q in rows:
            running += q
            qty_running.append(running)

        x = pd.to_datetime(ts_list, unit="s")
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]
        fig.add_trace(
            go.Scatter(
                x=x,
                y=qty_running,
                mode="lines",
                name=sym,
                line={"color": color, "width": 2, "shape": "hv"},
                hovertemplate=(
                    "<b>%{fullData.name}</b><br>%{x|%Y-%m-%d %H:%M}<br>"
                    "Position: %{y:+,.0f}<extra></extra>"
                ),
            )
        )

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel="Position size",
        figsize=figsize,
        filename=filename,
        display=display,
    )
