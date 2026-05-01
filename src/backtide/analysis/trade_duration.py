"""Backtide.

Author: Mavs
Description: Module containing the trade-duration histogram chart.

"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Any, Literal, overload

import numpy as np
import plotly.graph_objects as go

from backtide.analysis.utils import _is_benchmark, _plot
from backtide.config import get_config

if TYPE_CHECKING:
    from backtide.backtest import StrategyRunResult

cfg = get_config()

UnitName = Literal["auto", "minutes", "hours", "days"]
_UNIT_FACTORS: dict[str, float] = {
    "minutes": 60.0,
    "hours": 3_600.0,
    "days": 86_400.0,
}


def _pick_unit(median_seconds: float) -> str:
    """Select an axis unit based on the median trade duration."""
    if median_seconds >= 2 * 86_400:
        return "days"
    if median_seconds >= 2 * 3_600:
        return "hours"
    return "minutes"


@overload
def plot_trade_duration(
    runs: list[StrategyRunResult],
    *,
    bins: int = ...,
    unit: UnitName = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_trade_duration(
    runs: list[StrategyRunResult],
    *,
    bins: int = ...,
    unit: UnitName = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_trade_duration(
    runs: StrategyRunResult | list[StrategyRunResult],
    *,
    bins: int = 40,
    unit: UnitName = "auto",
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper right",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a histogram of trade durations for one or more strategy runs.

    Each strategy gets its own translucent histogram overlaid on the same
    axes — useful to compare how long each strategy holds positions.

    Parameters
    ----------
    runs : [StrategyRunResult] | list[[StrategyRunResult]]
        The per-strategy results to plot. Runs without trades are skipped.

    bins : int, default=40
        Number of histogram bins.

    unit : "auto" | "minutes" | "hours" | "days", default="auto"
        Time unit used on the x-axis. When `"auto"`, the unit is picked
        from the median trade duration across all runs.

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
    durations: dict[str, list[float]] = {}
    for run in runs:
        # Per-trade view; the benchmark has no real trades, skip it.
        if _is_benchmark(run.strategy_name):
            continue
        trades = getattr(run, "trades", None) or []
        if not trades:
            continue
        durations[run.strategy_name] = [
            float(int(t.exit_ts) - int(t.entry_ts)) for t in trades
        ]

    if not durations:
        fig = go.Figure()
        fig.add_annotation(
            text="No trades to plot.",
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
            xlabel="Trade duration",
            ylabel="Trade count",
            figsize=figsize,
            filename=filename,
            display=display,
        )

    if unit == "auto":
        all_secs = [d for v in durations.values() for d in v]
        unit = _pick_unit(float(np.median(all_secs))) if all_secs else "hours"  # ty: ignore[invalid-assignment]

    factor = _UNIT_FACTORS[unit]

    fig = go.Figure()
    runs_iter = list(runs)
    for idx, run in enumerate(runs_iter):
        if (secs := durations.get(run.strategy_name)) is None:
            continue
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]
        fig.add_trace(
            go.Histogram(
                x=[s / factor for s in secs],
                nbinsx=bins,
                name=run.strategy_name,
                marker_color=color,
                opacity=0.55,
                hovertemplate=(
                    f"<b>%{{fullData.name}}</b><br>Duration: %{{x:,.2f}} {unit}<br>"
                    "Trades: %{y}<extra></extra>"
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

