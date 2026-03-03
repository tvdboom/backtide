"""Backtide.

Author: Mavs
Description: Page to download new data.

"""

from datetime import datetime

import streamlit as st

from backtide.assets.assets import AssetType
from backtide.constants import MAX_ASSET_SELECTION
from backtide.models.ui import Interval
from backtide.ui.utils import _prevent_deselection
from backtide.utils.utils import format_compact


st.set_page_config(page_title="Backtide - Download")

st.title("Download", text_alignment="center")

st.divider()

asset_type = st.segmented_control(
    label="Asset type",
    options=AssetType,
    key="asset_type",
    format_func=lambda asset_type: f"{asset_type.icon()} {asset_type.value}",
    on_change=_prevent_deselection("asset_type", AssetType.STOCKS),
    help="Select the type of financial asset you want to download data for.",
)

with st.spinner("Fetching symbols..."):
    all_assets = st.session_state.asset_type.list_symbols()

# Filter assets based on the selected currency
if currency := st.session_state.get("currency"):
    assets = [a for a in all_assets if currency == "All" or a.currency == currency]
else:
    assets = all_assets

match st.session_state.asset_type:
    case AssetType.STOCKS:
        symbol_description = (
            "List of yahoo stock tickers to download. The preloaded options are the primary "
            "listings for companies in major indices, but any valid yahoo stock ticker can be "
            "added."
        )
        curr_description = "Filter tickers by their denominated currency."
    case AssetType.FOREX:
        symbol_description = (
            "List of currency pairs to download. The preloaded options are frequently traded "
            "pairs, but any valid yahoo forex ticker can be added."
        )
        curr_description = "Filter pairs by their quote currency."
    case AssetType.ETF:
        symbol_description = (
            "List of yahoo ETF tickers to download. The preloaded options are frequently traded "
            "ETFs and funds, but any valid yahoo ETF ticker can be added."
        )
        curr_description = "Filter tickers by their denominated currency."
    case AssetType.CRYPTO:
        symbol_description = (
            "List of currency pairs to download. The preloaded options are frequently traded "
            "pairs, but any valid yahoo ticker can be added."
        )
        curr_description = "Filter symbols by their quote currency."

ticker_col, filter_col = st.columns([3, 1], vertical_alignment="bottom")

tickers = ticker_col.multiselect(
    label="Symbols",
    options=sorted(assets, key=lambda x: x.symbol),
    format_func=lambda x: f"{x.symbol} - {x.name}" if asset_type == AssetType.STOCKS else x.name,
    placeholder="Select one or more symbols...",
    max_selections=MAX_ASSET_SELECTION,
    accept_new_options=True,
    help=symbol_description,
)

filter_col.selectbox(
    label="Currency",
    key="currency",  # Use key to filter tickers
    options=["All", *sorted(dict.fromkeys(asset.currency for asset in all_assets))],
    placeholder="All",
    help=curr_description,
)

start_date_col, end_date_col = st.columns(2)

start_date = start_date_col.date_input(
    label="Start date",
    value=None,
    min_value="1980-01-01",
    max_value=datetime.now().date(),
    help=(
        "Download data starting from this date (inclusive). If the historical "
        "data does not go so far back, it downloads the full available history."
    ),
)

end_date = end_date_col.date_input(
    label="End date",
    value="today",
    min_value=start_date,
    max_value="today",
    help="Download data up to this date (inclusive).",
)

intervals = st.pills(
    label="Interval",
    options=Interval,
    format_func=lambda x: x.value,
    selection_mode="multi",
    default=Interval.OneHour,
    help=(
        "The frequency of the data points to download. Note that full history is "
        "only available for intervals >= 1d."
    ),
)

if tickers and start_date and end_date and intervals:
    n_days = (end_date - start_date).days + 1
    n_points = len(tickers) * sum(max(1, n_days * 24 * 60 / i.to_minutes()) for i in intervals)
    st.info(f"Download {format_compact(n_points)} data entries.", icon=":material/info:")

st.divider()

if st.button(
    label="Download",
    icon=":material/cloud_download:",
    type="primary",
    disabled=not (tickers and start_date and end_date),
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
                f"Successfully downloaded {len(tickers)} ticker(s) "
                f"from {start_date} to {end_date}.",
                icon=":material/check_circle:",
            )
