"""Backtide.

Author: Mavs
Description: Module containing the cash-holdings-over-time chart.

"""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import _plot
from backtide.config import get_config
from backtide.data import Currency
from backtide.utils.utils import _to_list

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.backtest import RunResult

cfg = get_config()


@overload
def plot_cash_holdings(
    runs: RunResult | Sequence[RunResult],
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_cash_holdings(
    runs: RunResult | Sequence[RunResult],
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_cash_holdings(
    runs: RunResult | Sequence[RunResult],
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a cash-holdings-over-time chart for one or more strategy runs.

    For multi-currency strategies, one line is drawn per `(strategy, currency)`
    pair.

    Parameters
    ----------
    runs : [RunResult] | list[[RunResult]]
        The per-strategy results to plot.

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
    [Figure] | None
        The Plotly figure object. Only returned if `display=None`.

    See Also
    --------
    - backtide.analysis:plot_mae_mfe
    - backtide.analysis:plot_price
    - backtide.analysis:plot_rolling_sharpe

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_cash_holdings
    from backtide.storage import query_experiments, query_strategy_runs

    exp = query_experiments().iloc[0]
    runs = query_strategy_runs(exp.id)
    plot_cash_holdings(runs)
    ```

    """
    if not runs:
        raise ValueError("Parameter runs cannot be empty.")

    runs_l = _to_list(runs)

    dash = ("solid", "dash", "dashdot", "dot", "longdash", "longdashdot")

    fig = go.Figure()
    all_currencies = {}
    for idx, run in enumerate(runs_l):
        if run.is_benchmark or not run.equity_curve:
            continue

        # Build per-currency (timestamp, amount) pairs.
        currencies: dict[str, tuple[list, list]] = defaultdict(lambda: ([], []))
        for eq in run.equity_curve:
            ts = pd.to_datetime(eq.timestamp, unit="s")
            for ccy, amount in eq.cash.items():
                xs, ys = currencies[str(ccy)]
                xs.append(ts)
                ys.append(amount)

        for idx2, (ccy, (xs, ys)) in enumerate(currencies.items()):
            fig.add_trace(
                go.Scatter(
                    x=xs,
                    y=ys,
                    mode="lines",
                    name=run.strategy_name if len(currencies) == 1 else ccy,
                    legendgroup=run.strategy_name,
                    legendgrouptitle_text=run.strategy_name,
                    line={
                        "color": cfg.plots.palette[idx % len(cfg.plots.palette)],
                        "dash": dash[idx2 % len(dash)],
                        "width": cfg.plots.line_width,
                    },
                    hovertemplate=(
                        f"%{{x}}<br>{ccy}: %{{y:,.2f}}<extra>{run.strategy_name}</extra>"
                    ),
                )
            )

        all_currencies = all_currencies | currencies

    ylabel = "Cash"
    if len(all_currencies) == 1:
        ccy = next(iter(all_currencies))
        try:
            symbol = Currency(ccy).symbol
        except ValueError:
            symbol = ccy

        ylabel = f"Cash ({symbol})"

    fig.update_layout(hovermode="x unified")

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=ylabel,
        figsize=figsize,
        filename=filename,
        display=display,
    )
