"""Backtide.

Author: Mavs
Description: Page to download new data.

"""

from datetime import datetime

import streamlit as st


# Asset type options with material icons
ASSET_TYPES = {
    "Stocks": ":material/candlestick_chart:",
    "Forex": ":material/currency_exchange:",
    "ETF": ":material/account_balance:",
    "Crypto": ":material/currency_bitcoin:",
}

# Ticker suggestions per asset type
TICKERS = {
    "Stocks": ["AAPL", "MSFT", "GOOGL", "AMZN", "TSLA", "NVDA", "META", "JPM", "V", "JNJ"],
    "Forex": ["EUR/USD", "GBP/USD", "USD/JPY", "AUD/USD", "USD/CHF", "USD/CAD", "NZD/USD"],
    "ETF": ["SPY", "QQQ", "IWM", "GLD", "TLT", "VTI", "VOO", "EFA", "AGG", "XLF"],
    "Crypto": ["BTC/USD", "ETH/USD", "SOL/USD", "BNB/USD", "XRP/USD", "ADA/USD", "DOGE/USD"],
}

INTERVALS = ["1m", "5m", "15m", "30m", "1h", "4h", "1d", "1wk", "1mo"]

st.set_page_config(page_title="Backtide - Download", layout="centered")

st.title("Download", text_alignment="center")

st.divider()

asset_type = st.segmented_control(
    label="Asset type",
    options=list(ASSET_TYPES.keys()),
    format_func=lambda x: f"{ASSET_TYPES[x]} {x}",
    default="Stocks",
    help="Select the type of financial asset you want to download data for.",
)

tickers = st.multiselect(
    label="Tickers",
    options=TICKERS.get(asset_type, []),
    placeholder="Select one or more tickers...",
    help=(
        "Select the tickers to download. The available options change "
        "based on the selected asset type. You can also type to search."
    ),
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
