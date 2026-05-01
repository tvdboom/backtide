"""Backtide.

Author: Mavs
Description: Module containing the trade PnL histogram chart.

"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Any, overload

import plotly.graph_objects as go

from backtide.analysis.utils import _is_benchmark, _plot, _resolve_run_currency
from backtide.config import get_config
from backtide.core.data import Currency

if TYPE_CHECKING:
    from backtide.backtest import StrategyRunResult

cfg = get_config()


@overload
def plot_pnl_histogram(
    runs: list[StrategyRunResult],
    *,
    bins: int = ...,
    currency: str | Currency | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_pnl_histogram(
    runs: list[StrategyRunResult],
    *,
    bins: int = ...,
    currency: str | Currency | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_pnl_histogram(
    runs: StrategyRunResult | list[StrategyRunResult],
    *,
    bins: int = 40,
    currency: str | Currency | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper right",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a histogram of per-trade PnL for one or more strategy runs.

    Each strategy gets its own translucent histogram overlaid on the same
    axes, so the shape and skew of the trade-PnL distribution can be
    compared at a glance.

    Parameters
    ----------
    runs : [StrategyRunResult] | list[[StrategyRunResult]]
        The per-strategy results to plot. Runs without trades are skipped.

    bins : int, default=40
        Number of histogram bins.

    currency : str | [Currency] | None, default=None
        Currency used to label the PnL axis. When `None`, the run's own
        `base_currency` (set by the engine from
        `ExperimentConfig.portfolio.base_currency`) is used.

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
    ccy = _resolve_run_currency(currency, runs)
    sym = ccy.symbol if ccy else ""
    fig = go.Figure()
    plotted = 0
    for idx, run in enumerate(runs):
        # Per-trade view; the benchmark has no real trades, skip it.
        if _is_benchmark(run.strategy_name):
            continue
        trades = getattr(run, "trades", None) or []
        if not trades:
            continue

        pnls = [float(t.pnl) for t in trades]
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]
        fig.add_trace(
            go.Histogram(
                x=pnls,
                nbinsx=bins,
                name=run.strategy_name,
                marker_color=color,
                opacity=0.55,
                hovertemplate=(
                    "<b>%{fullData.name}</b><br>"
                    f"PnL: %{{x:+,.2f}} {sym}<br>"
                    "Trades: %{y}<extra></extra>"
                ),
            )
        )
        plotted += 1

    if plotted == 0:
        fig.add_annotation(
            text="No trades to plot.",
            xref="paper",
            yref="paper",
            x=0.5,
            y=0.5,
            showarrow=False,
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

