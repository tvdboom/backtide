"""Backtide.

Author: Mavs
Description: Module containing the trade PnL histogram chart.

"""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, overload

import plotly.graph_objects as go

from backtide.analysis.utils import _is_benchmark, _plot, _resolve_runs_currency
from backtide.config import get_config
from backtide.utils.utils import _format_price, _to_list

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.backtest import RunResult


cfg = get_config()


@overload
def plot_pnl_histogram(
    runs: RunResult | Sequence[RunResult],
    *,
    bins: int | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_pnl_histogram(
    runs: RunResult | Sequence[RunResult],
    *,
    bins: int | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_pnl_histogram(
    runs: RunResult | Sequence[RunResult],
    *,
    bins: int | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper right",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a histogram of per-trade PnL for one or more strategy runs.

    Each strategy plots its own histogram overlaid on the same axes, so the
    shape and skew of the trade-PnL distribution can be easily compared.

    Parameters
    ----------
    runs : [RunResult] | list[[RunResult]]
        The per-strategy results to plot.

    bins : int | None, default=None
        Number of histogram bins. If `None`, Plotly's default binning algorithm
        is used.

    title : str | dict | None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, [title configuration][parameters].

    legend : str | dict | None, default="upper right"
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
    - backtide.analysis:plot_trade_duration
    - backtide.analysis:plot_trade_pnl

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_pnl_histogram
    from backtide.storage import query_experiments, query_strategy_runs

    exp = query_experiments()[0]
    runs = query_strategy_runs(exp.id)
    plot_pnl_histogram(runs)
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

        fig.add_trace(
            go.Histogram(
                x=(x := [t.pnl for t in run.trades]),
                nbinsx=bins,
                name=run.strategy_name,
                marker_color=cfg.plots.palette[idx % len(cfg.plots.palette)],
                opacity=0.55,
                customdata=[_format_price(v, signed=True, currency=ccy) for v in x],
                hovertemplate="Trades: %{y}<br>PnL: %{customdata}<extra>%{fullData.name}</extra>",
            )
        )

    fig.update_layout(barmode="overlay")
    fig.add_vline(x=0, line_width=1, line_dash="dot", line_color="rgba(128,128,128,0.6)")

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel=f"Trade PnL ({ccy.symbol})" if ccy else "Trade PnL",
        ylabel="Trade count",
        figsize=figsize,
        filename=filename,
        display=display,
    )
