"""Backtide.

Author: Mavs
Description: Module containing the PnL chart function.

"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, overload

import pandas as pd
import plotly.graph_objects as go
from plotly.subplots import make_subplots

from backtide.analysis.utils import BENCHMARK_LINE, _is_benchmark, _plot, _resolve_run_currency
from backtide.config import get_config
from backtide.utils.utils import _format_price, _to_list

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.backtest import RunResult
    from backtide.core.data import Currency

cfg = get_config()


@overload
def plot_pnl(
    runs: list[RunResult],
    *,
    normalize: bool = ...,
    drawdown: bool = ...,
    currency: str | Currency | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_pnl(
    runs: list[RunResult],
    *,
    normalize: bool = ...,
    drawdown: bool = ...,
    currency: str | Currency | None = ...,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_pnl(
    runs: RunResult | list[RunResult],
    *,
    normalize: bool = False,
    drawdown: bool = True,
    currency: str | Currency | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a PnL-over-time chart for one or more strategy runs.

    Each line tracks a strategy's running profit & loss (current equity minus
    the starting equity) in the base currency. When `normalize=True`, PnL is
    normalized to a percentage of the starting equity instead, which makes
    strategies with different initial cash visually comparable. When
    `drawdown=True` (the default), a second panel is rendered below the
    PnL curve showing each strategy's running drawdown on a shared x-axis.
    The benchmark run is shown as a dashed gray line, if provided.

    Parameters
    ----------
    runs : [RunResult] | list[[RunResult]]
        The per-strategy results to plot, typically obtained from
        `query_strategy_runs` or directly from [`ExperimentResult`].

    normalize : bool, default=False
        - If False, plot absolute PnL (`equity - initial_equity`).
        - If True, plot relative PnL (`(equity / initial_equity - 1) * 100`)
          as a percentage.

    drawdown : bool, default=True
        Whether to render a drawdown panel underneath the PnL curve. When
        True, the figure has two stacked rows (PnL on top, drawdown below)
        sharing the same x-axis. Set to False for a single-panel chart.

    currency : str | [Currency] | None, default=None
        Currency used to format the y-axis label and hover tooltips. When
        `None`, the run's own `base_currency` (set by the engine from
        `ExperimentConfig.portfolio.base_currency`) is used. Pass an
        explicit value to override. Ignored when `normalize=True`.

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
    - backtide.analysis:plot_rolling_returns
    - backtide.analysis:plot_trade_pnl
    - backtide.backtest:RunResult

    Examples
    --------
    ```pycon
    from backtide.analysis import plot_pnl
    from backtide.storage import query_strategy_runs, query_experiments

    exp = query_experiments()[0]
    runs = query_strategy_runs(exp.id)

    # Absolute PnL
    plot_pnl(runs)

    # Normalized values
    plot_pnl(runs, normalize=True)
    ```

    """
    if not runs:
        raise ValueError("Parameter runs cannot be empty.")

    ccy = None if normalize else _resolve_run_currency(currency, runs)

    if drawdown:
        fig = make_subplots(
            rows=2,
            cols=1,
            shared_xaxes=True,
            row_heights=[0.7, 0.3],
            vertical_spacing=0.04,
        )
    else:
        fig = go.Figure()

    for idx, run in enumerate(_to_list(runs)):
        if not (curve := getattr(run, "equity_curve", None)):
            continue

        ts = pd.to_datetime([s.timestamp for s in curve], unit="s")
        equity = [float(s.equity) for s in curve]
        base = next((e for e in equity if e), 0.0)  # First non-zero equity
        if base == 0.0:
            continue

        if normalize:
            y = [(e / base - 1.0) * 100.0 for e in equity]
        else:
            y = [e - base for e in equity]

        if is_benchmark := _is_benchmark(run.strategy_name):
            line = BENCHMARK_LINE
        else:
            color = cfg.plots.palette[idx % len(cfg.plots.palette)]
            line = {"color": color, "width": 2}

        equity_trace = go.Scatter(
            x=ts,
            y=y,
            mode="lines",
            name=run.strategy_name,
            line=line,
            legendgroup=run.strategy_name,
            showlegend=not is_benchmark,
            customdata=[_format_price(v, signed=True, currency=ccy) for v in y],
            hovertemplate=(
                f"%{{x}}<br>PnL: {'%{y:+.2f}' if normalize else '%{customdata}'}"
                "<extra>%{fullData.name}</extra>"
            ),
        )

        if drawdown:
            fig.add_trace(equity_trace, row=1, col=1)

            # Drawdown is plotted in the same unit family as the top panel.
            dd_y = []
            peak = equity[0]
            for e in equity:
                peak = max(peak, e)
                dd_frac = ((e - peak) / peak) if peak else 0.0
                dd_y.append(dd_frac * 100.0 if normalize else (e - peak))

            fig.add_trace(
                go.Scatter(
                    x=ts,
                    y=dd_y,
                    mode="lines",
                    name=run.strategy_name,
                    line=line,
                    legendgroup=run.strategy_name,
                    showlegend=False,
                    customdata=[_format_price(v, signed=True, currency=ccy) for v in dd_y],
                    hovertemplate=(
                        f"%{{x}}<br>Drawdown: {'%{y:+.2f}' if normalize else '%{customdata}'}"
                        "<extra>%{fullData.name}</extra>"
                    ),
                ),
                row=2,
                col=1,
            )
        else:
            fig.add_trace(equity_trace)

    # Zero reference line: the break-even level for absolute PnL and
    # the 0 % return level for relative PnL — both useful anchors.
    if drawdown:
        fig.add_hline(
            y=0,
            line_width=1,
            line_dash="dot",
            line_color="rgba(128,128,128,0.6)",
            row=1,
            col=1,
        )

        fig.update_xaxes(title_text="Date", row=2, col=1)
        fig.update_yaxes(
            title_text=f"Drawdown{' (%)' if normalize else (f' ({ccy.symbol})' if ccy else '')}",
            row=2,
            col=1,
        )
    else:
        fig.add_hline(y=0, line_width=1, line_dash="dot", line_color="rgba(128,128,128,0.6)")

    return _plot(
        fig,
        groupclick="togglegroup",
        title=title,
        legend=legend,
        xlabel=None if drawdown else "Date",
        ylabel="Return (%)" if normalize else (f"PnL ({ccy.symbol})" if ccy else "PnL"),
        figsize=figsize,
        filename=filename,
        display=display,
    )
