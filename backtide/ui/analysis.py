"""Backtide.

Author: Mavs
Description: Data analysis page for exploring stored market data.

"""

from functools import reduce
from zoneinfo import ZoneInfo

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.utils.constants import MAX_INSTRUMENT_SELECTION
from backtide.core.storage import query_bars, query_instruments
from backtide.plots.candlestick import plot_candlestick
from backtide.ui.utils import (
    _get_timezone,
    _query_bars_summary,
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

# Collect all configured providers
configured_providers = list({str(p) for p in cfg.data.providers.values()})

st.set_page_config(page_title="Backtide - Analysis")

st.title("Analysis", text_alignment="center")

st.divider()

# Load the storage summary to know what data is available
summary_df = _to_pandas(_query_bars_summary())

if summary_df.empty:
    st.info(
        "The database is empty. Head over to the **Download** page to fetch some market data.",
        icon=":material/info:",
    )
    st.stop()

# Filter summary to only configured providers
if "provider" in summary_df.columns:
    summary_df = summary_df[summary_df["provider"].isin(configured_providers)]

if summary_df.empty:
    st.info(
        "No data found for the currently configured providers.",
        icon=":material/info:",
    )
    st.stop()

available_symbols = sorted(summary_df["symbol"].unique().tolist())

# If there are symbols selected that are not in storage, remove them from selection
if "symbols" in st.session_state:
    default = [s for s in st.session_state.symbols if s in available_symbols]
else:
    default = None

ai = {x.symbol: x for x in query_instruments()}

symbols = st.multiselect(
    label="Symbols",
    key=(key := "symbols"),
    options=sorted(ai.keys()),
    default=_default(key, []),
    format_func=lambda s: f"{s} - {ai[s].name}" if ai[s].instrument_type.is_equity else s,
    placeholder="Select one or more symbols...",
    max_selections=MAX_INSTRUMENT_SELECTION,
    on_change=lambda: (_to_upper_values("symbols"), _persist("symbols")),
    help="Select the symbols to analyze. Only symbols available in the database are shown.",
)

if not symbols:
    st.warning("Select at least one symbol to begin the analysis.", icon=":material/warning:")
else:
    # Intervals available for ALL selected symbols (intersection)
    interval_sets = [
        set(summary_df[summary_df["symbol"] == s]["interval"].unique().tolist())
        for s in symbols
    ]
    available_intervals = sorted(reduce(lambda a, b: a & b, interval_sets)) if interval_sets else []

    interval = st.pills(
        label="Interval",
        options=available_intervals,
        selection_mode="single",
        default=available_intervals[0] if available_intervals else None,
        help="Select the bar interval to analyze. Only intervals available for all selected symbols are shown.",
    )

    if not interval:
        st.warning("No common intervals available for the selected symbols.", icon=":material/warning:")

    # Determine provider from config (use first configured provider that has data)
    provider = configured_providers[0] if configured_providers else None

    bars_df = _load_bars(symbols, interval, provider)

    if bars_df.empty:
        st.info("No bars found for the selected symbols and interval.", icon=":material/info:")
        st.stop()

    # Add datetime column for convenience
    bars_df["datetime"] = _ts_to_datetime(bars_df["open_ts"], tz)


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
        symbol_df = bars_df[bars_df["symbol"] == symbol].sort_values("datetime")

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
