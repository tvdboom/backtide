"""Backtide.

Author: Mavs
Description: Data analysis page for exploring stored market data.

"""

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import Interval
from backtide.core.storage import query_bars, query_dividends, query_instruments
from backtide.indicators.utils import _load_stored_indicators
from backtide.plots.candlestick import plot_candlestick
from backtide.plots.correlation import plot_correlation
from backtide.plots.dividends import plot_dividends
from backtide.plots.drawdown import plot_drawdown
from backtide.plots.price import PRICE_COLUMNS, plot_price
from backtide.plots.returns import plot_returns
from backtide.plots.seasonality import plot_seasonality
from backtide.plots.stats import compute_summary_stats
from backtide.plots.volume import plot_volume
from backtide.plots.vwap import plot_vwap
from backtide.ui.utils import (
    _default,
    _get_logokit_url,
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

TAB_LABELS = [
    ":material/analytics: Summary",
    ":material/show_chart: Price",
    ":material/candlestick_chart: Candlestick",
    ":material/bar_chart: Volume",
    ":material/waterfall_chart: VWAP",
    ":material/stacked_line_chart: Returns",
    ":material/calendar_month: Seasonality",
    ":material/trending_down: Drawdown",
    ":material/grid_on: Correlation",
    ":material/payments: Dividends",
]

tabs = st.tabs(
    TAB_LABELS,
    key=(key := "plot_tabs"),
    default=_default(key),
    on_change=lambda k=key: _persist(k),
)

# Determine active tab index for lazy rendering
active_tab = st.session_state.get("plot_tabs", TAB_LABELS[0])

# Add datetime column for plotting
bars["dt"] = _ts_to_datetime(bars["open_ts"], tz)

# Add currency column from instruments
bars["currency"] = bars["symbol"].map(lambda s: str(all_i[s].quote) if s in all_i else None)

# Warn if symbols are denominated in multiple currencies for relevant tabs
currencies = bars["currency"].dropna().unique()
non_currency_tabs = (TAB_LABELS[0], TAB_LABELS[5], TAB_LABELS[6], TAB_LABELS[7], TAB_LABELS[8])
if len(currencies) > 1 and active_tab not in non_currency_tabs:
    st.warning(
        "The selected symbols are denominated in multiple currencies "
        f"({', '.join(f'**{c}**' for c in sorted(currencies))}). "
        "Currency labels are hidden from plot axes.",
        icon=":material/warning:",
    )

price_col_radio = lambda key: st.radio(
    label="Price",
    key=key,
    options=PRICE_COLUMNS,
    index=list(PRICE_COLUMNS).index("adj_close" if (x := _default("price_col")) is None else x),
    format_func=lambda c: PRICE_COLUMNS[c],
    horizontal=False,
    on_change=lambda k=key: st.session_state.update(_price_col=st.session_state[k]),
)

# ── Tab 0: Summary ───────────────────────────────────────────────────────────

with tabs[0]:
    col1, col2 = st.columns([10, 1])
    col1.caption("Key performance and risk metrics for each selected symbol.")

    with col2.popover(":material/tune:"):
        price_col = price_col_radio("price_col_summary")

    if active_tab == TAB_LABELS[0]:
        stats_df = compute_summary_stats(data=bars, price_col=price_col)

        logokit_key = cfg.display.logokit_api_key
        if logokit_key:
            stats_df.index = pd.Index(
                data=stats_df.apply(
                    lambda row: _get_logokit_url(
                        row["Symbol"],
                        all_i[row["Symbol"]].instrument_type,
                        logokit_key,
                    )
                    if row["Symbol"] in all_i
                    else "",
                    axis=1,
                ),
                name="Logo",
            )

        summary_column_config = {
            "Symbol": st.column_config.TextColumn(
                pinned=True,
                help="Ticker symbol of the instrument.",
            ),
            "Ann. Return": st.column_config.NumberColumn(
                format="%+.2f%%",
                help="Compound annual growth rate (CAGR). Measures the geometric average yearly return over the full period.",
            ),
            "Ann. Volatility": st.column_config.NumberColumn(
                format="%.2f%%",
                help="Annualized standard deviation of returns. Higher values indicate greater price variability and risk.",
            ),
            "Sharpe Ratio": st.column_config.NumberColumn(
                format="%.2f",
                help="Risk-adjusted return: excess return per unit of total volatility. Higher is better; above 1.0 is generally considered good.",
            ),
            "Sortino Ratio": st.column_config.NumberColumn(
                format="%.2f",
                help="Like the Sharpe ratio but only penalizes downside volatility. Better suited for assets with asymmetric return distributions.",
            ),
            "Max Drawdown": st.column_config.NumberColumn(
                format="%.2f%%",
                help="Largest peak-to-trough decline in cumulative returns. Represents the worst-case loss an investor would have experienced.",
            ),
            "Win Rate": st.column_config.NumberColumn(
                format="%.1f%%",
                help="Percentage of periods with a positive return. A value above 50% means the price went up more often than it went down.",
            ),
            "Total Bars": st.column_config.NumberColumn(
                format="%d",
                help="Total number of data points (bars) available for this symbol at the selected interval.",
            ),
        }

        if logokit_key:
            summary_column_config["Logo"] = st.column_config.ImageColumn(
                label="", width="small"
            )

        st.dataframe(
            stats_df,
            column_config=summary_column_config,
            hide_index=stats_df.index.name is None,
            width="stretch",
        )

# ── Tab 1: Price ─────────────────────────────────────────────────────────────

with tabs[1]:
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

    if active_tab == TAB_LABELS[1]:
        st.plotly_chart(
            plot_price(
                data=bars,
                price_col=price_col,
                indicators={n: stored_ind[n] for n in selected_ind},
                display=None,
            ),
            width="stretch",
        )

# ── Tab 2: Candlestick ───────────────────────────────────────────────────────

with tabs[2]:
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

    if active_tab == TAB_LABELS[2]:
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

# ── Tab 3: Volume ────────────────────────────────────────────────────────────

with tabs[3]:
    col1, col2 = st.columns([10, 1])
    col1.caption("Trading volume over time for selected symbols.")

    with col2.popover(":material/tune:"):
        vol_dollar = st.toggle(
            label="Dollar volume",
            key=(key := "vol_dollar"),
            value=_default(key, fallback=False),
            on_change=lambda k=key: _persist(k),
            help="Show volume as price x shares (dollar volume) instead of raw share count.",
        )

        vol_log = st.toggle(
            label="Log scale",
            key=(key := "vol_log_scale"),
            value=_default(key, fallback=False),
            on_change=lambda k=key: _persist(k),
            help="Use a logarithmic scale for the y-axis.",
        )

    if active_tab == TAB_LABELS[3]:
        vol_bars = bars.copy()
        if vol_dollar:
            vol_bars["volume"] = vol_bars["volume"] * vol_bars["close"]
        else:
            vol_bars = vol_bars.drop(columns=["currency"], errors="ignore")

        fig = plot_volume(data=vol_bars, display=None)
        if vol_log:
            fig.update_yaxes(type="log")
        st.plotly_chart(fig, width="stretch")

# ── Tab 4: VWAP ──────────────────────────────────────────────────────────────

with tabs[4]:
    st.caption("Volume-Weighted Average Price compared to closing price.")

    if active_tab == TAB_LABELS[4]:
        st.plotly_chart(
            plot_vwap(data=bars, display=None),
            width="stretch",
        )

# ── Tab 5: Returns ───────────────────────────────────────────────────────────

with tabs[5]:
    col1, col2 = st.columns([10, 1])
    col1.caption("Distribution of period-over-period percentage returns.")

    with col2.popover(":material/tune:"):
        price_col = price_col_radio("price_col_dist")

        ret_log = st.toggle(
            label="Log scale",
            key=(key := "ret_log_scale"),
            value=_default(key, fallback=False),
            on_change=lambda k=key: _persist(k),
            help="Use a logarithmic scale for the y-axis.",
        )

    if active_tab == TAB_LABELS[5]:
        fig = plot_returns(data=bars, price_col=price_col, display=None)
        if ret_log:
            fig.update_yaxes(type="log")
        st.plotly_chart(fig, width="stretch")

# ── Tab 6: Seasonality ───────────────────────────────────────────────────────

with tabs[6]:
    col1, col2 = st.columns([10, 1])
    col1.caption("Monthly returns heatmap showing seasonal patterns.")

    with col2.popover(":material/tune:"):
        price_col = price_col_radio("price_col_season")

        season_symbol = st.selectbox(
            label="Symbol",
            key=(key := "season_symbol"),
            options=symbols,
            index=0,
            on_change=lambda k=key: _persist(k),
            help="Select the symbol to display in the seasonality heatmap.",
        )

    if active_tab == TAB_LABELS[6]:
        st.plotly_chart(
            plot_seasonality(
                data=bars, price_col=price_col, symbol=season_symbol, display=None
            ),
            width="stretch",
        )

# ── Tab 7: Drawdown ──────────────────────────────────────────────────────────

with tabs[7]:
    col1, col2 = st.columns([10, 1])
    col1.caption("Percentage drawdown from the running peak over time.")

    with col2.popover(":material/tune:"):
        price_col = price_col_radio("price_col_drawdown")

    if active_tab == TAB_LABELS[7]:
        st.plotly_chart(
            plot_drawdown(data=bars, price_col=price_col, display=None),
            width="stretch",
        )

# ── Tab 8: Correlation ───────────────────────────────────────────────────────

with tabs[8]:
    col1, col2 = st.columns([10, 1])
    col1.caption(
        "Pairwise correlation of returns across selected symbols. Select at least two symbols."
    )

    with col2.popover(":material/tune:"):
        price_col = price_col_radio("price_col_corr")

    if active_tab == TAB_LABELS[8]:
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

# ── Tab 9: Dividends ─────────────────────────────────────────────────────────

with tabs[9]:
    st.caption("Dividend payment history for selected symbols.")

    if active_tab == TAB_LABELS[9]:
        dividends = _to_pandas(query_dividends(symbol=symbols))

        if dividends.empty:
            st.info(
                "No dividend data available for the selected symbols.",
                icon=":material/info:",
            )
        else:
            dividends["dt"] = _ts_to_datetime(dividends["ex_date"], tz)
            dividends["currency"] = dividends["symbol"].map(
                lambda s: str(all_i[s].quote) if s in all_i else None
            )
            st.plotly_chart(
                plot_dividends(data=dividends, display=None),
                width="stretch",
            )
