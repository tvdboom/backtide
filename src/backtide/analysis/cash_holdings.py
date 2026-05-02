"""Backtide.

Author: Mavs
Description: Module containing the cash-holdings-over-time chart.

"""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import _is_benchmark, _plot
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
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_cash_holdings(
    runs: RunResult | Sequence[RunResult],
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def _currency_code(value: Any) -> str:
    """Return a stable currency code from enum- or string-like values."""
    if value is None:
        return ""
    if isinstance(value, str):
        return value
    if (code := getattr(value, "value", None)) is not None:
        return str(code)
    text = str(value)
    if text.startswith("Currency."):
        return text.split(".", 1)[1]
    return text


def _currency_symbol(code: str) -> str:
    """Resolve a display symbol for an ISO currency code when available."""
    try:
        return Currency(code).symbol
    except ValueError:
        return code


def plot_cash_holdings(
    runs: RunResult | Sequence[RunResult],
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a cash-holdings-over-time chart for one or more strategy runs.

    For multi-currency strategies, one line is drawn per `(strategy, currency)`
    pair.

    """
    if not runs:
        raise ValueError("Parameter runs cannot be empty.")

    runs = _to_list(runs)

    line_idx = 0
    run_currency_count: list[int] = []
    run_single_currency: list[str] = []

    fig = go.Figure()
    for run in runs:
        if _is_benchmark(run) or not run.equity_curve:
            continue

        ts_vals = [pd.to_datetime(int(getattr(s, "timestamp", 0)), unit="s") for s in curve]
        cash_maps = [getattr(s, "cash", {}) or {} for s in curve]
        currencies = sorted({_currency_code(k) for c in cash_maps for k in c if _currency_code(k)})
        if not currencies:
            continue

        run_currency_count.append(len(currencies))
        if len(currencies) == 1:
            run_single_currency.append(currencies[0])

        for c_idx, ccy in enumerate(currencies):
            y_vals = [float(c.get(ccy, 0.0)) for c in cash_maps]

            if len(currencies) == 1:
                trace_name = run.strategy_name
                legend_group = run.strategy_name
                legend_title = None
            else:
                trace_name = ccy
                legend_group = run.strategy_name
                legend_title = run.strategy_name if c_idx == 0 else None

            fig.add_trace(
                go.Scatter(
                    x=ts_vals,
                    y=y_vals,
                    mode="lines",
                    name=trace_name,
                    legendgroup=legend_group,
                    legendgrouptitle_text=legend_title,
                    line={
                        "color": cfg.plots.palette[line_idx % len(cfg.plots.palette)],
                        "width": 2,
                    },
                    hovertemplate=(
                        f"%{{x}}<br>{ccy}: %{{y:,.2f}}<extra>{run.strategy_name}</extra>"
                    ),
                )
            )
            line_idx += 1

    ylabel = "Cash holdings"
    if run_currency_count and all(n == 1 for n in run_currency_count) and len(set(run_single_currency)) == 1:
        sym = _currency_symbol(run_single_currency[0])
        ylabel = f"Cash holdings ({sym})"

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
