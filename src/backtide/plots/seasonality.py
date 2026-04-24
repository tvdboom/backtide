"""Backtide.

Author: Mavs
Description: Module containing the seasonality heatmap function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Literal, overload

import numpy as np
import pandas as pd
import plotly.graph_objects as go

from backtide.config import get_config
from backtide.plots.utils import _check_columns, _plot

cfg = get_config()

MONTH_LABELS = [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
]


@overload
def plot_seasonality(
    data: pd.DataFrame,
    price_col: str = ...,
    symbol: str | None = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: Literal[None] = ...,
) -> go.Figure: ...
@overload
def plot_seasonality(
    data: pd.DataFrame,
    price_col: str = ...,
    symbol: str | None = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_seasonality(
    data: pd.DataFrame,
    price_col: str = "adj_close",
    symbol: str | None = None,
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = None,
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a seasonality heatmap of monthly returns.

    Aggregates daily (or other interval) returns into calendar months and
    displays a year x month grid colored by total return. Useful for
    spotting recurring seasonal patterns.

    Parameters
    ----------
    data : pd.DataFrame
        Input data containing columns `symbol`, the column specified by
        `price_col`, and `dt` with the datetime.

    price_col : str, default="adj_close"
        Column name used to compute returns.

    symbol : str | None, default=None
        Symbol to plot. If None and data contains a single symbol, that
        symbol is used. If multiple symbols are present and symbol is
        None, the first symbol alphabetically is selected.

    title : str | dict | None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, [title configuration][parameters].

    legend : str | dict | None, default=None
        Legend for the plot. Defaults to None since a colorbar is shown
        instead.

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
    - backtide.plots:plot_correlation
    - backtide.plots:plot_drawdown
    - backtide.plots:plot_returns

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.plots import plot_seasonality

    df = query_bars(["AAPL"], "1d")
    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)

    plot_seasonality(df)
    ```

    """
    _check_columns(data, ["symbol", price_col, "dt"], "plot_seasonality")

    # Select single symbol
    symbols = sorted(data["symbol"].unique())
    sym = symbol or symbols[0]
    subset = data[data["symbol"] == sym].sort_values("dt").copy()

    # Compute monthly returns
    subset["return"] = subset[price_col].pct_change() * 100
    subset["year"] = subset["dt"].dt.year
    subset["month"] = subset["dt"].dt.month

    monthly = subset.groupby(["year", "month"])["return"].sum().reset_index()
    pivot = monthly.pivot_table(index="year", columns="month", values="return")

    # Ensure all 12 months are present
    for m in range(1, 13):
        if m not in pivot.columns:
            pivot[m] = float("nan")
    pivot = pivot[sorted(pivot.columns)]

    years = [str(y) for y in pivot.index]
    months = [MONTH_LABELS[m - 1] for m in pivot.columns]
    n_years = len(years)

    # Determine text color per cell: dark text on light backgrounds, white on dark
    z_vals = pivot.to_numpy()
    z_abs_max = np.nanmax(np.abs(z_vals)) if np.any(np.isfinite(z_vals)) else 1.0
    # RdYlGn: red (negative) and green (positive) are dark, yellow (near zero) is light
    text_colors = [
        ["#333" if (pd.isna(v) or abs(v) < z_abs_max * 0.3) else "white" for v in row]
        for row in z_vals
    ]

    fig = go.Figure(
        data=go.Heatmap(
            z=z_vals,
            x=months,
            y=years,
            colorscale="RdYlGn",
            zmid=0,
            texttemplate="%{z:+.1f}%",
            textfont={"size": 11},
            colorbar={"title": {"text": "Return (%)", "font": {"size": cfg.plots.label_fontsize}}},
            hovertemplate="%{y} %{x}: %{z:+.2f}%<extra>" + sym + "</extra>",
        )
    )

    # Apply per-cell text colors via annotations (texttemplate doesn't support per-cell color)
    fig.data[0].textfont.color = None  # clear global color
    fig.data[0].texttemplate = None  # we'll use annotations instead

    # Scale annotation font size based on number of years
    _font_size = max(8, min(12, 200 // max(n_years, 1)))

    annotations = []
    for i, year in enumerate(years):
        for j, month in enumerate(months):
            val = z_vals[i][j]
            if pd.notna(val):
                annotations.append(
                    {
                        "x": month,
                        "y": year,
                        "text": f"{val:+.1f}%",
                        "showarrow": False,
                        "font": {"size": _font_size, "color": text_colors[i][j]},
                    }
                )

    fig.update_layout(
        annotations=annotations,
        yaxis={
            "ticksuffix": "  ",
            "autorange": True,
            "type": "category",
            "categoryorder": "array",
            "categoryarray": years,
        },
    )

    # Scale figure height to the number of years so rows don't overlap
    base_w, base_h = figsize or (900, 600)
    scaled_h = max(base_h, n_years * 35)

    return _plot(
        fig,
        title=title,
        legend=legend,
        figsize=(base_w, scaled_h),
        filename=filename,
        display=display,
    )
