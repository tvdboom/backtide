"""Backtide.

Author: Mavs
Description: Module containing the returns distribution chart.

"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, overload

import numpy as np
import plotly.graph_objects as go

from backtide.analysis.utils import _check_columns, _plot, _resolve_dt
from backtide.config import get_config
from backtide.utils.utils import _to_pandas

if TYPE_CHECKING:
    from pathlib import Path

    from backtide.utils.types import DataFrameLike

cfg = get_config()


@overload
def plot_returns(
    data: DataFrameLike,
    price_col: str = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: None = ...,
) -> go.Figure: ...
@overload
def plot_returns(
    data: DataFrameLike,
    price_col: str = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_returns(
    data: DataFrameLike,
    price_col: str = "adj_close",
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a returns distribution histogram.

    Shows the distribution of period-over-period percentage returns for
    one or more symbols. Useful for visualizing volatility, skewness and
    tail risk.

    Parameters
    ----------
    data : pd.DataFrame | pl.DataFrame
        Input data containing columns `symbol`, the column specified by
        `price_col`, and `dt` with the datetime.

    price_col : str, default="adj_close"
        Column name used to compute returns.

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
    - backtide.analysis:plot_correlation
    - backtide.analysis:plot_drawdown
    - backtide.analysis:plot_price

    Examples
    --------
    ```pycon
    from backtide.storage import query_bars
    from backtide.analysis import plot_returns

    df = query_bars("AAPL", "1d")
    plot_returns(df)
    ```

    """
    data = _resolve_dt(_to_pandas(data))
    _check_columns(data, ["symbol", price_col, "dt"], "plot_returns")

    fig = go.Figure()

    # Collect per-symbol returns first so we can derive a shared x-axis range
    # that crops extreme outliers (which would otherwise compress the bulk of
    # the distribution into a single bin).
    series_by_symbol = {}
    for symbol in data["symbol"].unique():
        subset = data[data["symbol"] == symbol].sort_values("dt")
        returns = subset[price_col].pct_change().dropna().to_numpy() * 100
        if returns.size:
            series_by_symbol[str(symbol)] = returns

    if not series_by_symbol:
        return _plot(
            fig,
            title=title,
            legend=legend,
            xlabel="Return (%)",
            ylabel="Density",
            figsize=figsize,
            filename=filename,
            display=display,
        )

    # Robust symmetric range based on the 0.5-99.5 percentiles across all
    # symbols. Outliers stay in the data (so stats stay honest) but the view
    # focuses on the meaningful bulk of the distribution.
    all_returns = np.concatenate(list(series_by_symbol.values()))
    lo, hi = np.percentile(all_returns, [0.5, 99.5])
    bound = float(max(abs(lo), abs(hi)) or np.std(all_returns) * 4 or 1.0)
    bin_size = (2 * bound) / 60  # ~60 visible bins

    x_curve = np.linspace(-bound, bound, 400)

    for idx, (symbol, returns) in enumerate(series_by_symbol.items()):
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        fig.add_trace(
            go.Histogram(
                x=returns,
                name=symbol,
                legendgroup=symbol,
                marker_color=color,
                marker_line_width=0,
                opacity=0.55,
                histnorm="probability density",
                xbins={"start": -bound, "end": bound, "size": bin_size},
                hovertemplate=f"Return: %{{x:.2f}}%<br>Density: %{{y:.3f}}<extra>{symbol}</extra>",
            )
        )

        # Overlay a normal-fit curve for a smoother visual reference.
        mu = float(np.mean(returns))
        sigma = float(np.std(returns, ddof=1)) if returns.size > 1 else 0.0
        if sigma > 0:
            pdf = np.exp(-0.5 * ((x_curve - mu) / sigma) ** 2) / (sigma * np.sqrt(2 * np.pi))
            fig.add_trace(
                go.Scatter(
                    x=x_curve,
                    y=pdf,
                    mode="lines",
                    name=f"{symbol} (normal fit)",
                    legendgroup=symbol,
                    showlegend=False,
                    line={"color": color, "width": 2, "dash": "dot"},
                    hoverinfo="skip",
                )
            )

    # Reference line at zero return.
    fig.add_vline(
        x=0,
        line_width=2,
        line_dash="dash",
        line_color="rgba(120, 120, 120, 0.7)",
    )

    fig.update_layout(barmode="overlay", bargap=0.02)

    return _plot(
        fig,
        groupclick="togglegroup",
        title=title,
        legend=legend,
        xlabel="Return (%)",
        ylabel="Density",
        xlim=(-bound, bound),
        figsize=figsize,
        filename=filename,
        display=display,
    )
