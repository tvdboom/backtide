"""Backtide.

Author: Mavs
Description: Module containing the trade-duration histogram chart.

"""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, Literal, overload

import numpy as np
import plotly.graph_objects as go

from backtide.analysis.utils import _is_benchmark, _plot
from backtide.config import get_config
from backtide.utils.utils import _to_list

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.backtest import RunResult


cfg = get_config()


@overload
def plot_trade_duration(
    runs: RunResult | Sequence[RunResult],
    *,
    bins: int | None = ...,
    unit: Literal["auto", "minutes", "hours", "days"] = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_trade_duration(
    runs: RunResult | Sequence[RunResult],
    *,
    bins: int | None = ...,
    unit: Literal["auto", "minutes", "hours", "days"] = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_trade_duration(
    runs: RunResult | Sequence[RunResult],
    *,
    bins: int | None = None,
    unit: Literal["auto", "minutes", "hours", "days"] = "auto",
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper right",
    figsize: tuple[int, int] = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a histogram of trade durations for one or more strategy runs.

    Each strategy gets its own translucent histogram overlaid on the same
    axes — useful to compare how long each strategy holds positions.

    Parameters
    ----------
    runs : [RunResult] | list[[RunResult]]
        The per-strategy results to plot. Runs without trades are skipped.

    bins : int | None, default=None
        Number of histogram bins. If `None`, Plotly's default binning algorithm
        is used.

    unit : "auto" | "minutes" | "hours" | "days", default="auto"
        Time unit used on the x-axis. When `auto`, the unit is picked from the
        median trade duration across all runs.

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
    - backtide.analysis:plot_pnl_histogram
    - backtide.analysis:plot_trade_pnl
    - backtide.backtest:Trade

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_trade_duration
    from backtide.storage import query_experiments, query_strategy_runs

    exp = query_experiments()[0]
    runs = query_strategy_runs(exp.id)
    plot_trade_duration(runs)
    ```

    """
    if not runs:
        raise ValueError("Parameter runs cannot be empty.")

    runs = _to_list(runs)

    durations = {}
    for run in runs:
        if _is_benchmark(run) or not run.trades:
            continue

        durations[run.strategy_name] = [t.exit_ts - t.entry_ts for t in run.trades]

    unit = unit.lower()
    if unit == "auto":
        median_secs = np.median([d for v in durations.values() for d in v])
        if median_secs >= 2 * 86_400:
            unit = "days"
        elif median_secs >= 2 * 3_600:
            unit = "hours"
        else:
            unit = "minutes"

    factor = {"minutes": 60.0, "hours": 3_600.0, "days": 86_400.0}[unit]

    fig = go.Figure()
    for idx, run in enumerate(runs):
        if (secs := durations.get(run.strategy_name)) is None:
            continue

        fig.add_trace(
            go.Histogram(
                x=[s / factor for s in secs],
                nbinsx=bins,
                name=run.strategy_name,
                marker_color=cfg.plots.palette[idx % len(cfg.plots.palette)],
                opacity=0.55,
                hovertemplate=(
                    f"Duration: %{{x:,}}{unit[0]}<br>Trades: %{{y}}"
                    "<extra>%{fullData.name}</extra>"
                ),
            )
        )

    fig.update_layout(barmode="overlay")

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel=f"Trade duration ({unit})",
        ylabel="Trade count",
        figsize=figsize,
        filename=filename,
        display=display,
    )
