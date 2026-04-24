"""Backtide.

Author: Mavs
Description: Shared plotting utilities for consistent figure styling.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pandas as pd
import plotly.graph_objects as go
from backtide.core.data import Currency
from backtide.core.config import get_config
from backtide.ui.utils import _moment_to_strftime

cfg = get_config()


def _check_columns(data: pd.DataFrame, columns: list[str], caller: str) -> None:
    """Verify that required columns exist in the DataFrame.

    Parameters
    ----------
    data : pd.DataFrame
        The input DataFrame to check.

    columns : list[str]
        Column names that must be present.

    caller : str
        Name of the calling function, used in the error message.

    Raises
    ------
    ValueError
        If any of the required columns are missing.

    """
    if missing := [c for c in columns if c not in data.columns]:
        raise ValueError(
            f"Function {caller} requires column(s) {missing} but the provided data "
            f"only has: {list(data.columns)}"
        )


def _get_currency_symbol(data: pd.DataFrame) -> Currency:
    """Extract a single currency from the `currency` column.

    Returns the currency when every row shares the same currency code,
    otherwise `None`.

    """
    if "currency" not in data.columns:
        return None

    if len(codes := data["currency"].dropna().unique()) == 1:
        try:
            return Currency(codes[0])
        except (ValueError, KeyError):
            return None

    return None


def _plot(
    fig: go.Figure,
    *,
    xlabel: str | None = None,
    ylabel: str | None = None,
    xlim: tuple[int, int] | None = None,
    ylim: tuple[int, int] | None = None,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper right",
    figsize: tuple[int, int] | None = None,
    filename: str | Path | None = None,
    display: bool | None = True,
    **kwargs,
) -> go.Figure | None:
    """Apply consistent layout to a Plotly figure and optionally display/save it.

    This helper centralizes all styling decisions so that every plot in the
    library looks the same without duplicating layout code.

    Parameters
    ----------
    fig : go.Figure
        The Plotly figure to style.

    xlabel : str | None, default=None
        Label for the x-axis.

    ylabel : str | None, default=None
        Label for the y-axis.

    xlim : tuple[int, int] | None, default=None
        Limits for the x-axis as `(min, max)`.

    ylim : tuple[int, int] | None, default=None
        Limits for the y-axis as `(min, max)`.

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

    **kwargs
        Additional keyword arguments for plotly's layout.

    Returns
    -------
    go.Figure | None
        The figure object. Only returned when `display=None`.

    """
    width, height = figsize or (900, 600)

    default_title = {
        "x": 0.5,
        "y": 1,
        "pad": {"t": 15, "b": 15},
        "xanchor": "center",
        "yanchor": "top",
        "xref": "paper",
        "font_size": cfg.plots.title_fontsize,
    }

    if isinstance(title, dict):
        title_cfg = default_title | title
    elif isinstance(title, str):
        title_cfg = {"text": title, **default_title}
    else:
        title_cfg = None

    position_map: dict[str, dict[str, Any]] = {
        "upper left": {"x": 0.01, "y": 0.99, "xanchor": "left", "yanchor": "top"},
        "lower left": {"x": 0.01, "y": 0.01, "xanchor": "left", "yanchor": "bottom"},
        "upper right": {"x": 0.99, "y": 0.99, "xanchor": "right", "yanchor": "top"},
        "lower right": {"x": 0.99, "y": 0.01, "xanchor": "right", "yanchor": "bottom"},
        "upper center": {"x": 0.5, "y": 0.99, "xanchor": "center", "yanchor": "top"},
        "lower center": {"x": 0.5, "y": 0.01, "xanchor": "center", "yanchor": "bottom"},
        "center left": {"x": 0.01, "y": 0.5, "xanchor": "left", "yanchor": "middle"},
        "center right": {"x": 0.99, "y": 0.5, "xanchor": "right", "yanchor": "middle"},
        "center": {"x": 0.5, "y": 0.5, "xanchor": "center", "yanchor": "middle"},
    }

    default_legend = {
        "traceorder": "grouped",
        "groupclick": kwargs.get("groupclick", "toggleitem"),
        "font_size": cfg.plots.label_fontsize,
        "grouptitlefont_size": cfg.plots.label_fontsize,
        "grouptitlefont_color": "rgb(0, 0, 0)",
        "bgcolor": "rgba(255, 255, 255, 0.2)",
    }

    if isinstance(legend, str):
        legend_cfg = default_legend | position_map.get(legend, {})
    elif isinstance(legend, dict):
        legend_cfg = default_legend | legend
    else:
        legend_cfg = None

    title_space = cfg.plots.title_fontsize if (title_cfg and title_cfg.get("text")) else 10

    layout = {
        "template": kwargs.get("template", cfg.plots.template),
        "width": width,
        "height": height,
        "showlegend": legend is not None,
        "hoverlabel": {"font_size": cfg.plots.label_fontsize},
        "margin": {"l": 50, "b": 50, "r": 0, "t": 25 + title_space, "pad": 0},
        "xaxis_tickformat": _moment_to_strftime(get_config().display.date_format),
        "xaxis_tickfont_size": cfg.plots.tick_fontsize,
        "yaxis_tickfont_size": cfg.plots.tick_fontsize,
    }

    if title_cfg:
        layout["title"] = title_cfg
    if legend_cfg:
        layout["legend"] = legend_cfg
    if xlabel:
        layout["xaxis_title"] = {"text": xlabel, "font_size": cfg.plots.label_fontsize}
    if ylabel:
        layout["yaxis_title"] = {
            "text": ylabel,
            "font_size": cfg.plots.label_fontsize,
            "standoff": 20,
        }

    if xlim is not None:
        layout["xaxis_range"] = xlim
    if ylim is not None:
        layout["yaxis_range"] = ylim

    fig.update_layout(**layout)

    if filename:
        path = Path(filename)
        if path.suffix in ("", ".html"):
            fig.write_html(path.with_suffix(".html"))
        else:
            fig.write_image(path)

    if display:
        fig.show()
    elif display is None:
        return fig

    return None
