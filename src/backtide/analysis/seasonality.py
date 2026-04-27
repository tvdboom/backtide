"""Backtide.

Author: Mavs
Description: Module containing the seasonality heatmap function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any, overload

import numpy as np
import pandas as pd
import plotly.graph_objects as go

from backtide.analysis.utils import DataFrameLike, _check_columns, _plot, _resolve_dt
from backtide.config import get_config
from backtide.utils.utils import _to_pandas

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
def plot_seasonality(
    data: DataFrameLike,
    price_col: str = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_seasonality(
    data: DataFrameLike,
    price_col: str = "adj_close",
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = None,
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a seasonality heatmap.

    For daily or longer intervals, aggregates returns into calendar months and
    displays a year x month grid. For intraday intervals, displays a day-of-week
    x hour grid of average returns. Useful for spotting recurring seasonal or
    intraday patterns.

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
    - backtide.analysis:plot_correlation
    - backtide.analysis:plot_drawdown
    - backtide.analysis:plot_returns

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.analysis import plot_seasonality

    # Daily interval
    df = query_bars("AAPL", "1d")
    plot_seasonality(df)

    # Intraday interval
    df = query_bars("AAPL", "1h")
    plot_seasonality(df)

    ```

    """
    data = _resolve_dt(_to_pandas(data))
    _check_columns(data, ["symbol", price_col, "dt"], "plot_seasonality")

    # Select single symbol
    if len(symbols := data["symbol"].unique()) != 1:
        raise ValueError(
            f"The plot_seasonality function requires a single symbol, "
            f"but {len(symbols)} were found ({', '.join(symbols)})."
        )

    subset = data[data["symbol"] == symbols[0]].sort_values("dt").copy()
    subset["return"] = subset[price_col].pct_change() * 100

    if subset["dt"].dt.date.duplicated(keep=False).any():
        # Averages returns across all history per (day-of-week, hour) slot.
        day_labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        subset["dow"] = subset["dt"].dt.dayofweek  # 0=Mon
        subset["hour"] = subset["dt"].dt.hour

        hourly = subset.groupby(["dow", "hour"])["return"].mean().reset_index()
        pivot = hourly.pivot_table(index="dow", columns="hour", values="return")
        pivot = pivot.reindex(columns=sorted(pivot.columns))

        # Drop days/hours with no data (e.g., weekends, overnight hours)
        pivot = pivot.dropna(axis=0, how="all")
        pivot = pivot.dropna(axis=1, how="all")

        y_labels = [day_labels[d] for d in pivot.index]
        x_labels = [f"{h:02d}:00" for h in pivot.columns]
        z_vals = pivot.to_numpy().round(2)

        n_rows = len(y_labels)
        xlabel_text = "Hour"
    else:
        subset["year"] = subset["dt"].dt.year
        subset["month"] = subset["dt"].dt.month

        monthly = subset.groupby(["year", "month"])["return"].sum().reset_index()
        pivot = monthly.pivot_table(index="year", columns="month", values="return")
        pivot = pivot.reindex(columns=sorted(pivot.columns))

        # Drop months with no data across all years
        pivot = pivot.dropna(axis=1, how="all")

        y_labels = [str(y) for y in pivot.index]
        x_labels = [MONTH_LABELS[m - 1] for m in pivot.columns]
        z_vals = pivot.to_numpy().round(2)

        n_rows = len(y_labels)
        xlabel_text = None

    # Determine text color per cell: white on dark (intense) cells, dark on light
    z_abs_max = np.nanmax(np.abs(z_vals)) if np.any(np.isfinite(z_vals)) else 1.0
    cell_text = [[f"{v:+.1f}%" if pd.notna(v) else "" for v in row] for row in z_vals]
    text_colors = [
        ["white" if (pd.notna(v) and abs(v) >= z_abs_max * 0.35) else "#222" for v in row]
        for row in z_vals
    ]

    # Scale font size based on number of rows
    font_size = max(8, min(12, 200 // max(n_rows, 1)))

    fig = go.Figure(
        data=go.Heatmap(
            x=x_labels,
            y=y_labels,
            z=z_vals,
            text=cell_text,
            texttemplate="%{text}",
            textfont={"size": font_size},
            colorscale="RdYlGn",
            zmid=0,
            colorbar={"title": {"text": "Return (%)", "font": {"size": cfg.plots.label_fontsize}}},
            hovertemplate=f"%{{x}} %{{y}}: %{{z:+.2f}}%<extra>{symbols[0]}</extra>",
            xgap=2,
            ygap=2,
        )
    )

    # Apply per-cell text colors (textfont.color doesn't support 2D, so use annotations)
    fig.data[0].texttemplate = None
    annotations = []
    for i, _ in enumerate(y_labels):
        for j, _ in enumerate(x_labels):
            val = z_vals[i][j]
            if pd.notna(val):
                annotations.append(
                    {
                        "x": j,
                        "y": i,
                        "text": cell_text[i][j],
                        "showarrow": False,
                        "font": {"size": font_size, "color": text_colors[i][j]},
                    }
                )

    fig.update_layout(
        annotations=annotations,
        plot_bgcolor="white",
        xaxis={
            "type": "category",
            "tickformat": "",
            "showgrid": False,
            "title": {"text": xlabel_text, "font_size": cfg.plots.label_fontsize}
            if xlabel_text
            else None,
        },
        yaxis={
            "ticksuffix": "  ",
            "autorange": True,
            "type": "category",
            "categoryorder": "array",
            "categoryarray": y_labels,
            "showgrid": False,
        },
    )

    # Scale figure height to the number of rows so they don't overlap
    base_w, base_h = figsize or (900, 600)
    scaled_h = max(base_h, n_rows * 35)

    return _plot(
        fig,
        title=title,
        legend=legend,
        figsize=(base_w, scaled_h),
        filename=filename,
        display=display,
    )
