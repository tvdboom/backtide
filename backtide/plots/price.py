"""Backtide.

Author: Mavs
Description: Module containing the price line chart function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pandas as pd
import plotly.graph_objects as go

from backtide.plots.utils import PALETTE, _plot

# Supported price columns and their display labels.
PRICE_COLUMNS: dict[str, str] = {
    "open": "Open",
    "high": "High",
    "low": "Low",
    "close": "Close",
    "adj_close": "Adj. Close",
}


def _hex_to_rgba(hex_color: str, alpha: float) -> str:
    """Convert a hex color to an rgba string."""
    h = hex_color.lstrip("#")
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    return f"rgba({r},{g},{b},{alpha})"


def _add_indicator_traces(
    fig: go.Figure,
    result: dict,
    name: str,
    color: str,
    symbol: str,
    x: pd.Series,
) -> None:
    """Add indicator output traces to a plotly figure for a single symbol."""
    if not result or not isinstance(result, dict):
        return

    keys = list(result.keys())

    # Two keys ending in _upper/_lower → range fill
    upper_keys = [k for k in keys if k.endswith("_upper")]
    lower_keys = [k for k in keys if k.endswith("_lower")]

    if len(upper_keys) == 1 and len(lower_keys) == 1:
        upper = result[upper_keys[0]]
        lower = result[lower_keys[0]]

        fig.add_trace(
            go.Scatter(
                x=x,
                y=upper,
                mode="lines",
                line={"width": 0},
                name=f"{name} upper ({symbol})",
                showlegend=False,
            )
        )
        fig.add_trace(
            go.Scatter(
                x=x,
                y=lower,
                mode="lines",
                line={"width": 0},
                fill="tonexty",
                fillcolor=_hex_to_rgba(color, 0.15),
                name=f"{name} ({symbol})",
                showlegend=True,
            )
        )
    else:
        # Each key is a separate line
        dash_styles = ["dash", "dot", "dashdot", "longdash", "longdashdot"]
        for j, (key, values) in enumerate(result.items()):
            fig.add_trace(
                go.Scatter(
                    x=x,
                    y=values,
                    mode="lines",
                    name=f"{name} {key} ({symbol})" if len(keys) > 1 else f"{name} ({symbol})",
                    line={"color": color, "width": 1, "dash": dash_styles[j % len(dash_styles)]},
                )
            )


def plot_price(
    df: pd.DataFrame,
    *,
    price_col: str = "adj_close",
    indicators: list[dict] | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = None,
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a price line chart.

    Parameters
    ----------
    df : pd.DataFrame
        Input data containing a `dt` datetime column, a `symbol`
        column, and at least the column named by `price_col`.

    price_col : str, default="adj_close"
        Column name to plot on the y-axis. Must be one of `open`, `high`,
        `low`, `close`, or `adj_close`.

    indicators : list[dict] or None, default=None
        Indicator overlays. Each dict must have keys ``name`` (str) and
        ``fn`` (callable accepting a DataFrame and returning a dict of
        arrays). Built-in indicators use the Rust pyclass ``.compute``
        method; custom ones wrap user code.

    title : str, dict or None, default=None
        Title for the plot.

    legend : str, dict or None, default="upper left"
        Legend for the plot.

    figsize : tuple[int, int] or None, default=None
        Figure size in pixels as ``(width, height)``.

    filename : str, Path or None, default=None
        Save the plot to this path.

    display : bool or None, default=True
        Whether to render the plot. If None, return the figure.

    Returns
    -------
    go.Figure or None
        The Plotly figure object. Only returned if ``display=None``.

    """
    fig = go.Figure()

    symbols = sorted(df["symbol"].unique())

    for idx, symbol in enumerate(symbols):
        subset = df[df["symbol"] == symbol].sort_values("dt")
        color = PALETTE[idx % len(PALETTE)]

        fig.add_trace(
            go.Scatter(
                x=subset["dt"],
                y=subset[price_col],
                mode="lines",
                name=symbol,
                line={"color": color, "width": 1.5},
            )
        )

        # Overlay indicators for this symbol
        if indicators:
            for ind in indicators:
                try:
                    result = ind["fn"](subset)
                    _add_indicator_traces(fig, result, ind["name"], color, symbol, subset["dt"])
                except Exception:  # noqa: BLE001
                    continue

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel=PRICE_COLUMNS[price_col],
        figsize=figsize,
        filename=filename,
        display=display,
    )
