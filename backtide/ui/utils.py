"""Backtide.

Author: Mavs
Description: Utility functions for the UI.

"""

import base64
from datetime import datetime as dt
from pathlib import Path
import re
from typing import Any
from zoneinfo import ZoneInfo

import streamlit as st

from backtide.constants import MOMENT_TO_STRFTIME
from backtide.core.data import Instrument, InstrumentType, list_instruments
from backtide.utils.constants import MAX_PRELOADED_INSTRUMENTS
from backtide.utils.utils import to_list


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
def _get_provider_logo(provider: str) -> str:
    """Load the logo image from a provider."""
    path = Path(f"images/providers/{provider.lower()}.png")
    data = base64.b64encode(path.read_bytes()).decode()
    return f"data:image/png;base64,{data}"


@st.cache_resource(ttl=3600, show_spinner=False)
def _list_instruments(instrument_type: InstrumentType) -> list[Instrument]:
    """Cache the major instruments per instrument type."""
    if instrument_type is None:
        instrument_type = InstrumentType.get_default()
    return list_instruments(instrument_type, MAX_PRELOADED_INSTRUMENTS)


def _moment_to_strftime(fmt: str) -> str:
    """Convert a momentjs string to strftime format."""
    regex = re.compile(
        "|".join(sorted(map(re.escape, MOMENT_TO_STRFTIME.keys()), key=len, reverse=True)),
    )

    def replace(match: re.Match) -> str:
        """Replace a token in the string."""
        token = match.group(0)
        return MOMENT_TO_STRFTIME.get(token, token)

    return regex.sub(replace, fmt)


def _parse_date(ts: int, fmt: str, tz: ZoneInfo) -> str:
    """Format a Unix timestamp into the user's date format."""
    fmt = _moment_to_strftime(fmt)
    return dt.fromtimestamp(ts, tz=tz).strftime(fmt)


def _prevent_deselection(key: str, default: Any, reset: list[str] | None = None):
    """On-change function to call for widgets for which a valid must be selected.

    Additionally, remove entries in the `reset` keys from Streamlit's state.

    """
    if "_cache" not in st.session_state:
        st.session_state["_cache"] = {}
    cache = st.session_state["_cache"]

    if st.session_state.get(key) is None:
        st.session_state[key] = cache.get(key, default)
    else:
        if reset and cache.get(key) != st.session_state[key]:
            for k in reset:
                st.session_state.pop(k, None)

        cache[key] = st.session_state[key]


def _to_upper_values(key: str):
    """Convert values in a streamlit state to uppercase."""
    if key in st.session_state:
        st.session_state[key] = [
            s.upper() if isinstance(s, str) else s for s in to_list(st.session_state[key])
        ]
