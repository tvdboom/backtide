"""Backtide.

Author: Mavs
Description: Utility functions for the UI.

"""

from typing import Any

import streamlit as st

from backtide.core.data import AssetType
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
            s.upper() for s in to_list(st.session_state[key]) if isinstance(s, str)
        ]
