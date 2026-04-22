"""Backtide.

Author: Mavs
Description: Module containing the volume bar chart function for data analysis.

"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pandas as pd
import plotly.graph_objects as go

from backtide.config import get_config
from backtide.plots.utils import _plot

cfg = get_config()


def plot_volume(
    data: pd.DataFrame,
    *,
    title: str | dict[str, Any] | None = None,
    legend: str | dict[str, Any] | None = "upper left",
    figsize: tuple[int, int] | None = (900, 600),
    filename: str | Path | None = None,
    display: bool | None = True,
) -> go.Figure | None:
    """Create a volume bar chart.

    Displays trading volume over time for one or more symbols. Each symbol
    is rendered as a separate bar trace with its own color.

    Parameters
    ----------
    data : pd.DataFrame
        Input data containing columns `symbol`, `volume` and `dt` with the
        datetime.

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
    - backtide.plots:plot_price

    Examples
    --------
    ```pycon
    import pandas as pd

    from backtide.storage import query_bars
    from backtide.plots import plot_volume

    df = query_bars(["AAPL", "MSFT"], "1d")
    df["dt"] = pd.to_datetime(df["open_ts"], unit="s", utc=True)

    plot_volume(df)
    ```

    """
    fig = go.Figure()

    for idx, symbol in enumerate(data["symbol"].unique()):
        subset = data[data["symbol"] == symbol].sort_values("dt")
        color = cfg.plots.palette[idx % len(cfg.plots.palette)]

        fig.add_trace(
            go.Bar(
                x=subset["dt"],
                y=subset["volume"],
                name=symbol,
                marker_color=color,
                opacity=0.7,
            )
        )

    fig.update_layout(barmode="group")

    return _plot(
        fig,
        title=title,
        legend=legend,
        xlabel="Date",
        ylabel="Volume",
        figsize=figsize,
        filename=filename,
        display=display,
    )

