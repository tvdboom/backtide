"""Backtide.

Author: Mavs
Description: Module containing the rolling returns chart.

"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import BENCHMARK_LINE, _is_benchmark, _plot
from backtide.config import get_config

if TYPE_CHECKING:
    from backtide.backtest import StrategyRunResult

cfg = get_config()


@overload
def plot_rolling_returns(
    runs: list[StrategyRunResult],
    *,
    window: int = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_rolling_returns(
    runs: list[StrategyRunResult],
    *,
    window: int = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_rolling_returns(
    runs: StrategyRunResult | list[StrategyRunResult],
    *,
    window: int = 30,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a rolling-return chart for one or more strategy runs.

    Each line plots the compounded return over a trailing `window` of
    equity samples — useful to compare how strategies behave over short
    horizons rather than the cumulative view in [`plot_pnl`].

    Parameters
    ----------
    runs : [StrategyRunResult] | list[[StrategyRunResult]]
        The per-strategy results to plot.

    window : int, default=30
        Number of equity samples used in the trailing return window.
        For daily samples this corresponds to a ~1-month horizon.

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
    - backtide.analysis:plot_rolling_sharpe
    - backtide.backtest:StrategyRunResult

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_rolling_returns
    from backtide.storage import query_experiments, query_strategy_runs

    exp = query_experiments()[0]
    runs = query_strategy_runs(exp.id)
    plot_rolling_returns(runs, window=60)
    ```

    """
    fig = go.Figure()
    plotted = 0
    for idx, run in enumerate(runs):
        curve = getattr(run, "equity_curve", None)
        if not curve or len(curve) <= window:
            continue

        equity = pd.Series(
            [float(s.equity) for s in curve],
            index=pd.to_datetime([s.timestamp for s in curve], unit="s"),
        )
        # Compounded return over the trailing window: (E_t / E_{t-w}) - 1.
        rolling_ret = ((equity / equity.shift(window)) - 1.0) * 100.0
        rolling_ret = rolling_ret.dropna()
        if rolling_ret.empty:
            continue

        is_benchmark = _is_benchmark(run.strategy_name)
        if is_benchmark:
            line: dict[str, Any] = BENCHMARK_LINE
        else:
            color = cfg.plots.palette[idx % len(cfg.plots.palette)]
            line = {"color": color, "width": 2}

        fig.add_trace(
            go.Scatter(
                x=rolling_ret.index,
                y=rolling_ret.values,
                mode="lines",
                name=run.strategy_name,
                line=line,
                showlegend=not is_benchmark,
                hovertemplate=(
                    "<b>%{fullData.name}</b><br>%{x|%Y-%m-%d}<br>"
                    "%{y:+.2f}%<extra></extra>"
                ),
            )
        )
        plotted += 1

    if plotted == 0:
        fig.add_annotation(
            text="Not enough equity data to compute rolling returns.",
            xref="paper",
            yref="paper",
            x=0.5,
            y=0.5,
            showarrow=False,
        )

    fig.add_hline(y=0, line_width=1, line_dash="dot", line_color="rgba(128,128,128,0.6)")

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=f"Rolling return ({window}) (%)",
        figsize=figsize,
        filename=filename,
        display=display,
    )

