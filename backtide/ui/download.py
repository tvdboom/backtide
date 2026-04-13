"""Backtide.

Author: Mavs
Description: Page to download new data.

"""

from datetime import datetime as dt
from datetime import timedelta
from zoneinfo import ZoneInfo

import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import (
    Currency,
    Exchange,
    InstrumentProfile,
    InstrumentType,
    Interval,
    download_instruments,
    resolve_profiles,
)
from backtide.ui.utils import (
    _fmt_number,
    _get_instrument_type_description,
    _get_logokit_url,
    _get_provider_logo,
    _list_instruments,
    _moment_to_strftime,
    _prevent_deselection,
    _to_upper_values,
)
from backtide.utils.constants import MAX_INSTRUMENT_SELECTION

# ─────────────────────────────────────────────────────────────────────────────
# Helper functionalities
# ─────────────────────────────────────────────────────────────────────────────

CARD_CSS = """
    <style>
        .section {
            font-size: 12px;
            font-weight: 600;
            color: #888;
            letter-spacing: 0.08em;
            text-transform: uppercase;
            margin: 18px 0 8px;
        }

        .card {
            position: relative;
            min-height: 215px;
            border: 1px solid rgba(0,0,0, 0.2);
            border-radius: 12px;
            padding: 1.2rem 1.4rem;
            margin-bottom: 10px;
        }

        .card-header {
            display: flex;
            align-items: center;
            gap: 14px;
            margin-bottom: 12px;
        }

        .logo {
            height: 64px;
            border-radius: 6px;
            margin-top: -4px;
        }

        .quote {
            height: 32px;
            margin-top: 4px;
        }

        .title {
            display: flex;
            flex-direction: column;
        }

        .symbol {
            font-size: 22px;
            font-weight: 700;
        }

        .flag {
            height: 20px;
            margin-top: -4px;
            margin-left: 12px;
        }

        .name {
            font-size: 20px;
            opacity: 0.7;
        }

        .badge {
            font-size: 16px;
            padding: 3px 8px;
            border-radius: 6px;
            background: rgba(250,250,250,0.07);
            border: 1px solid rgba(250,250,250,0.1);
            white-space: nowrap;
        }

        .badge.leg {
            background: rgba(99,179,237,0.12);
            color: #63b3ed;
            font-weight: 600;
        }

        .intervals {
            display: flex;
            flex-direction: column;
            gap: 6px;
            border-top: 1px solid rgba(250,250,250,0.08);
            padding-top: 10px;
        }

        .interval-row {
            display: grid;
            grid-template-columns: 60px 230px 80px 100px;
            gap: 12px;
            font-size: 13px;
        }

        .iv-label {
            font-weight: 600;
            font-size: 18px;
            opacity: 0.7;
            text-align: right;
        }

        .iv-range {
            font-size: 18px;
            text-align: right;
        }

        .iv-rows {
            font-size: 18px;
            opacity: 0.6;
            text-align: right;
        }

        .legs-row {
            display: flex;
            gap: 6px;
            flex-wrap: wrap;
            align-items: center;
            margin-top: 10px;
            padding-top: 10px;
            border-top: 1px solid rgba(250,250,250,0.08);
        }

        .meta-right {
            position: absolute;
            top: 1.2rem;
            right: 1.4rem;
            display: flex;
            flex-direction: column;
            align-items: flex-end;
            gap: 4px;
        }

        .provider {
            display: flex;
            align-items: center;
            gap: 6px;
            font-size: 12px;
        }

        .provider img {
            width: 60px;
            border-radius: 2px;
        }

        .meta-inline {
            display: flex;
            flex-direction: column;
            justify-content: center;
            gap: 1px;
            margin-top: 30px;
            margin-left: auto;
            text-align: right;
        }

        .meta-label {
            font-size: 14px;
            font-weight: 600;
            opacity: 0.5;
            text-transform: uppercase;
            letter-spacing: 0.06em;
        }

        .meta-value {
            margin-top: -5px;
            font-size: 18px;
        }
    </style>
    """


def draw_cards(profiles: list[InstrumentProfile]) -> int:
    """Generate HTML code to draw the instrument cards."""
    html = "<div class='section'></div>"

    get_flag = lambda code: f"https://flagcdn.com/80x60/{code.lower()}.png"
    parse_date = lambda date: date.strftime(_moment_to_strftime(cfg.display.date_format))

    total_rows = 0
    for profile in profiles:
        interval_rows = ""
        for interval in Interval.variants():
            start_iv = profile.earliest_ts.get(interval)
            end_iv = profile.latest_ts.get(interval)
            if not (start_iv and end_iv):
                continue

            iv_start = dt.fromtimestamp(start_iv, tz=tz).date()
            iv_end = dt.fromtimestamp(end_iv, tz=tz).date()
            if not full_history:
                iv_start = max(start_ts, iv_start)
                iv_end = min(end_ts, iv_end)

            # Estimate rows for this interval
            delta_minutes = max((iv_end - iv_start).total_seconds() / 60, 1)
            delta_days = (iv_end - iv_start).days

            if profile.instrument_type.is_equity:
                # Stocks / ETF markets open 8/5
                if interval.is_intraday():
                    rows = max(int(delta_minutes * (5 / 7) * (8 / 24) // interval.minutes()), 1)
                else:
                    rows = max(int(delta_days * (5 / 7) // (interval.minutes() / 1440)), 1)
            elif instrument_type == InstrumentType.Forex:
                # Forex markets open 24/5
                if interval.is_intraday():
                    rows = max(int(delta_minutes * (5 / 7) // interval.minutes()), 1)
                else:
                    rows = max(int(delta_days * (5 / 7) // (interval.minutes() / 1440)), 1)
            else:
                # Crypto markets open 24/7
                rows = max(int(delta_minutes // interval.minutes()), 1)

            total_rows += rows

            n_years = iv_end.year - iv_start.year

            # Adjust if end is before the anniversary
            anniversary = iv_start.replace(year=iv_start.year + n_years)
            if anniversary > iv_end:
                n_years -= 1
                anniversary = iv_start.replace(year=iv_start.year + n_years)

            # Remaining days after full years
            remaining_days = (iv_end - anniversary).days

            if n_years > 0:
                n_days_str = f"{n_years}y {remaining_days}d"
            else:
                n_days_str = f"{remaining_days}d"

            interval_rows += f"""
                <div class="interval-row">
                    <span class="iv-label">{interval}</span>
                    <span class="iv-range">
                        {parse_date(iv_start)} &nbsp → &nbsp {parse_date(iv_end)}
                    </span>
                    <span class="iv-range">{n_days_str}</span>
                    <span class="iv-rows">~{_fmt_number(rows)} bars</span>
                </div>"""

        if logokit_key := cfg.display.logokit_api_key:
            url = _get_logokit_url(profile.symbol, profile.instrument_type, logokit_key)
            logo = f"<img src='{url}' class='logo'>"
        else:
            logo = ""

        name = profile.name if profile.instrument_type.is_equity else ""

        legs = ""
        if profile.legs:
            badges = "".join(f'<span class="badge leg">{leg}</span>' for leg in profile.legs)
            legs = f'<div class="legs-row"><span style="font-size:16px">via</span>{badges}</div>'

        provider = str(cfg.data.providers[profile.instrument_type])
        provider_html = f"""
            <div class="provider">
                <img src="{_get_provider_logo(provider)}" alt="{provider}">
            </div>"""

        flag = ""
        meta_inline = ""
        if profile.instrument_type.is_equity:
            if isinstance(profile.exchange, Exchange):
                flag = f"<img src='{get_flag(profile.exchange.country.alpha2)}' class='flag'>"
                exchange = f"{profile.exchange.name} ({profile.exchange})"
            else:
                exchange = profile.exchange

            meta_inline = f"""
                <div class="meta-inline">
                    <span class="meta-label">Exchange</span>
                    <span class="meta-value">{exchange}</span>
                    <span class="meta-label" style="margin-top:8px;">Currency</span>
                    <span class="meta-value">{profile.quote}</span>
                </div>"""

        elif profile.instrument_type == InstrumentType.Crypto:
            if isinstance(profile.quote, Currency):
                img = get_flag(profile.quote.country.alpha2)
            elif logokit_key:
                img = _get_logokit_url(
                    profile.symbol, profile.instrument_type, logokit_key, use_quote=True
                )
            else:
                img = ""

            if img:
                meta_inline = f"""
                    <div class="meta-inline">
                        <span class="meta-label">Quote</span>
                        <span class="meta-value"><img src='{img}' class='quote'></span>
                    </div>"""

        html += f"""
            <div class="card">
              <div class="card-header">
                {logo}
                <div>
                    <div class="symbol">{profile.symbol}{flag}</div>
                    <div class="name">{name}</div>
                </div>
                <div class="meta-right">
                    {provider_html}
                    {meta_inline}
                </div>
              </div>
              <div class="intervals">{interval_rows}</div>
              {legs}
            </div>"""

    return html, total_rows


# ─────────────────────────────────────────────────────────────────────────────
# Download interface
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
if cfg.display.timezone:
    tz = ZoneInfo(cfg.display.timezone)
else:
    tz = dt.now().astimezone().tzinfo

st.set_page_config(page_title="Backtide - Download")

st.title("Download", text_alignment="center")

st.divider()

if st.session_state.get("instrument_type") is None:
    _cache = st.session_state.get("_cache", {})
    st.session_state.instrument_type = _cache.get("instrument_type", InstrumentType.get_default())

instrument_type = st.segmented_control(
    label="Instrument type",
    key="instrument_type",
    options=InstrumentType.variants(),
    default=st.session_state.instrument_type,
    format_func=lambda at: f"{at.icon()} {at}",
    on_change=_prevent_deselection(
        key="instrument_type",
        default=InstrumentType.get_default(),
        reset=["symbols", "_currency"],
    ),
    help="Select the type of financial instrument you want to download.",
)

all_instruments = _list_instruments(instrument_type)

# Filter instruments based on the selected currency
if not st.session_state.get("_currency"):
    st.session_state._currency = "All"

if currency := st.session_state.get("_currency"):
    filtered_instruments = [
        inst
        for inst in all_instruments
        if currency == "All" or inst.base == currency or str(inst.quote) == currency
    ]
else:
    filtered_instruments = all_instruments

col1, col2 = st.columns([5, 1], vertical_alignment="bottom")
instrument_d, currency_d = _get_instrument_type_description(instrument_type)

symbols = col1.multiselect(
    label="Symbols",
    key="symbols",
    options=sorted(filtered_instruments, key=lambda a: a.symbol),
    format_func=lambda a: (
        f"{a.symbol} - {a.name}"
        if a.instrument_type in (InstrumentType.Stocks, InstrumentType.Etf)
        else a.symbol
    ),
    placeholder="Select one or more symbols...",
    max_selections=MAX_INSTRUMENT_SELECTION,
    accept_new_options=True,
    on_change=_to_upper_values("symbols"),
    help=instrument_d,
)

# Symbols can become symbol - name when changing currency, so extract the symbol part
symbols = [s.split(" - ")[0] if isinstance(s, str) else s for s in symbols]

intervals = st.session_state.get("intervals", Interval.get_default())

try:
    # Convert custom symbols to instruments and add conversion currencies
    if symbols and intervals:
        profiles = resolve_profiles(symbols, instrument_type, intervals, verbose=False)
        direct = profiles[: len(symbols)]  # Direct profiles (no legs)
    else:
        profiles = None
        direct = None
except RuntimeError as ex:
    st.error(ex, icon=":material/error:")

options = ["All", *sorted(dict.fromkeys(str(inst.quote) for inst in all_instruments))]
col2.selectbox(
    label="Currency",
    index=options.index(st.session_state._currency),
    key="currency",
    options=options,
    on_change=lambda: st.session_state.update(_currency=st.session_state.currency),
    placeholder="All",
    help=currency_d,
)

full_history = st.toggle(
    label="Download full history",
    value=True,
    help=(
        "Whether to download the maximum available history for all selected symbols and FX rates. "
        "If toggled off, select the start and end download dates."
    ),
)

today = dt.now(tz=tz).date()
if profiles and intervals:
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
        value=earliest_ts,
        min_value=earliest_ts,
        max_value="today",
        format=cfg.display.date_format,
        help=(
            "Download data starting from this date. A download can start later if the "
            "provider doesn't have the data this far back, but it can't start earlier."
        ),
    )

    end_ts = col2.date_input(
        label="End date",
        value=latest_ts,
        min_value=start_ts + timedelta(days=1),
        max_value="today",
        format=cfg.display.date_format,
        help="Download data up to this date.",
    )

intervals = st.pills(
    label="Interval",
    key="intervals",
    options=cfg.data.providers[instrument_type].intervals(),
    selection_mode="multi",
    default=Interval.get_default(),
    help=(
        "The frequency of the data points to download. Note that full history is "
        "only available for intervals >= 1d."
    ),
)

is_enabled = profiles and start_ts and latest_ts and intervals

if is_enabled:
    BYTES_PER_ROW = 120  # Estimated memory required per OHLC bar
    ROWS_PER_SECOND = 40_000  # Estimated number of rows downloaded per second

    st.divider()

    with st.expander("Download details", icon=":material/archive:", expanded=False):
        html, n_bars = draw_cards(profiles)
        st.html(CARD_CSS + html)

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
    icon=":material/get_app:",
    type="primary",
    disabled=not is_enabled or downloading,
    shortcut="Enter",
    width="stretch",
    key="downloading",
):
    if latest_ts > dt.now(tz=tz).date():
        st.error("End date cannot be in the future.", icon=":material/error:")
    elif start_ts > latest_ts:  # ty:ignore[unsupported-operator]
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
                result = download_instruments(
                    profiles, start=dl_start, end=dl_end, verbose=False
                )
        except RuntimeError as ex:
            st.error(f"Download error: {ex}", icon=":material/error:")
        else:
            for warn in result.warnings:
                st.warning(warn, icon=":material/warning:")

            n_total = result.n_succeeded + result.n_failed

            if result.n_failed and result.n_succeeded:
                st.success(
                    f"Successfully downloaded {result.n_succeeded} of {n_total} instruments.",
                    icon=":material/check_circle:",
                )
            elif result.n_failed:
                st.error(
                    f"All {n_total} instruments had warnings during download.",
                    icon=":material/error:",
                )
            else:
                st.success(
                    f"Successfully downloaded {result.n_succeeded} instruments.",
                    icon=":material/check_circle:",
                )
