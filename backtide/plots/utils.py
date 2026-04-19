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
TITLE_FONTSIZE: int = 20
LABEL_FONTSIZE: int = 18
TICK_FONTSIZE: int = 14

# Backtide default color palette (blue-centric)
PALETTE: list[str] = [
    "#1565C0",  # Blue 800
    "#42A5F5",  # Blue 400
    "#0D47A1",  # Blue 900
    "#90CAF9",  # Blue 200
    "#1E88E5",  # Blue 600
    "#64B5F6",  # Blue 300
    "#0277BD",  # Light Blue 800
    "#4FC3F7",  # Light Blue 300
    "#01579B",  # Light Blue 900
    "#81D4FA",  # Light Blue 200
]


def _plot(
    fig: go.Figure,
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper right",
    xlabel: str | None = None,
    ylabel: str | None = None,
    xlim: list[Any] | tuple[Any, Any] | None = None,
    ylim: list[Any] | tuple[Any, Any] | None = None,
    figsize: tuple[int, int] | None = None,
    template: str = "plotly_dark",
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Apply consistent layout to a Plotly figure and optionally display/save it.

    This helper centralizes all styling decisions so that every plot in the
    library looks the same without duplicating layout code.

    Parameters
    ----------
    fig : go.Figure
        The Plotly figure to style.

    title : str, dict or None, default=None
        Title for the plot.

        - If None, no title is shown.
        - If str, text for the title.
        - If dict, custom title configuration forwarded to
          `fig.update_layout(title=...)`.

    legend : str, dict or None, default="upper right"
        Legend for the plot.

        - If None: no legend is shown.
        - If str: named position (e.g., `"upper right"`, `"lower left"`).
        - If dict: legend configuration forwarded to
          `fig.update_layout(legend=...)`.

    xlabel : str or None, default=None
        Label for the x-axis.

    ylabel : str or None, default=None
        Label for the y-axis.

    xlim : tuple or None, default=None
        Limits for the x-axis as `(min, max)`.

    ylim : tuple or None, default=None
        Limits for the y-axis as `(min, max)`.

    figsize : tuple[int, int] or None, default=None
        Figure size in pixels as `(width, height)`. If None, defaults to
        `(900, 600)`.

    template : str, default="plotly_dark"
        Plotly template name for figure styling.

    filename : str, Path or None, default=None
        Save the plot to this path. The file type is inferred from the suffix
        (`.html`, `.png`, `.pdf`...). If the path has no suffix, the plot is
        saved as `.html`. If None, the plot isn't saved.

    display : bool or None, default=True
        Whether to render the plot.

        - True: show the plot.
        - False: do not show or return.
        - None: return the figure without showing.

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

    _position_map: dict[str, dict[str, Any]] = {
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
        "groupclick": "toggleitem",
        "font_size": LABEL_FONTSIZE,
        "bgcolor": "rgba(0, 0, 0, 0)",
    }

    if isinstance(legend, str):
        legend_cfg = default_legend | _position_map.get(legend, {})
    elif isinstance(legend, dict):
        legend_cfg = default_legend | legend
    else:
        legend_cfg = None

    title_space = TITLE_FONTSIZE if (title_cfg and title_cfg.get("text")) else 10

    layout: dict[str, Any] = {
        "template": template,
        "width": width,
        "height": height,
        "showlegend": legend is not None,
        "hoverlabel": {"font_size": LABEL_FONTSIZE},
        "font_size": TICK_FONTSIZE,
        "margin": {"l": 50, "b": 50, "r": 0, "t": 25 + title_space, "pad": 0},
        "xaxis_tickformat": _moment_to_strftime(get_config().display.date_format),
    }

    if title_cfg:
        layout["title"] = title_cfg
    if legend_cfg:
        layout["legend"] = legend_cfg
    if xlabel:
        layout["xaxis_title"] = {"text": xlabel, "font_size": LABEL_FONTSIZE}
    if ylabel:
        layout["yaxis_title"] = {"text": ylabel, "font_size": LABEL_FONTSIZE}
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
