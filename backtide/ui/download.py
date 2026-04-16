"""Backtide.

Author: Mavs
Description: Page to download new data.

"""

from datetime import datetime as dt
from datetime import timedelta

import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import (
    InstrumentType,
    Interval,
    download_bars,
    resolve_profiles,
)
from backtide.ui.utils import (
    _CARD_CSS,
    _clear_state,
    _default,
    _draw_cards,
    _fmt_number,
    _get_instrument_type_description,
    _get_timezone,
    _list_instruments,
    _persist,
    _to_upper_values,
)
from backtide.utils.constants import MAX_INSTRUMENT_SELECTION

# ─────────────────────────────────────────────────────────────────────────────
# Download interface
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
tz = _get_timezone(cfg.display.timezone)

st.set_page_config(page_title="Backtide - Download")

st.title("Download", text_alignment="center")

st.divider()

instrument_type = st.segmented_control(  # ty: ignore[no-matching-overload]
    label="Instrument type",
    key=(key := "instrument_type"),
    required=True,
    options=InstrumentType.variants(),
    default=_default(key, InstrumentType.get_default()),
    format_func=lambda x: f"{x.icon()} {x}",
    on_change=lambda k=key: (_clear_state("symbols", "currency"), _persist(k)),
    help="Select the type of financial instrument you want to download.",
)

all_instruments = _list_instruments(instrument_type)

# Filter instruments based on the selected currency
if not st.session_state.get("_currency"):
    st.session_state._currency = "All"

if (currency := st.session_state.get("_currency", "All")) != "All":
    fi = {
        k: v for k, v in all_instruments.items() if v.base == currency or str(v.quote) == currency
    }
else:
    fi = all_instruments

col1, col2 = st.columns([5, 1], vertical_alignment="bottom")
symbol_d, currency_d = _get_instrument_type_description(instrument_type)

symbols = col1.multiselect(
    label="Symbols",
    key=(key := "symbols"),
    options=sorted(list(fi) + _default(key, [])),
    default=_default(key, []),
    format_func=lambda s: (
        f"{s} - {fi[s].name}" if s in fi and fi[s].instrument_type.is_equity else s
    ),
    placeholder="Select one or more symbols...",
    max_selections=MAX_INSTRUMENT_SELECTION,
    accept_new_options=True,
    on_change=lambda: (_to_upper_values("symbols"), _persist("symbols")),
    help=symbol_d,
)

# Symbols can become 'symbol - name' when changing currency -> extract the symbol
symbols = [s.split(" - ")[0] if isinstance(s, str) else s for s in symbols]

profiles = direct = None
intervals = _default("intervals", Interval.get_default())
try:
    if symbols and intervals:
        profiles = resolve_profiles(symbols, instrument_type, intervals, verbose=False)
        direct = profiles[: len(symbols)]  # Direct profiles (no legs)
except RuntimeError as ex:
    st.error(ex, icon=":material/error:")

options = ["All", *sorted(dict.fromkeys(str(x.quote) for x in all_instruments.values()))]
col2.selectbox(
    label="Currency",
    key=(key := "currency"),
    options=options,
    index=options.index(_default(key)),
    placeholder="All",
    on_change=lambda k=key: _persist(k),
    help=currency_d,
)

full_history = st.toggle(
    label="Download full history",
    key=(key := "full_history"),
    value=_default(key, fallback=True),
    on_change=lambda k=key: _persist(k),
    help=(
        "Whether to download the maximum available history for all selected symbols and FX rates. "
        "If toggled off, select the start and end download dates."
    ),
)

today = dt.now(tz=tz).date()
if profiles and intervals and direct:
    earliest_ts = dt.fromtimestamp(min(min(p.earliest_ts.values()) for p in direct), tz=tz).date()
    latest_ts = dt.fromtimestamp(max(max(p.latest_ts.values()) for p in direct), tz=tz).date()
else:
    earliest_ts = dt(2000, 1, 1, tzinfo=tz).date()
    latest_ts = today

# Correct latest_ts since some providers return closing bar at 00:00 (so tomorrow)
latest_ts = min(latest_ts, today)

if full_history:
    start_ts = earliest_ts
    end_ts = latest_ts
else:
    col1, col2 = st.columns(2)

    start_ts = col1.date_input(
        label="Start date",
        key=(key := "start_date"),
        value=_default(key, earliest_ts),
        min_value=earliest_ts,
        max_value="today",
        format=cfg.display.date_format,
        on_change=lambda k=key: _persist(k),
        help=(
            "Download data starting from this date. A download can start later if the "
            "provider doesn't have the data this far back, but it can't start earlier."
        ),
    )

    end_ts = col2.date_input(
        label="End date",
        key=(key := "end_date"),
        value=_default(key, latest_ts),
        min_value=start_ts + timedelta(days=1),
        max_value="today",
        format=cfg.display.date_format,
        on_change=lambda k=key: _persist(k),
        help="Download data up to this date.",
    )

intervals = st.pills(
    label="Interval",
    key=(key := "intervals"),
    options=cfg.data.providers[instrument_type].intervals(),
    selection_mode="multi",
    default=_default(key, Interval.get_default()),
    on_change=lambda k=key: _persist(k),
    help=(
        "The frequency of the data points to download. Note that full history is "
        "only available for intervals >= 1d."
    ),
)

if profiles and intervals:
    BYTES_PER_ROW = 120  # Estimated memory required per OHLC bar
    ROWS_PER_SECOND = 40_000  # Estimated number of rows downloaded per second

    st.divider()

    with st.expander(
        label="Download details",
        key=(key := "details_expander"),
        icon=":material/archive:",
        expanded=bool(_default(key)),
        on_change=lambda k=key: _persist(k),
    ):
        html, n_bars = _draw_cards(
            profiles,
            cfg=cfg,
            tz=tz,
            instrument_type=instrument_type,
            full_history=full_history,
            start_ts=start_ts,
            end_ts=end_ts,
        )
        st.html(_CARD_CSS + html)

    estimated_memory = (n_bars * BYTES_PER_ROW) / (1024**2)
    estimated_seconds = int(n_bars / ROWS_PER_SECOND)

    hours, remainder = divmod(estimated_seconds, 3600)
    minutes, seconds = divmod(remainder, 60)

    if hours:
        time_str = f"{hours}h {minutes}m"
    elif minutes:
        time_str = f"{minutes}m {seconds}s"
    elif seconds:
        time_str = f"{seconds}s"
    else:
        time_str = "<1s"

    if estimated_memory >= 1024:
        size_str = f"{estimated_memory / 1024:.2f} GB"
    elif estimated_memory >= 1:
        size_str = f"{estimated_memory:.1f} MB"
    else:
        size_str = "<0.1 MB"

    col1, col2, col3 = st.columns(3)
    col1.metric(":material/candlestick_chart: Estimated bars", _fmt_number(n_bars), border=True)
    col2.metric(":material/timer: Estimated time", time_str, border=True)
    col3.metric(":material/memory: Estimated memory", size_str, border=True)

st.divider()


# ─────────────────────────────────────────────────────────────────────────────
# Download logic
# ─────────────────────────────────────────────────────────────────────────────

downloading = st.session_state.get("downloading", False)

if st.button(
    label="Downloading..." if downloading else "Download",
    key="downloading",
    icon=":material/get_app:",
    type="primary",
    disabled=not (profiles and start_ts and latest_ts and intervals) or downloading,
    shortcut="Enter",
    width="stretch",
):
    if latest_ts > dt.now(tz=tz).date():
        st.error("End date cannot be in the future.", icon=":material/error:")
    elif start_ts > latest_ts:
        st.error("Start date must be equal or prior to end date.", icon=":material/error:")
    else:
        try:
            # Convert date range to Unix timestamps for the download.
            # When full_history is on, pass None to use the full provider range.
            if full_history:
                dl_start = dl_end = None
            else:
                dl_start = int(dt.combine(start_ts, dt.min.time(), tzinfo=tz).timestamp())
                dl_end = int(dt.combine(end_ts, dt.min.time(), tzinfo=tz).timestamp())

            with st.spinner("Downloading data..."):
                result = download_bars(profiles, start=dl_start, end=dl_end, verbose=False)
        except RuntimeError as ex:
            st.error(f"Download error: {ex}", icon=":material/error:")
        else:
            # Invalidate the storage cache so new bars become visible.
            st.cache_data.clear()

            for warn in result.warnings:
                st.warning(warn, icon=":material/warning:")

            n_total = result.n_succeeded + result.n_failed

            if result.n_failed and result.n_succeeded:
                st.success(
                    f"Successfully downloaded {result.n_succeeded} of {n_total} series.",
                    icon=":material/check_circle:",
                )
            elif result.n_failed:
                st.error(
                    f"All {n_total} series had warnings during download.",
                    icon=":material/error:",
                )
            else:
                st.success(
                    f"Successfully downloaded {result.n_succeeded} series.",
                    icon=":material/check_circle:",
                )
