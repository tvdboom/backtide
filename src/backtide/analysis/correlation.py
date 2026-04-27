"""Backtide.

Author: Mavs
Description: Module containing the correlation heatmap function.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any, overload

import plotly.graph_objects as go

from backtide.analysis.utils import DataFrameLike, _check_columns, _plot, _resolve_dt
from backtide.config import get_config
from backtide.utils.utils import _to_pandas

cfg = get_config()


@overload
def plot_correlation(
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
def plot_correlation(
    data: DataFrameLike,
    price_col: str = ...,
    *,
    title: str | dict[str, Any] | None = ...,
    legend: str | dict[str, Any] | None = ...,
    figsize: tuple[int, int] | None = ...,
    filename: str | Path | None = ...,
    display: bool = ...,
) -> None: ...


def plot_correlation(
    data: DataFrameLike,
    price_col: str = "adj_close",
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = None,
    figsize: tuple[int, int] | None = (700, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a correlation heatmap.

    Computes pairwise Pearson correlation of period-over-period returns
    across symbols and displays the result as a heatmap. Requires data
    with at least two symbols.

    Parameters
    ----------
    data : pd.DataFrame | pl.DataFrame
        Input data containing columns `symbol`, the column specified by
        `price_col`, and `dt` with the datetime.

    price_col : str, default="adj_close"
        Column name used to compute returns for correlation.

    title : str | dict | None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, [title configuration][parameters].

    legend : str | dict | None, default=None
        Legend for the plot. Defaults to None since a colorbar is shown
        instead.

    figsize : tuple[int, int] | None, default=(700, 600)
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
    - backtide.analysis:plot_drawdown
    - backtide.analysis:plot_price
    - backtide.analysis:plot_returns

    Examples
    --------
    ```pycon
    from backtide.storage import query_bars
    from backtide.analysis import plot_correlation

    df = query_bars(["AAPL", "MSFT", "GOOG"], "1d")
    plot_correlation(df)
    ```

    """
    data = _resolve_dt(_to_pandas(data))
    _check_columns(data, ["symbol", price_col, "dt"], "plot_correlation")

    # Pivot to get one column per symbol, compute returns, then correlate
    pivot = data.pivot_table(index="dt", columns="symbol", values=price_col)
    returns = pivot.pct_change().dropna(how="all")
    corr = returns.corr()

    # Annotate cells with correlation values
    annotations = []
    for i, row_label in enumerate(corr.index):
        for j, col_label in enumerate(corr.columns):
            annotations.append(
                {
                    "x": col_label,
                    "y": row_label,
                    "text": f"{corr.iloc[i, j]:.2f}",
                    "showarrow": False,
                    "font": {"size": cfg.plots.label_fontsize, "color": "white"},
                }
            )

    fig = go.Figure(
        data=go.Heatmap(
            z=corr.values,
            x=corr.columns.tolist(),
            y=corr.index.tolist(),
            colorscale="Blues",
            zmin=-1,
            zmax=1,
            colorbar={
                "title": {"text": "Correlation", "font": {"size": cfg.plots.label_fontsize}}
            },
        )
    )

    fig.update_layout(annotations=annotations, yaxis={"ticksuffix": "  "})

    return _plot(
        fig,
        title=title,
        legend=legend,
        figsize=figsize,
        filename=filename,
        display=display,
    )
