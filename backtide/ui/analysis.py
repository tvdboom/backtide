"""Backtide.

Author: Mavs
Description: Data analysis page for exploring stored market data.

"""

from zoneinfo import ZoneInfo

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.utils.constants import MAX_INSTRUMENT_SELECTION
from backtide.core.storage import query_bars, query_instruments
from backtide.plots.candlestick import plot_candlestick
from backtide.ui.utils import (
    _get_timezone,
    _to_upper_values,
    _persist,
    _default,
    _to_pandas,
)


# ─────────────────────────────────────────────────────────────────────────────
# Utility functions
# ─────────────────────────────────────────────────────────────────────────────


@st.cache_data(show_spinner="Loading bars from database...", max_entries=10)
def _load_bars(symbols: list[str], interval: str, provider: str | None = None) -> pd.DataFrame:
    """Fetch bars for the selected symbols and interval."""
    return _to_pandas(query_bars(symbol=symbols, interval=interval, provider=provider))


def _ts_to_datetime(series: pd.Series, tz: ZoneInfo) -> pd.Series:
    """Convert a Unix-timestamp column to timezone-aware datetimes."""
    return pd.to_datetime(series, unit="s", utc=True).dt.tz_convert(tz)


# ─────────────────────────────────────────────────────────────────────────────
# Analysis interface
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
tz = _get_timezone(cfg.display.timezone)

st.set_page_config(page_title="Backtide - Analysis")

st.title("Analysis", text_alignment="center")

st.divider()

# Load all instruments from the database
all_i = {x.symbol: x for x in query_instruments()}

if len(all_i) == 0:
    st.info(
        "The database is empty. Head over to the **Download** page to fetch some market data.",
        icon=":material/info:",
    )
    st.stop()

# Filter instruments to only configured providers
all_i = {
    k: v for k, v in all_i.items() if v.provider == cfg.data.providers[v.instrument_type]
}

if len(all_i) == 0:
    st.info(
        "There is no data in the database for the currently configured providers.",
        icon=":material/info:",
    )
    st.stop()

# If there are symbols selected that are not in storage, remove them from selection
if default := _default("symbols"):
    default = [s for s in default if s in all_i]

symbols = st.multiselect(
    label="Symbols",
    key=(key := "symbols"),
    options=all_i,
    default=default,
    format_func=lambda s: f"{s} - {all_i[s].name}" if all_i[s].instrument_type.is_equity else s,
    placeholder="Select one or more symbols...",
    max_selections=MAX_INSTRUMENT_SELECTION,
    on_change=lambda: (_to_upper_values("symbols"), _persist("symbols")),
    help="Select the symbols to analyze. Only symbols available in the database are shown.",
)

if not symbols:
    st.warning("Select at least one symbol to begin the analysis.", icon=":material/warning:")
else:
    # Intervals available for all selected symbols (intersection)
    available_intervals = Interval.variants()

    if default := _default("interval"):
        default = [s for s in default if s in available_intervals]

    interval = st.pills(
        label="Interval",
        key=(key := "interval"),
        options=available_intervals or Interval.variants(),
        default=default,
        selection_mode="single",
        on_change=lambda k=key: _persist(k),
        disabled=len(available_intervals) == 0,
        help=(
            "Select the interval to analyze. Only intervals available for all "
            "selected symbols can be selected."
        ),
    )

    if not available_intervals:
        st.warning(
            "No common intervals available for the selected symbols.",
            icon=":material/warning:",
        )

    bars = _load_bars([all_i[s] for s in symbols], interval)

    if bars.empty:
        st.info("No bars found for the selected symbols and interval.", icon=":material/info:")

    # Add datetime column for convenience
    bars["datetime"] = _ts_to_datetime(bars["open_ts"], tz)


# ─────────────────────────────────────────────────────────────────────────────
# Tabs
# ─────────────────────────────────────────────────────────────────────────────

st.divider()

tab1, tab2 = st.tabs(
    [
        ":material/candlestick_chart: Candlestick",
        ":material/analytics: Distribution",
    ],
    key=(key := "plot_tabs"),
    default=_default(key),
    on_change=lambda k=key: _persist(k),
)


# ─────────────────────────────────────────────────────────────────────────────
# 1. Candlestick chart
# ─────────────────────────────────────────────────────────────────────────────

with tab1:
    st.caption("OHLC candlestick chart showing price action over time.")

    for symbol in symbols:
        symbol_df = bars[bars["symbol"] == symbol].sort_values("datetime")

        fig_candle = plot_candlestick(
            symbol_df,
            title=f"{symbol} ({interval})",
            ylabel="Price",
            showlegend=cs_showlegend,
            rangeslider=cs_rangeslider,
            template=cs_template,
        )

        st.plotly_chart(fig_candle, use_container_width=True)


# ─────────────────────────────────────────────────────────────────────────────
# 2. Price distribution boxplot
# ─────────────────────────────────────────────────────────────────────────────

with tab2:
    st.subheader("Price Distribution")

    st.caption(
        "Compare the distribution of prices across selected symbols. "
        "The boxplot shows median, quartiles, and outliers."
    )
