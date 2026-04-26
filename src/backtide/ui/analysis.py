"""Backtide.

Author: Mavs
Description: Data analysis page for exploring stored market data.

"""

from datetime import date, timedelta

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
    _query_bars_summary,
    _to_upper_values,
)
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
    max_selections=5,
    on_change=lambda k=key: (_to_upper_values("symbols"), _persist(k)),
    help="Select the symbols to analyze. Only symbols available in the database are shown.",
)

if not symbols:
    st.warning("Select at least one symbol to begin the analysis.", icon=":material/warning:")
    st.stop()

# Determine which intervals are available for all selected symbols
summary = _query_bars_summary()
summary = summary[
    summary.apply(
        lambda r: (
            r["symbol"] in symbols
            and r["provider"] == str(cfg.data.providers[all_i[r["symbol"]].instrument_type])
        ),
        axis=1,
    )
]

# Compute per-symbol interval sets and find the common ones
intervals_per_sym = summary.groupby("symbol")["interval"].apply(set)
if intervals_per_sym.empty:
    st.warning("No data available for the selected symbols.", icon=":material/warning:")
    st.stop()

common_intervals = set.intersection(*intervals_per_sym.values)

# Build ordered list of available intervals (preserving the canonical order)
available_intervals = [i for i in Interval.variants() if str(i) in common_intervals]

if not available_intervals:
    # Show which intervals each symbol has
    per_sym_info = "\n".join(
        f"* **{sym}**: {', '.join(sorted(ivs))}" for sym, ivs in intervals_per_sym.items()
    )
    st.warning(
        "The selected symbols have no intervals in common. "
        f"Available intervals per symbol:\n{per_sym_info}",
        icon=":material/warning:",
    )
    st.stop()

col1, col2 = st.columns([1, 1.6])

with col1:
    _default_interval = _default(key := "interval", Interval.get_default())
    if _default_interval not in available_intervals and available_intervals:
        _default_interval = available_intervals[0]

    interval = st.pills(
        label="Interval",
        key=key,
        required=True,
        options=available_intervals,
        selection_mode="single",
        default=_default_interval,
        on_change=lambda k=key: _persist(k),
        help=(
            "Interval for which to analyze the data. Only intervals "
            "available for all selected symbols are shown."
        ),
    )

with col2:
    period = st.pills(
        label="Period",
        key=(key := "period"),
        options=["Max", "5Y", "1Y", "YTD", "6M", "3M", "1M", "7d", "1d"],
        selection_mode="single",
        default=_default(key, "Max"),
        on_change=lambda k=key: _persist(k),
        help="Filter data to a specific time window.",
    )

    if not period:
        custom_range = st.date_input(
            label="Date range",
            key=(key := "custom_date_range"),
            value=_default(key, (date.today() - timedelta(days=30), date.today())),
            format=cfg.display.date_format,
            on_change=lambda k=key: _persist(k),
            help="Select a custom date range for the analysis.",
        )
    else:
        custom_range = None

bars = _load_bars(symbols, interval)

if bars.empty:
    st.warning(
        "No bars found for the selected symbols and interval.",
        icon=":material/warning:",
    )
    st.stop()


# Select only data from configured providers per symbol
bars = bars[
    bars.apply(
        lambda r: r["provider"] == str(cfg.data.providers[all_i[r["symbol"]].instrument_type]),
        axis=1,
    )
]

# Add datetime column for plotting
bars["dt"] = _ts_to_datetime(bars["open_ts"], tz)

# ── Apply period filter ──────────────────────────────────────────────────────

today = pd.Timestamp.now(tz=tz)
if isinstance(custom_range, tuple) and len(custom_range) == 2:
    bars = bars[(bars["dt"].dt.date >= custom_range[0]) & (bars["dt"].dt.date <= custom_range[1])]
elif period != "Max":
    offsets = {
        "5Y": timedelta(days=5 * 365),
        "1Y": timedelta(days=365),
        "6M": timedelta(days=182),
        "3M": timedelta(days=91),
        "1M": timedelta(days=30),
        "7d": timedelta(days=7),
        "1d": timedelta(days=1),
    }

    if period == "YTD":
        cutoff = pd.Timestamp(date(today.year, 1, 1), tz=tz)
    else:
        cutoff = today - offsets[period]

    bars = bars[bars["dt"] >= cutoff]

if bars.empty:
    st.warning("No bars found for the selected period.", icon=":material/warning:")
    st.stop()


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

# Shared price column selection across all tabs
PRICE_COL_KEYS = [
    "price_col_summary",
    "price_col_price",
    "price_col_dist",
    "price_col_season",
    "price_col_drawdown",
    "price_col_corr",
]

if "_price_col" not in st.session_state:
    st.session_state["_price_col"] = _default("_price_col", "adj_close")

def _sync_price_col(key: str) -> None:
    """Sync a tab-specific radio key to the shared _price_col and all other radios."""
    st.session_state["_price_col"] = st.session_state[key]
    for k in PRICE_COL_KEYS:
        st.session_state[k] = st.session_state[key]
    _persist("_price_col")


price_col_radio = lambda key: st.radio(
    label="Price",
    key=key,
    options=PRICE_COLUMNS,
    index=list(PRICE_COLUMNS).index(st.session_state.get(key, st.session_state["_price_col"])),
    format_func=lambda c: PRICE_COLUMNS[c],
    horizontal=False,
    on_change=_sync_price_col,
    args=(key,),
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

        # ── Metric cards ─────────────────────────────────────────────────
        n = len(stats_df)

        for _, row in stats_df.iterrows():
            sym = row["Symbol"]
            inst = all_i.get(sym)
            name = inst.name if inst else sym

            with st.container(border=True):
                # Header row: logo + symbol + name (left) | metrics (right)
                header_col, *metric_cols = st.columns([2, 1, 1, 1, 1])

                with header_col:
                    if logokit_key and inst:
                        logo_url = _get_logokit_url(sym, inst.instrument_type, logokit_key)
                        st.markdown(
                            f'<div style="display:flex;align-items:center;gap:10px">'
                            f'<img src="{logo_url}" width="32" height="32" '
                            f'style="border-radius:6px">'
                            f"<div>"
                            f'<span style="font-size:1.15em;font-weight:600">{sym}</span><br>'
                            f'<span style="font-size:0.78em;color:gray">{name}</span>'
                            f"</div></div>",
                            unsafe_allow_html=True,
                        )
                    else:
                        st.markdown(f"**{sym}**")
                        st.caption(name)

                ret = row["Ann. Return"]
                sharpe = row["Sharpe"]
                max_dd = row["Max Drawdown"]
                win_rate = row["Win Rate"]

                _g = "color:green"
                _r = "color:red"

                with metric_cols[0]:
                    _c = _g if ret >= 0 else _r
                    st.markdown(
                        f":material/trending_up: **Return**<br>"
                        f'<span style="font-size:1.3em;{_c}">{ret:+.2f}%</span>',
                        unsafe_allow_html=True,
                    )

                with metric_cols[1]:
                    _c = _g if sharpe >= 1 else (_r if sharpe < 0 else "")
                    st.markdown(
                        f":material/speed: **Sharpe**<br>"
                        f'<span style="font-size:1.3em;{_c}">{sharpe:.2f}</span>',
                        unsafe_allow_html=True,
                    )

                with metric_cols[2]:
                    _c = _r if max_dd < -20 else (_g if max_dd > -5 else "")
                    st.markdown(
                        f":material/trending_down: **Max Drawdown**<br>"
                        f'<span style="font-size:1.3em;{_c}">{max_dd:.2f}%</span>',
                        unsafe_allow_html=True,
                    )

                with metric_cols[3]:
                    _c = _g if win_rate >= 50 else _r
                    st.markdown(
                        f":material/trophy: **Win Rate**<br>"
                        f'<span style="font-size:1.3em;{_c}">{win_rate:.1f}%</span>',
                        unsafe_allow_html=True,
                    )

        # ── Full table in expander ────────────────────────────────────────

        with st.expander("Full statistics table", icon=":material/table:"):
            summary_column_config = {
                "Symbol": st.column_config.TextColumn(
                    pinned=True,
                    help="Ticker symbol of the instrument.",
                ),
                "Ann. Return": st.column_config.NumberColumn(
                    format="%+.2f%%",
                    help="Compound annual growth rate (CAGR).",
                ),
                "Ann. Volatility": st.column_config.NumberColumn(
                    format="%.2f%%",
                    help="Annualized standard deviation of returns.",
                ),
                "Sharpe": st.column_config.NumberColumn(
                    width="small",
                    format="%.2f",
                    help="Sharpe ratio: risk-adjusted return per unit of total volatility.",
                ),
                "Sortino": st.column_config.NumberColumn(
                    width="small",
                    format="%.2f",
                    help="Sortino ratio: like Sharpe but only penalizes downside volatility.",
                ),
                "Max Drawdown": st.column_config.NumberColumn(
                    format="%.2f%%",
                    help="Largest peak-to-trough decline in cumulative returns.",
                ),
                "Win Rate": st.column_config.NumberColumn(
                    format="%.1f%%",
                    help="Percentage of periods with a positive return.",
                ),
                "Total Bars": st.column_config.NumberColumn(
                    format="%d",
                    help="Total number of bars for this symbol at the selected interval.",
                ),
            }

            if logokit_key:
                stats_df.index = pd.Index(
                    data=stats_df.apply(
                        lambda r: (
                            _get_logokit_url(
                                r["Symbol"],
                                all_i[r["Symbol"]].instrument_type,
                                logokit_key,
                            )
                            if r["Symbol"] in all_i
                            else ""
                        ),
                        axis=1,
                    ),
                    name="Logo",
                )
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

    with col2.popover(":material/tune:"):
        cs_rangeslider = st.toggle(
            label="Range slider",
            key=(key := "cs_rangeslider"),
            value=_default(key, fallback=True),
            on_change=lambda k=key: _persist(k),
            help="Hide/show the range slider below the chart.",
        )

    if active_tab == TAB_LABELS[2]:
        st.plotly_chart(
            plot_candlestick(
                bars,
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
            plot_seasonality(data=bars, price_col=price_col, symbol=season_symbol, display=None),
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
