"""Backtide.

Author: Mavs
Description: Page to download new data.

"""

from datetime import datetime

import streamlit as st

from backtide.assets import AssetType
from backtide.constants import MAX_ASSET_SELECTION


INTERVALS = ["1m", "5m", "15m", "30m", "1h", "4h", "1d", "1wk", "1mo"]

st.set_page_config(page_title="Backtide - Download")

st.title("Download", text_alignment="center")

st.divider()

asset_type = st.segmented_control(
    label="Asset type",
    options=AssetType,
    format_func=lambda asset_type: f"{asset_type.icon()} {asset_type.value}",
    default=AssetType.STOCKS,
    help="Select the type of financial asset you want to download data for.",
)

ticker_col, filter_col = st.columns([4, 1], vertical_alignment="bottom")

assets = asset_type.list_preloaded()
if st.session_state.get("currency"):
    assets = [a for a in assets if a.currency == "EUR"]

tickers = ticker_col.multiselect(
    label=f"{asset_type.identifier().capitalize()}s",
    options=sorted(assets, key=lambda x: x.symbol),
    format_func=lambda asset: f"{asset.symbol} - {asset.name}",
    placeholder=f"Select one or more {asset_type.identifier()}s...",
    max_selections=MAX_ASSET_SELECTION,
    help=f"Select the {asset_type.identifier()}(s) to download.",
)

with filter_col:
    st.selectbox(
        label="Currency",
        key="currency",
        options=[None] + [c for c in ["EUR", "USD"]],
        format_func=lambda c: "All" if c is None else c,
        help=f"Filter {asset_type.identifier()}s by their denominated  currency.",
    )

start_date_col, end_date_col = st.columns(2)

start_date = start_date_col.date_input(
    label="Start date",
    value=None,
    help="The start date of the data to download (inclusive).",
)

end_date = end_date_col.date_input(
    label="End date",
    value=datetime.now().date(),
    help="The end date of the data to download (inclusive).",
)

interval = st.segmented_control(
    label="Interval",
    options=INTERVALS,
    default="1d",
    help=(
        "The frequency of the data points. Note that very short intervals "
        "(e.g. 1m, 5m) are only available for recent dates and may be "
        "limited by the data provider."
    ),
)

st.divider()

if st.button(
    label="Download",
    icon=":material/cloud_download:",
    type="primary",
    disabled=not (asset_type and tickers and start_date and end_date),
    width="stretch",
):
    if end_date > datetime.now().date():
        st.error("End date cannot be in the future.", icon=":material/error:")
    elif start_date >= end_date:
        st.error("Start date must be before end date.", icon=":material/error:")
    else:
        with st.spinner("Downloading data..."):
            # TODO: implement download logic
            st.success(
                f"Successfully downloaded {len(tickers)} ticker(s) "
                f"from {start_date} to {end_date} at {interval} interval.",
                icon=":material/check_circle:",
            )
