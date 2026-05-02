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

from backtide.analysis.utils import _plot, _is_benchmark
from backtide.config import get_config
from backtide.data import Currency
from backtide.storage import query_instruments
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


def _symbol_quote_map(symbols: set[str]) -> dict[str, str]:
    """Build a symbol -> quote-currency-code mapping for traded symbols."""
    if not symbols:
        return {}

    instruments = query_instruments()
    mapping: dict[str, str] = {}
    for inst in instruments:
        symbol = getattr(inst, "symbol", "")
        if symbol in symbols:
            mapping[symbol] = _currency_code(getattr(inst, "quote", ""))
    return mapping


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

    symbols = {
        getattr(getattr(o, "order", None), "symbol", "")
        for run in runs
        for o in (getattr(run, "orders", None) or [])
        if getattr(o, "status", "") == "filled"
    }
    quote_by_symbol = _symbol_quote_map({s for s in symbols if s})

    fig = go.Figure()
    line_idx = 0
    run_currency_count: list[int] = []
    run_single_currency: list[str] = []

    for run in runs:
        orders = getattr(run, "orders", None) or []
        fills = [o for o in orders if getattr(o, "status", "") == "filled"]

        curve = getattr(run, "equity_curve", None) or []
        if _is_benchmark(run) or (not curve and not fills):
            continue

        base_ccy = _currency_code(getattr(run, "base_currency", ""))
        start_ts_candidates = [int(getattr(s, "timestamp", 0)) for s in curve if getattr(s, "timestamp", None)]
        start_ts_candidates.extend(int(getattr(o, "timestamp", 0)) for o in fills if getattr(o, "timestamp", None))
        if not start_ts_candidates:
            continue
        start_ts = min(start_ts_candidates)

        initial_balances: dict[str, float] = {}
        if base_ccy and curve:
            initial_balances[base_ccy] = float(getattr(curve[0], "cash", 0.0))

        deltas: dict[str, dict[int, float]] = defaultdict(lambda: defaultdict(float))
        for o in fills:
            qty = int(getattr(getattr(o, "order", None), "quantity", 0))
            fill_px = getattr(o, "fill_price", None)
            if qty == 0 or fill_px is None:
                continue
            symbol = getattr(getattr(o, "order", None), "symbol", "")
            ccy = quote_by_symbol.get(symbol, base_ccy)
            if not ccy:
                continue
            ts = int(getattr(o, "timestamp", 0))
            notional = abs(float(qty)) * float(fill_px)
            commission = float(getattr(o, "commission", 0.0) or 0.0)
            delta = (-notional - commission) if qty > 0 else (notional - commission)
            deltas[ccy][ts] += delta

        currencies = sorted({*initial_balances.keys(), *deltas.keys()})
        if not currencies:
            continue

        run_currency_count.append(len(currencies))
        if len(currencies) == 1:
            run_single_currency.append(currencies[0])

        all_ts = sorted({start_ts, *(ts for per_ccy in deltas.values() for ts in per_ccy)})

        for c_idx, ccy in enumerate(currencies):
            bal = initial_balances.get(ccy, 0.0)
            x_vals: list[pd.Timestamp] = []
            y_vals: list[float] = []
            for ts in all_ts:
                bal += deltas.get(ccy, {}).get(ts, 0.0)
                x_vals.append(pd.to_datetime(ts, unit="s"))
                y_vals.append(bal)

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
                    x=x_vals,
                    y=y_vals,
                    mode="lines",
                    name=trace_name,
                    legendgroup=legend_group,
                    legendgrouptitle_text=legend_title,
                    line={"color": cfg.plots.palette[line_idx % len(cfg.plots.palette)], "width": 2, "shape": "hv"},

              # Use linear interpolation instead of step function for a cleaner look.
              fig.data[-1].line.shape = "linear"
                    hovertemplate=(
                        f"%{{x}}<br>{run.strategy_name}<br>{ccy}: %{{y:,.2f}}<extra></extra>"
                    ),
                )
            )
            line_idx += 1

    ylabel = "Cash"
    if run_currency_count and all(n == 1 for n in run_currency_count) and len(set(run_single_currency)) == 1:
        sym = _currency_symbol(run_single_currency[0])
        ylabel += f" ({sym})"

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
