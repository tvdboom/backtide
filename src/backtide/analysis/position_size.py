"""Backtide.

Author: Mavs
Description: Module containing the position-size-over-time chart.

"""

from __future__ import annotations

from collections import defaultdict
from itertools import accumulate
from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import REFERENCE_LINE, _plot
from backtide.config import get_config

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.backtest import RunResult

cfg = get_config()


@overload
def plot_position_size(
    run: RunResult,
    *,
    symbols: list[str] | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_position_size(
    run: RunResult,
    *,
    symbols: list[str] | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_position_size(
    run: RunResult,
    *,
    symbols: list[str] | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a position-size over time chart for a single strategy run.

    Reconstructs the held quantity per symbol from the run's filled
    orders and renders one-step line per symbol. Positive values are
    long positions, negative values are short.

    Parameters
    ----------
    run : [RunResult]
        The strategy run to plot.

    symbols : list[str] | None, default=None
        List of symbols to include in the plot. If `None` or empty,
        all traded symbols are included.

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
    fig.add_hline(y=0, line=REFERENCE_LINE)

    fills = [o for o in run.orders if o.status == "filled"]

    by_symbol = defaultdict(list)
    for o in fills:
        # Filter by symbols if specified
        if symbols and o.order.symbol not in symbols:
            continue
        by_symbol[o.order.symbol].append((o.timestamp, o.order.quantity))

    for idx, (sym, rows) in enumerate(sorted(by_symbol.items())):
        rows.sort(key=lambda r: r[0])

        fig.add_trace(
            go.Scatter(
                x=pd.to_datetime([r[0] for r in rows], unit="s"),
                y=list(accumulate(q for _, q in rows)),
                mode="lines",
                name=sym,
                line={
                    "color": cfg.plots.palette[idx % len(cfg.plots.palette)],
                    "width": cfg.plots.line_width,
                    "shape": "hv",
                },
                hovertemplate="%{x}<br>Position: %{y:,}<extra>%{fullData.name}</extra>",
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
