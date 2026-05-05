"""Backtide.

Author: Mavs
Description: Module containing the rolling Sharpe-ratio chart.

"""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, overload

import numpy as np
import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import BENCHMARK_LINE, GREEN, RED, _is_benchmark, _plot
from backtide.config import get_config
from backtide.utils.utils import _to_list

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.backtest import RunResult

cfg = get_config()


@overload
def plot_rolling_sharpe(
    runs: RunResult | Sequence[RunResult],
    window: int = ...,
    periods_per_year: int = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_rolling_sharpe(
    runs: RunResult | Sequence[RunResult],
    window: int = ...,
    periods_per_year: int = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_rolling_sharpe(
    runs: RunResult | Sequence[RunResult],
    window: int = 60,
    periods_per_year: int = 252,
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a rolling Sharpe-ratio chart for one or more strategy runs.

    Each line plots the Sharpe ratio over a trailing `window` of samples,
    annualized by `periods_per_year`. Useful to spot when a strategy's
    risk-adjusted edge erodes or strengthens over time.

    Parameters
    ----------
    runs : [RunResult] | list[[RunResult]]
        The per-strategy results to plot.

    window : int, default=60
        Number of samples used in the trailing window.

    periods_per_year : int, default=252
        Annualization factor (number of periods/bars per year). Use 252 for
        daily intervals (or 365 for crypto) and 52 for weekly.

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
    - backtide.analysis:plot_rolling_returns
    - backtide.backtest:RunResult

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_rolling_sharpe
    from backtide.storage import query_experiments, query_strategy_runs

    exp = query_experiments()[0]
    runs = query_strategy_runs(exp.id)
    plot_rolling_sharpe(runs, window=90)
    ```

    """
    if not runs:
        raise ValueError("Parameter runs cannot be empty.")

    runs_l = _to_list(runs)
    scale = float(np.sqrt(periods_per_year))

    fig = go.Figure()
    for idx, run in enumerate(runs_l):
        curve = run.equity_curve
        if not curve or len(curve) <= window:
            continue

        equity = pd.Series(
            [s.equity for s in curve],
            index=pd.to_datetime([s.timestamp for s in curve], unit="s"),
        )

        roll = equity.pct_change().rolling(window)
        sharpe = (roll.mean() / roll.std()) * scale
        sharpe = sharpe.replace([np.inf, -np.inf], np.nan).dropna()
        if sharpe.empty:
            continue

        if is_benchmark := _is_benchmark(run):
            line = BENCHMARK_LINE
        else:
            line = {
                "color": cfg.plots.palette[idx % len(cfg.plots.palette)],
                "width": cfg.plots.line_width,
            }

        fig.add_trace(
            go.Scatter(
                x=sharpe.index,
                y=sharpe.values,
                mode="lines",
                name=run.strategy_name,
                line=line,
                hovertemplate="%{x}<br>Sharpe ratio: %{y:.2f}<extra>%{fullData.name}</extra>",
                showlegend=not is_benchmark,
            )
        )

    fig.add_hline(y=0, line_width=cfg.plots.line_width / 2, line_dash="dash", line_color=RED)
    fig.add_hline(y=1, line_width=cfg.plots.line_width / 2, line_dash="dash", line_color=GREEN)

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel="Rolling Sharpe",
        figsize=figsize,
        filename=filename,
        display=display,
    )
