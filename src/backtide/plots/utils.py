"""Backtide.

Author: Mavs
Description: Shared plotting utilities for consistent figure styling.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import plotly.graph_objects as go

from backtide.core.config import get_config
from backtide.ui.utils import _moment_to_strftime

# Default font sizes
TITLE_FONTSIZE: int = 22
LABEL_FONTSIZE: int = 20
TICK_FONTSIZE: int = 14

# Backtide default color palette (blue → teal gradient)
PALETTE: list[str] = [
    "rgb(13, 71, 161)",  # Blue 900
    "rgb(2, 136, 209)",  # Light Blue 600
    "rgb(0, 172, 193)",  # Cyan 600
    "rgb(0, 137, 123)",  # Teal 600
    "rgb(56, 142, 60)",  # Green 700
    "rgb(129, 199, 132)",  # Green 300
]


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
    template: str = "plotly_dark",
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

    xlabel : str or None, default=None
        Label for the x-axis.

    ylabel : str or None, default=None
        Label for the y-axis.

    xlim : tuple[int, int] or None, default=None
        Limits for the x-axis as `(min, max)`.

    ylim : tuple[int, int] or None, default=None
        Limits for the y-axis as `(min, max)`.

    title: str, dict or None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, [title configuration][parameters].

    legend: str, dict or None, default="upper left"
        Legend for the plot. See the [user guide][parameters] for an extended
        description of the choices.

        * If None: No legend is shown.
        * If str: Position to display the legend.
        * If dict: Legend configuration.

    figsize: tuple, default=(900, 600)
        Figure's size in pixels, format as (x, y).

    filename: str, Path or None, default=None
        Save the plot using this name. The type of the file depends on the
        provided name (`.html`, `.png`, `.pdf`, etc...). If `filename` has no
        file type, the plot is saved as `.html`. If `None`, the plot isn't saved.

    display: bool or None, default=True
        Whether to render the plot. If `None`, it returns the figure.

    **kwargs
        Additional keyword arguments for plotly's layout.

    Returns
    -------
    go.Figure or None
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
        "font_size": TITLE_FONTSIZE,
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
        "font_size": LABEL_FONTSIZE,
        "bgcolor": "rgba(255, 255, 255, 0.5)",
    }

    if isinstance(legend, str):
        legend_cfg = default_legend | position_map.get(legend, {})
    elif isinstance(legend, dict):
        legend_cfg = default_legend | legend
    else:
        legend_cfg = None

    title_space = TITLE_FONTSIZE if (title_cfg and title_cfg.get("text")) else 10

    layout: dict[str, Any] = {
        "template": kwargs.get("template"),
        "width": width,
        "height": height,
        "showlegend": legend is not None,
        "hoverlabel": {"font_size": LABEL_FONTSIZE},
        "font_size": TICK_FONTSIZE,
        "margin": {"l": 50, "b": 50, "r": 0, "t": 25 + title_space, "pad": 0},
        "xaxis_tickformat": _moment_to_strftime(get_config().display.date_format),
        "yaxis_tickformat": "f",
    }

    if title_cfg:
        layout["title"] = title_cfg
    if legend_cfg:
        layout["legend"] = legend_cfg
    if xlabel:
        layout["xaxis_title"] = {"text": xlabel, "font_size": LABEL_FONTSIZE}
    if ylabel:
        layout["yaxis_title"] = {"text": ylabel, "font_size": LABEL_FONTSIZE, "standoff": 20}

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
