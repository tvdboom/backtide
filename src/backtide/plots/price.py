"""Backtide.

Author: Mavs
Description: Module containing the price line chart function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pandas as pd
import plotly.graph_objects as go

from backtide.indicators import BaseIndicator
from backtide.plots.utils import PALETTE, _plot
from backtide.ui.utils import _to_pandas

# Supported price columns and their display labels.
PRICE_COLUMNS: dict[str, str] = {
    "open": "Open",
    "high": "High",
    "low": "Low",
    "close": "Close",
    "adj_close": "Adj. Close",
}


def plot_price(
    data: pd.DataFrame,
    *,
    price_col: str = "adj_close",
    indicators: dict[str, BaseIndicator] | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = None,
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a price line chart for one or multiple symbols.

    Optionally, overlay the prices with indicators.

    Parameters
    ----------
    data : pd.DataFrame
        Input data containing columns `symbol`, `open`, `high`, `low`, `close`
        and `dt` with the datetime.

    price_col : str, default="adj_close"
        Column name to plot on the y-axis. Must be one of `open`, `high`, `low`,
        `close` or `adj_close`.

    indicators : dict[str, [BaseIndicator]] or None, default=None
        Indicators to overlay on the price chart. Each dict must map a name to
        a `BaseIndicator` instance.

    title: str, dict or None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, [title configuration][parameters].

    legend: str, dict or None, default="upper left"
        Legend for the plot. See the [user guide][parameters] for an extended
        description of the choices.

        - If None: No legend is shown.
        - If str: Position to display the legend.
        - If dict: Legend configuration.

    figsize: tuple, default=(900, 600)
        Figure's size in pixels, format as (x, y).

    filename: str, Path or None, default=None
        Save the plot using this name. The type of the file depends on the
        provided name (`.html`, `.png`, `.pdf`, etc...). If `filename` has no
        file type, the plot is saved as `.html`. If `None`, the plot isn't saved.

    display: bool or None, default=True
        Whether to render the plot. If `None`, it returns the figure.

    Returns
    -------
    go.Figure or None
        The Plotly figure object. Only returned if `display=None`.

    See Also
    --------
    - backtide.plots:plot_candlestick

    Examples
    --------
    ```pycon
    from backtide.storage import query_bars
    from backtide.plots import plot_price

    df = query_bars("AAPL", "1d")
    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)
    fig = plot_price(df, display=None)
    ```

    """
    fig = go.Figure()

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        color = PALETTE[idx % len(PALETTE)]

        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=subset[price_col],
                mode="lines",
                name=symbol,
                legendgroup=symbol,
                line={"color": color, "width": 2},
            )
        )

        if indicators:
            for name, ind in indicators.items():
                values = _to_pandas(ind.compute(subset))

                if values.shape[1] == 1:
                    fig.add_trace(
                        go.Scatter(
                            x=subset["dt"],
                            y=values.iloc[:, 0],
                            mode="lines",
                            line={"color": f"rgba{color[3:-1]}, 0.7)", "width": 1.5},
                            name=name,
                            legendgroup=symbol,
                        )
                    )
                else:
                    fig.add_traces(
                        [
                            go.Scatter(
                                x=subset["dt"],
                                y=values.iloc[:, 0],
                                mode="lines",
                                line={"width": 1, "color": color},
                                hovertemplate="%{y}<extra>upper bound</extra>",
                                name=name,
                                legendgroup=symbol,
                                showlegend=False,
                            ),
                            go.Scatter(
                                x=subset["dt"],
                                y=values.iloc[:, 1],
                                mode="lines",
                                line={"width": 1, "color": color},
                                fill="tonexty",
                                fillcolor=f"rgba{color[3:-1]}, 0.2)",
                                hovertemplate="%{y}<extra>lower bound</extra>",
                                name=name,
                                legendgroup=symbol,
                                showlegend=True,
                            ),
                        ]
                    )

    return _plot(
        fig,
        groupclick="togglegroup",
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=PRICE_COLUMNS[price_col],
        figsize=figsize,
        filename=filename,
        display=display,
    )
