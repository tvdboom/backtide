"""Backtide.

Author: Mavs
Description: Utility functions for the UI.

"""

from typing import Any

import streamlit as st

from backtide.core.data import Asset, AssetType
from backtide.utils.utils import to_list


def _get_asset_type_description(asset_type: AssetType) -> tuple[str, str]:
    """Get the description of a given asset type for the symbol and currency."""
    match asset_type:
        case AssetType.Stocks:
            asset_description = (
                "List of stock tickers. The preloaded options are the primary listings "
                "for companies in major indices, but any valid stock ticker can be added."
            )
            currency_description = "Filter the preloaded symbols by their denominated currency."
        case AssetType.Etf:
            asset_description = (
                "List of ETF tickers. The preloaded options are frequently traded ETFs, but "
                "any valid ETF ticker can be added."
            )
            currency_description = "Filter the preloaded symbols by their denominated currency."
        case AssetType.Forex:
            asset_description = (
                "List of currency pairs. The preloaded options are frequently traded pairs, "
                "but any valid forex symbol can be added."
            )
            currency_description = "Filter the preloaded pairs by their base/quote currencies."
        case AssetType.Crypto:
            asset_description = (
                "List of cryptocurrency pairs. The preloaded options are frequently traded "
                "pairs, but any valid crypto symbol can be added."
            )
            currency_description = "Filter the preloaded symbols by their base/quote currencies."

    return asset_description, currency_description


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


def _get_logokit_url(asset: Asset, api_key: str) -> str:
    """Retrieve the Logokit url to retrieve the logo for an asset."""
    match asset.asset_type:
        case AssetType.Forex:
            url = "ticker"
            symbol = f"{asset.base}{asset.quote}:CUR"
        case AssetType.Crypto:
            url = "crypto"
            symbol = asset.base
        case _:
            url = "ticker"
            symbol = asset.symbol

    return f"https://img.logokit.com/{url}/{symbol}?token={api_key}"


def _prevent_deselection(key: str, default: Any, reset: list[str] | None = None):
    """On-change function to call for widgets for which a valid must be selected.

    Additionally, remove entries in the `reset` keys from streamlit's state.

    """
    if "_cache" not in st.session_state:
        st.session_state["_cache"] = {}
    cache = st.session_state["_cache"]

    if st.session_state.get(key) is None:
        st.session_state[key] = cache.get(key, default)
    else:
        if reset and cache.get(key) != st.session_state[key]:
            for k in reset:
                st.session_state[k] = []

        cache[key] = st.session_state[key]


def _to_upper_values(key: str):
    """Convert values in a streamlit state to uppercase."""
    if key in st.session_state:
        st.session_state[key] = [
            s.upper() if isinstance(s, str) else s for s in to_list(st.session_state[key])
        ]
