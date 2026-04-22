"""Backtide.

Author: Mavs
Description: Data analysis page for exploring stored market data.

"""

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import Interval
from backtide.core.storage import query_bars, query_instruments
from backtide.indicators.utils import _load_stored_indicators
from backtide.plots.candlestick import plot_candlestick
from backtide.plots.correlation import plot_correlation
from backtide.plots.drawdown import plot_drawdown
from backtide.plots.price import PRICE_COLUMNS, plot_price
from backtide.plots.returns import plot_returns
from backtide.plots.volume import plot_volume
from backtide.ui.utils import (
    _default,
    _get_timezone,
    _persist,
    _to_upper_values,
)
from backtide.utils.constants import MAX_INSTRUMENT_SELECTION
from backtide.utils.utils import _to_pandas, _ts_to_datetime

# ─────────────────────────────────────────────────────────────────────────────
# Utility functions
# ─────────────────────────────────────────────────────────────────────────────


@st.cache_data(show_spinner="Loading bars from database...", max_entries=10)
def _load_bars(symbols: list[str], interval: str) -> pd.DataFrame:
    """Fetch bars for the selected symbols and interval."""
    return _to_pandas(query_bars(symbol=symbols, interval=interval))


# ─────────────────────────────────────────────────────────────────────────────
# Analysis interface
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
tz = _get_timezone(cfg.display.timezone)

st.set_page_config(page_title="Backtide - Analysis")

st.subheader("Analysis", text_alignment="center")
st.write("")

# Load all instruments from the database
all_i = {x.symbol: x for x in query_instruments()}

if len(all_i) == 0:
    st.info(
        "The database is empty. Head over to the **Download** page to fetch some market data.",
        icon=":material/info:",
    )
    st.stop()

# Filter instruments to only configured providers
all_i = {k: v for k, v in all_i.items() if v.provider == cfg.data.providers[v.instrument_type]}

if len(all_i) == 0:
    st.warning(
        "There is no data in the database for the currently configured providers.",
        icon=":material/warning:",
    )
    st.stop()

# If there are symbols selected that are not in storage, remove them from selection
if default := _default("symbols"):
    default = [s for s in default if s in all_i]

symbols = st.multiselect(  # ty: ignore[no-matching-overload]
    label="Symbols",
    key=(key := "symbols"),
    options=all_i,
    default=default,
    format_func=lambda s: f"{s} - {all_i[s].name}" if all_i[s].instrument_type.is_equity else s,
    placeholder="Select one or more symbols...",
    max_selections=MAX_INSTRUMENT_SELECTION,
    on_change=lambda k=key: (_to_upper_values("symbols"), _persist(k)),
    help="Select the symbols to analyze. Only symbols available in the database are shown.",
)

interval = st.pills(
    label="Interval",
    key=(key := "interval"),
    required=True,
    options=Interval.variants(),
    selection_mode="single",
    default=_default(key, Interval.get_default()),
    on_change=lambda k=key: _persist(k),
    help="Select an interval to analyze.",
)

if not symbols:
    st.warning("Select at least one symbol to begin the analysis.", icon=":material/warning:")
    st.stop()

bars = _load_bars(symbols, interval)

if bars.empty:
    st.warning(
        "No bars found for the selected symbols and interval.",
        icon=":material/warning:",
    )
    st.stop()

# Check if any of the selected symbols have no bars for the selected interval
if missing := set(symbols) - set(bars["symbol"].unique()):
    symbols = [s for s in symbols if s not in missing]
    st.warning(
        f"No bars found for the following symbols at the **{interval}** interval: "
        f"{', '.join([f'**{m}**' for m in missing])}. They will be excluded from "
        "the analysis.",
        icon=":material/warning:",
    )

# Select only data from configured providers per symbol
bars = bars[
    bars.apply(
        lambda r: r["provider"] == str(cfg.data.providers[all_i[r["symbol"]].instrument_type]),
        axis=1,
    )
]

# ─────────────────────────────────────────────────────────────────────────────
# Tabs
# ─────────────────────────────────────────────────────────────────────────────

tab1, tab2, tab3, tab4, tab5, tab6 = st.tabs(
    [
        ":material/show_chart: Price",
        ":material/candlestick_chart: Candlestick",
        ":material/bar_chart: Volume",
        ":material/stacked_line_chart: Returns",
        ":material/trending_down: Drawdown",
        ":material/grid_on: Correlation",
    ],
    key=(key := "plot_tabs"),
    default=_default(key),
    on_change=lambda k=key: _persist(k),
)

# Add datetime column for plotting
bars["dt"] = _ts_to_datetime(bars["open_ts"], tz)

price_col_radio = lambda key: st.radio(
    label="Price",
    key=key,
    options=PRICE_COLUMNS,
    index=list(PRICE_COLUMNS).index("adj_close" if (x := _default("price_col")) is None else x),
    format_func=lambda c: PRICE_COLUMNS[c],
    horizontal=False,
    on_change=lambda k=key: st.session_state.update(_price_col=st.session_state[k]),
)

with tab1:
    col1, col2 = st.columns([10, 1])
    col1.caption("Price over time for selected symbols.")

    stored_ind = _load_stored_indicators(cfg)

    with col2.popover(":material/tune:"):
        price_col = price_col_radio("price_col_price")

        if stored_ind:
            selected_ind = st.multiselect(
                label="Indicators",
                key=(key := "price_indicators"),
                options=stored_ind,
                default=_default(key, []),
                placeholder="Select indicators...",
                on_change=lambda k=key: _persist(k),
                help="Overlay indicators on the price chart.",
            )
        else:
            selected_ind = []

    st.plotly_chart(
        plot_price(
            data=bars,
            price_col=price_col,
            indicators={n: stored_ind[n] for n in selected_ind},
            display=None,
        ),
        width="stretch",
    )

with tab2:
    col1, col2 = st.columns([10, 1])
    col1.caption("OHLC candlestick chart showing price action over time.")

    # Compute available date range from bars
    dates = bars["dt"].sort_values()
    min_date = dates.iloc[0].date()
    max_date = dates.iloc[-1].date()
    default_start = dates.iloc[-min(90, len(dates.unique()))].date()

    with col2.popover(":material/tune:"):
        cs_date_range = st.date_input(
            label="Date range",
            key=(key := "cs_date_range"),
            value=(_default(key, (default_start, max_date))),
            min_value=min_date,
            max_value=max_date,
            format=cfg.display.date_format,
            on_change=lambda k=key: _persist(k),
            help="Select the visible date range for the candlestick chart.",
        )

        cs_rangeslider = st.toggle(
            label="Range slider",
            key=(key := "cs_rangeslider"),
            value=_default(key, fallback=True),
            on_change=lambda k=key: _persist(k),
            help="Hide/show the range slider below the chart.",
        )

    # Only plot when both start and end are selected
    if isinstance(cs_date_range, tuple) and len(cs_date_range) == 2:
        _cs_start, _cs_end = cs_date_range
    else:
        _cs_start, _cs_end = default_start, max_date

    _cs_bars = bars[(bars["dt"].dt.date >= _cs_start) & (bars["dt"].dt.date <= _cs_end)]

    st.plotly_chart(
        plot_candlestick(
            _cs_bars,
            rangeslider=cs_rangeslider,
            display=None,
        ),
        width="stretch",
    )

with tab3:
    st.caption("Trading volume over time for selected symbols.")

    st.plotly_chart(
        plot_volume(data=bars, display=None),
        width="stretch",
    )

with tab4:
    col1, col2 = st.columns([10, 1])
    col1.caption("Distribution of period-over-period percentage returns.")

    with col2.popover(":material/tune:"):
        price_col = price_col_radio("price_col_dist")

    st.plotly_chart(
        plot_returns(data=bars, price_col=price_col, display=None),
        width="stretch",
    )

with tab5:
    col1, col2 = st.columns([10, 1])
    col1.caption("Percentage drawdown from the running peak over time.")

    with col2.popover(":material/tune:"):
        price_col = price_col_radio("price_col_drawdown")

    st.plotly_chart(
        plot_drawdown(data=bars, price_col=price_col, display=None),
        width="stretch",
    )

with tab6:
    col1, col2 = st.columns([10, 1])
    col1.caption(
        "Pairwise correlation of returns across selected symbols. Select at least two symbols."
    )

    with col2.popover(":material/tune:"):
        price_col = price_col_radio("price_col_corr")

    if len(symbols) < 2:
        st.info(
            "Select at least two symbols to compute correlation.",
            icon=":material/info:",
        )
    else:
        st.plotly_chart(
            plot_correlation(data=bars, price_col=price_col, display=None),
            width="stretch",
        )
