"""Backtide.

Author: Mavs
Description: Page to download new data.

"""

import math
from datetime import datetime

import streamlit as st

from backtide.core.config import get_config
from backtide.core.ingestion import list_assets
from backtide.core.models import AssetType, Interval
from backtide.ui.utils import (
    _get_asset_type_description,
    _prevent_deselection,
    _to_upper_values,
)
from backtide.utils.constants import MAX_ASSET_SELECTION, MAX_PRELOADED_ASSETS
from backtide.utils.utils import format_compact, to_list


config = get_config()

st.set_page_config(page_title="Backtide - Download")

st.title("Download", text_alignment="center")

st.text(
    """
    Perform bulk download of historical OHLC market data for multiple assets and/or intervals
    at once. FX rates for historical conversion rates are automatically downloaded if required.
    """
)

st.divider()
if not st.session_state.get("asset_type_download"):
    st.session_state.asset_type_download = AssetType.get_default()

with st.spinner("Loading assets..."):
    all_assets = list_assets(st.session_state.asset_type_download, MAX_PRELOADED_ASSETS)

asset_type = st.segmented_control(
    label="Asset type",
    key="asset_type_download",
    options=AssetType.variants(),
    format_func=lambda asset_type: f"{asset_type.icon()} {asset_type}",
    on_change=_prevent_deselection(
        key="asset_type_download",
        default=AssetType.get_default(),
        reset=["symbols_download", "currency_download"],
    ),
    help="Select the type of financial asset you want to backtest.",
)

# Filter assets based on the selected currency
if currency := st.session_state.get("currency_download"):
    filtered_assets = [
        asset
        for asset in all_assets
        if currency == "All"
        or asset.currency == currency
        or (asset_type in (AssetType.Forex, AssetType.Crypto) and currency in asset.name)
    ]
else:
    filtered_assets = all_assets

col1, col2 = st.columns([5, 1], vertical_alignment="bottom")
asset_d, currency_d = _get_asset_type_description(st.session_state.asset_type_download)


def format_func(asset: Asset) -> str:
    """User-friendly representation of an asset."""
    match asset_type:
        case AssetType.Stocks | AssetType.Etf:
            return f"{asset.symbol} - {asset.name}"
        case AssetType.Forex:
            return asset.name
        case _:
            return asset.symbol


assets = col1.multiselect(
    label="Symbols",
    key="assets_download",
    options=sorted(filtered_assets, key=lambda a: a.symbol),
    format_func=format_func,
    placeholder="Select one or more symbols...",
    max_selections=MAX_ASSET_SELECTION,
    accept_new_options=True,
    on_change=_to_upper_values("symbols"),
    help=asset_d,
)

col2.selectbox(
    label="Currency",
    key="currency_download",  # Use key to filter tickers
    options=["All", *sorted(dict.fromkeys(a.currency for a in all_assets))],
    placeholder="All",
    help=currency_d,
)

full_history = st.toggle(
    label="Download all history",
    value=True,
    help=(
        "Whether to download the maximum available history for all selected tickers. "
        "If toggled off, select the start and end download dates."
    ),
)

if not full_history:
    col1, col2 = st.columns(2)

    start_date = col1.date_input(
        label="Start date",
        value=None,
        min_value="1980-01-01",
        max_value=datetime.now().date(),
        format=config.display.date_format,
        help=(
            "Download data starting from this date (inclusive). If the historical "
            "data does not go so far back, it downloads the full available history."
        ),
    )

    end_date = col2.date_input(
        label="End date",
        value="today",
        min_value=start_date,
        max_value="today",
        format=config.display.date_format,
        help="Download data up to this date (exclusive).",
    )

intervals = st.pills(
    label="Interval",
    options=Interval.variants(),
    selection_mode="multi",
    default=Interval.get_default(),
    help=(
        "The frequency of the data points to download. Note that full history is "
        "only available for intervals >= 1d."
    ),
)

if assets and start_date and end_date and intervals:
    n_days = (end_date - start_date).days + 1
    n_years = int(n_days / 365.25)
    if n_years >= 1:
        ranges = f"{int(n_years)}y {math.ceil(n_days - n_years * 365.25)}d"
    else:
        ranges = f"{n_days}d"

    n_rows = len(assets) * sum(max(1, n_days * 24 * 60 / i.to_minutes()) for i in intervals)
    st.info(
        f"""
        Download overview:
        - Number of symbols: {len(assets)}
        - Range: {start_date} to {end_date} ({ranges})
        - Intervals: {",".join([x.value for x in to_list(intervals)])}
        - Approximate number of bars: {format_compact(n_rows)}
        """,
        icon=":material/info:",
    )

st.divider()

if st.button(
    label="Download",
    icon=":material/get_app:",
    type="primary",
    disabled=not (assets and start_date and end_date and intervals),
    width="stretch",
):
    if end_date > datetime.now().date():
        st.error("End date cannot be in the future.", icon=":material/error:")
    elif start_date > end_date:  # ty:ignore[unsupported-operator]
        st.error("Start date must be equal or prior to end date.", icon=":material/error:")
    else:
        with st.spinner("Downloading data..."):
            # TODO: implement download logic
            st.success(
                f"Successfully downloaded {len(symbols)} ticker(s) "
                f"from {start_date} to {end_date}.",
                icon=":material/check_circle:",
            )
