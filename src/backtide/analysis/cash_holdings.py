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

        currencies = defaultdict(list)
        for eq in run.equity_curve:
            for ccy, amount in eq.cash.items():
                currencies[str(ccy)].append(amount)

        for idx2, (ccy, y) in enumerate(currencies.items()):
            fig.add_trace(
                go.Scatter(
                    x=[pd.to_datetime(s.timestamp, unit="s") for s in run.equity_curve],
                    y=y,
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
