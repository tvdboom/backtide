"""Backtide.

Author: Mavs
Description: Utility functions for the UI.

"""

import base64
from datetime import date
from datetime import datetime as dt
import json
from pathlib import Path
import re
from typing import TYPE_CHECKING, Any
from zoneinfo import ZoneInfo

import pandas as pd
import streamlit as st
from tzlocal import get_localzone

from backtide.config import Config
from backtide.core.data import (
    Currency,
    Exchange,
    Instrument,
    InstrumentProfile,
    InstrumentType,
    Interval,
    Provider,
    list_instruments,
)
from backtide.core.storage import (
    query_bars,
    query_bars_summary,
)
from backtide.utils.constants import (
    MAX_PRELOADED_INSTRUMENTS,
    MOMENT_TO_STRFTIME,
)
from backtide.utils.utils import _to_list

if TYPE_CHECKING:
    from backtide.ui.indicators import SavedIndicator


# ─────────────────────────────────────────────────────────────────────────────
# Utility constants
# ─────────────────────────────────────────────────────────────────────────────

_CODE_OPTIONS = [":material/code: Code editor", ":material/upload_file: Upload file"]


# ─────────────────────────────────────────────────────────────────────────────
# Utility functions
# ─────────────────────────────────────────────────────────────────────────────


def _clear_state(*keys: str):
    """Remove `keys` from Streamlit's state (including shadow keys)."""
    for k in keys:
        st.session_state[k] = []
        st.session_state.pop(f"_{k}", None)


def _default(key: str, fallback: Any = None) -> Any:
    """Return the persisted shadow value for *key*, or *fallback*."""
    return st.session_state.get(f"_{key}", fallback)


def _fmt_number(n: float) -> str:
    """Nicely format a number."""
    if n > 10_000_000:
        return f"{n / 1_000_000:.1f}M"
    elif n > 1_000_000:
        return f"{n / 1_000_000:.2f}M"
    elif n >= 1_000:
        return f"{n / 1_000:.1f}k"
    else:
        return str(n)


def _get_timezone(tz: str | None) -> ZoneInfo:
    """Return the timezone from config or local."""
    if tz:
        return ZoneInfo(tz)
    else:
        return get_localzone()


def _get_instrument_type_description(instrument_type: InstrumentType) -> tuple[str, str]:
    """Get the description of a given instrument type for the symbol and currency."""
    match instrument_type:
        case InstrumentType.Stocks:
            instrument_description = (
                "List of stock tickers. The preloaded options are the primary listings "
                "for companies in major indices, but any valid stock ticker can be added."
            )
            currency_description = "Filter the preloaded symbols by their denominated currency."
        case InstrumentType.Etf:
            instrument_description = (
                "List of ETF tickers. The preloaded options are frequently traded ETFs, but "
                "any valid ETF ticker can be added."
            )
            currency_description = "Filter the preloaded symbols by their denominated currency."
        case InstrumentType.Forex:
            instrument_description = (
                "List of currency pairs. The preloaded options are frequently traded pairs, "
                "but any valid forex symbol can be added."
            )
            currency_description = "Filter the preloaded pairs by their quote currency."
        case InstrumentType.Crypto:
            instrument_description = (
                "List of cryptocurrency pairs. The preloaded options are frequently traded "
                "pairs, but any valid crypto symbol can be added."
            )
            currency_description = "Filter the preloaded symbols by their quote currency."

    return instrument_description, currency_description


def _get_logokit_url(
    symbol: str,
    it: InstrumentType,
    api_key: str,
    *,
    use_quote: bool = False,
) -> str:
    """Build a Logokit URL from a canonical symbol and its instrument type."""
    match it:
        case InstrumentType.Forex:
            domain = "ticker"
            base, quote = symbol.split("-")  # Canonical forex symbol has form base-quote
            symbol = f"{base}{quote}:CUR"
        case InstrumentType.Crypto:
            domain = "crypto"
            base, quote = symbol.split("-")  # Canonical crypto symbol has form base-quote
            symbol = base if not use_quote else quote
        case _:
            domain = "ticker"

    return f"https://img.logokit.com/{domain}/{symbol}?token={api_key}"


@st.cache_data
def _get_provider_logo(provider: Provider) -> str:
    """Load the logo image from a provider."""
    path = Path(f"images/providers/{provider}.png")
    data = base64.b64encode(path.read_bytes()).decode()
    return f"data:image/png;base64,{data}"


@st.cache_data(show_spinner="Fetching instruments...")
def _list_instruments(
    instrument_type: InstrumentType,
    *,
    limit: int = MAX_PRELOADED_INSTRUMENTS,
) -> dict[str, Instrument]:
    """Return available instruments for the given type."""
    return {x.symbol: x for x in list_instruments(instrument_type, limit=limit, verbose=False)}


def _load_stored_indicators(cfg: Config) -> list[SavedIndicator]:
    """Load and return the indicators from storage."""
    from backtide.ui.indicators import SavedIndicator

    path = Path(cfg.data.storage_path) / "indicators"

    indicators = []
    for f in path.glob("*.json"):
        try:
            data = json.loads(f.read_text(encoding="utf-8"))
            indicators.append(SavedIndicator(**data))
        except (json.JSONDecodeError, TypeError) as ex:
            st.error(f"Failed to load indicator **{f}**. Exception: {ex}")

    return sorted(indicators, key=lambda x: x.name)


def _moment_to_strftime(fmt: str) -> str:
    """Convert a momentjs string to strftime format."""
    tokens = [re.escape(k) for k in MOMENT_TO_STRFTIME]
    tokens.sort(key=len, reverse=True)
    regex = re.compile("|".join(tokens))

    def replace(match: re.Match) -> str:
        """Replace a token in the string."""
        token = match.group(0)
        return MOMENT_TO_STRFTIME.get(token, token)

    return regex.sub(replace, fmt)


def _parse_date(ts: int, fmt: str, tz: ZoneInfo) -> str:
    """Format a Unix timestamp into the user's date format."""
    fmt = _moment_to_strftime(fmt)
    return dt.fromtimestamp(ts, tz=tz).strftime(fmt)


def _persist(*keys: str):
    """Copy widget values to shadow keys so they survive page navigation."""
    for k in keys:
        if k in st.session_state:
            st.session_state[f"_{k}"] = st.session_state[k]


@st.cache_data(show_spinner="Loading stored data...")
def _query_bars_summary() -> pd.DataFrame:
    """Load and cache the raw storage summary from the database."""
    return _to_pandas(query_bars_summary())


def _to_pandas(df: Any) -> pd.DataFrame:
    """Ensure a DataFrame is pandas, converting from polars if needed."""
    if hasattr(df, "to_pandas"):
        return df.to_pandas()

    return df


def _to_upper_values(key: str):
    """Convert values in a streamlit state to uppercase."""
    if key in st.session_state:
        st.session_state[key] = [
            s.upper() if isinstance(s, str) else s for s in _to_list(st.session_state[key])
        ]


# ─────────────────────────────────────────────────────────────────────────────
# Instrument card rendering
# ─────────────────────────────────────────────────────────────────────────────

_CARD_CSS = """
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


def _draw_cards(
    profiles: list[InstrumentProfile],
    *,
    cfg: Config,
    tz: ZoneInfo,
    instrument_type: InstrumentType,
    full_history: bool,
    start_ts: date,
    end_ts: date,
    estimate_rows: bool,
) -> tuple[str, int]:
    """Generate HTML code to draw the instrument cards.

    Returns the HTML string and the (estimated) total number of bars.

    """
    html = "<div class='section'></div>"

    get_flag = lambda code: f"https://flagcdn.com/80x60/{code.lower()}.png"
    parse_date = lambda date: date.strftime(_moment_to_strftime(cfg.display.date_format))

    # Pre-fetch all bars from storage in one query when not estimating
    if not estimate_rows:
        all_bars = _to_pandas(
            query_bars(
                symbol=[p.symbol for p in profiles],
                interval=next(iter(profiles[0].earliest_ts)),
                provider=profiles[0].provider,
            )
        )

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

            if estimate_rows:
                # Estimate rows for this interval
                delta_minutes = max((iv_end - iv_start).total_seconds() / 60, 1)
                delta_days = (iv_end - iv_start).days

                if profile.instrument_type.is_equity:
                    # Stocks / ETF markets open 8/5
                    if interval.is_intraday():
                        rows = max(
                            int(delta_minutes * (5 / 7) * (8 / 24) // interval.minutes()), 1
                        )
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
            else:
                # Filter pre-fetched bars for this symbol/interval and count within range
                bars = all_bars[all_bars["symbol"] == profile.symbol]
                iv_start_ts = int(dt.combine(iv_start, dt.min.time(), tzinfo=tz).timestamp())
                iv_end_ts = int(dt.combine(iv_end, dt.max.time(), tzinfo=tz).timestamp())
                rows = ((bars["open_ts"] >= iv_start_ts) & (bars["open_ts"] <= iv_end_ts)).sum()

            total_rows += rows

            n_years = iv_end.year - iv_start.year

            # Adjust if end is before the anniversary
            anniversary = iv_start.replace(year=iv_start.year + n_years)
            if anniversary > iv_end:
                n_years -= 1
                anniversary = iv_start.replace(year=iv_start.year + n_years)

            # Remaining days after full years (+1 since both start and end are inclusive)
            remaining_days = (iv_end - anniversary).days + 1

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
                    <span class="iv-rows">
                        {"~" if estimate_rows else ""}{_fmt_number(rows)} bars
                    </span>
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

        provider_html = f"""
            <div class="provider">
                <img src="{_get_provider_logo(profile.provider)}" alt="{profile.provider}">
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
