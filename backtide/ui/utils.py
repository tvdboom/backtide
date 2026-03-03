"""Backtide.

Author: Mavs
Description: Utility functions for the UI.

"""

from typing import Any

import streamlit as st


def _prevent_deselection(key: str, default: Any):
    """On-change function to call for widgets for which a valid must be selected."""
    if "_cache" not in st.session_state:
        st.session_state["_cache"] = {}
    cache = st.session_state["_cache"]

    if st.session_state.get(key) is None:
        st.session_state[key] = cache.get(key, default)
    else:
        cache[key] = st.session_state[key]
