"""Backtide.

Author: Mavs
Description: Module containing the per-trade PnL scatter chart.

"""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import _is_benchmark, _plot, _resolve_runs_currency
from backtide.config import get_config
from backtide.utils.utils import _format_price, _to_list

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.backtest import RunResult

cfg = get_config()


@overload
def plot_trade_pnl(
    runs: RunResult | Sequence[RunResult],
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_trade_pnl(
    runs: RunResult | Sequence[RunResult],
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_trade_pnl(
    runs: RunResult | Sequence[RunResult],
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a per-trade PnL over time plot for one or more strategy runs.

    Each marker represents a single closed trade plotted at its exit
    timestamp. Useful to spot clustering of wins/losses, regime changes,
    and to compare the timing of returns across strategies.

    Parameters
    ----------
    runs : [RunResult] | list[[RunResult]]
        The per-strategy results to plot. Runs without trades are skipped.

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
    - backtide.analysis:plot_pnl
    - backtide.analysis:plot_pnl_histogram
    - backtide.analysis:plot_trade_duration

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_trade_pnl
    from backtide.storage import query_experiments, query_strategy_runs

    exp = query_experiments()[0]
    runs = query_strategy_runs(exp.id)
    plot_trade_pnl(runs)
    ```

    """
    if not runs:
        raise ValueError("Parameter runs cannot be empty.")

    runs = _to_list(runs)
    ccy = _resolve_runs_currency(runs)

    fig = go.Figure()
    for idx, run in enumerate(runs):
        if _is_benchmark(run) or not run.trades:
            continue

        exit_ts, pnls, symbols = [], [], []
        for t in run.trades:
            exit_ts.append(t.exit_ts)
            pnls.append(t.pnl)
            symbols.append(t.symbol)

        ts = pd.to_datetime(exit_ts, unit="s")

        fig.add_trace(
            go.Scatter(
                x=ts,
                y=pnls,
                mode="markers",
                name=run.strategy_name,
                marker={
                    "color": cfg.plots.palette[idx % len(cfg.plots.palette)],
                    "size": cfg.plots.marker_size,
                    "line": {"width": 0},
                },
                customdata=[
                    [sym, _format_price(p, signed=True, currency=ccy)]
                    for sym, p in zip(symbols, pnls, strict=True)
                ],
                hovertemplate=(
                    "%{x}<br>PnL: %{customdata[1]}<br>Symbol: %{customdata[0]}"
                    "<extra>%{fullData.name}</extra>"
                ),
            )
        )

    fig.add_hline(y=0, line_width=cfg.plots.line_width / 2, line_dash="dot", line_color="rgba(128,128,128,0.6)")

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Exit date",
        ylabel=f"Trade PnL ({ccy.symbol})" if ccy else "Trade PnL",
        figsize=figsize,
        filename=filename,
        display=display,
    )
