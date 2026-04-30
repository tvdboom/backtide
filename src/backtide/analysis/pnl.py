"""Backtide.

Author: Mavs
Description: Module containing the PnL chart function.

"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import _plot
from backtide.config import get_config
from backtide.utils.constants import BENCHMARK_NAME

if TYPE_CHECKING:
    from backtide.backtest import StrategyRunResult

cfg = get_config()


@overload
def plot_pnl(
    runs: list[StrategyRunResult],
    *,
    relative: bool = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_pnl(
    runs: list[StrategyRunResult],
    *,
    relative: bool = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_pnl(
    runs: list[StrategyRunResult],
    *,
    relative: bool = False,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a PnL-over-time chart for an experiment.

    Plots one line per strategy run, sharing a common time axis. Each
    line starts at zero and tracks the running profit & loss (current
    equity minus the strategy's starting equity). When `relative=True`,
    PnL is normalised to a percentage of the starting equity instead,
    which makes strategies with very different initial capital
    visually comparable.

    Useful as the headline visual for an experiment: at a glance the
    user sees which strategy compounds best, how each one drawdowns,
    and how the user strategies stack up against the benchmark.

    Parameters
    ----------
    runs : list[[StrategyRunResult]]
        The per-strategy results to plot, typically obtained from
        `query_experiment_strategies` or directly from
        `ExperimentResult.strategies`. Runs without an equity curve
        (e.g. failed strategies) are silently skipped.

    relative : bool, default=False
        - If False, plot absolute PnL (`equity - initial_equity`) in the
          base currency.
        - If True, plot relative PnL (`(equity / initial_equity - 1) * 100`)
          as a percentage. Lets strategies with different starting
          capitals share a single y-axis.

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
    - backtide.analysis:plot_drawdown
    - backtide.storage:query_experiment_strategies
    - backtide.backtest:StrategyRunResult

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_pnl
    from backtide.storage import query_experiment_strategies, query_experiments

    exp = query_experiments()[0]
    runs = query_experiment_strategies(exp.id)
    plot_pnl(runs, relative=True)
    ```

    """
    fig = go.Figure()
    plotted = 0
    for idx, run in enumerate(runs):
        curve = getattr(run, "equity_curve", None)
        if not curve:
            continue

        ts = pd.to_datetime([s.timestamp for s in curve], unit="s")
        equity = [float(s.equity) for s in curve]
        base = next((e for e in equity if e), 0.0)  # first non-zero equity
        if base == 0.0:
            continue

        if relative:
            y = [(e / base - 1.0) * 100.0 for e in equity]
        else:
            y = [e - base for e in equity]

        # Distinguish the benchmark with a dashed line so it stands out
        # from the user strategies on the same axis.
        is_benchmark = bool(BENCHMARK_NAME.match(run.strategy_name))
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]
        line: dict[str, Any] = {"color": color, "width": 2}
        if is_benchmark:
            line["dash"] = "dash"

        fig.add_trace(
            go.Scatter(
                x=ts,
                y=y,
                mode="lines",
                name=run.strategy_name,
                line=line,
                hovertemplate=(
                    "<b>%{fullData.name}</b><br>%{x|%Y-%m-%d}<br>"
                    + ("%{y:+.2f}%" if relative else "%{y:+,.2f}")
                    + "<extra></extra>"
                ),
            )
        )
        plotted += 1

    if plotted == 0:
        # Still produce a (blank) figure so callers get a deterministic
        # return type rather than having to guard against None.
        fig.add_annotation(
            text="No equity data to plot.",
            xref="paper",
            yref="paper",
            x=0.5,
            y=0.5,
            showarrow=False,
        )

    # Zero reference line: the break-even level for absolute PnL and
    # the 0 % return level for relative PnL — both useful anchors.
    fig.add_hline(y=0, line_width=1, line_dash="dot", line_color="rgba(128,128,128,0.6)")

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel="Return (%)" if relative else "PnL",
        figsize=figsize,
        filename=filename,
        display=display,
    )

