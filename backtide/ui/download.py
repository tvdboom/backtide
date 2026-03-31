"""Backtide.

Author: Mavs
Description: Page to download new data.

"""

from datetime import datetime, timedelta
from zoneinfo import ZoneInfo

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import AssetType, Interval, get_assets, list_assets, list_intervals
from backtide.ui.utils import (
    _format_number,
    _get_asset_type_description,
    _get_logokit_url,
    _prevent_deselection,
    _to_upper_values,
)
from backtide.utils.constants import MAX_ASSET_SELECTION, MAX_PRELOADED_ASSETS

config = get_config()
if config.display.timezone:
    tz = ZoneInfo(config.display.timezone)
else:
    tz = datetime.now().astimezone().tzinfo

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

asset_type = st.segmented_control(
    label="Asset type",
    key="asset_type_download",
    options=AssetType.variants(),
    format_func=lambda asset_type: f"{asset_type.icon()} {asset_type}",
    on_change=_prevent_deselection(
        key="asset_type_download",
        default=AssetType.get_default(),
        reset=["assets_download", "currency_download"],
    ),
    help="Select the type of financial asset you want to backtest.",
)

if not st.session_state.get(f"all_assets_{asset_type}"):
    with st.spinner("Loading assets..."):
        st.session_state[f"all_assets_{asset_type}"] = list_assets(
            st.session_state.asset_type_download, MAX_PRELOADED_ASSETS
        )

all_assets = st.session_state[f"all_assets_{asset_type}"]

# Filter assets based on the selected currency
if currency := st.session_state.get("currency_download"):
    filtered_assets = [
        asset
        for asset in all_assets
        if currency == "All" or asset.base == currency or str(asset.quote) == currency
    ]
else:
    filtered_assets = all_assets

col1, col2 = st.columns([5, 1], vertical_alignment="bottom")
asset_d, currency_d = _get_asset_type_description(st.session_state.asset_type_download)

assets = col1.multiselect(
    label="Symbols",
    key="assets_download",
    options=sorted(filtered_assets, key=lambda a: a.symbol),
    format_func=lambda a: (
        f"{a.symbol} - {a.name}" if a.asset_type in (AssetType.Stocks, AssetType.Etf) else a.symbol
    ),
    placeholder="Select one or more symbols...",
    max_selections=MAX_ASSET_SELECTION,
    accept_new_options=True,
    on_change=_to_upper_values("assets_download"),
    help=asset_d,
)

# Convert custom symbols to assets and retrieve all metadata for predefined assets
assets = get_assets([getattr(a, "symbol", a) for a in assets], asset_type)

col2.selectbox(
    label="Currency",
    key="currency_download",  # Use key to filter tickers
    options=["All", *sorted(dict.fromkeys(str(a.quote) for a in all_assets))],
    placeholder="All",
    help=currency_d,
)

full_history = st.toggle(
    label="Download full history",
    value=True,
    help=(
        "Whether to download the maximum available history for all selected tickers. "
        "If toggled off, select the start and end download dates."
    ),
)

if assets:
    earliest_ts = datetime.fromtimestamp(min(asset.earliest_ts for asset in assets), tz=tz)
    latest_ts = datetime.fromtimestamp(max(asset.latest_ts for asset in assets), tz=tz)
else:
    earliest_ts = datetime(1970, 1, 1, tzinfo=tz)
    latest_ts = datetime.now(tz=tz)

intervals = st.session_state.get("interval_download", [])
is_intraday = any(interval.is_intraday() for interval in intervals)

if full_history:
    start_ts = earliest_ts
    end_ts = latest_ts
else:
    col1, col2 = st.columns(2)

    if is_intraday:
        step = max(30, *[i.minutes() for i in intervals])

        # Use datetime widgets when there are intraday intervals
        start_ts = col1.datetime_input(
            label="Start date",
            value=earliest_ts,
            min_value=earliest_ts,
            max_value=datetime.now(tz=tz),
            step=timedelta(minutes=step),
            format=config.display.date_format,
            help="Download data starting from this timestamp (inclusive).",
        )

        latest_ts = col2.datetime_input(
            label="End date",
            value=latest_ts,
            min_value=start_ts,
            max_value=datetime.now(tz=tz),
            step=timedelta(minutes=step),
            format=config.display.date_format,
            help="Download data up to this timestamp (exclusive).",
        )
    else:
        # Use date widgets when there are no intraday intervals
        start_ts = col1.date_input(
            label="Start date",
            value=earliest_ts,
            min_value=earliest_ts,
            max_value="today",
            format=config.display.date_format,
            help="Download data starting from this date (inclusive).",
        )

        latest_ts = col2.date_input(
            label="End date",
            value=latest_ts,
            min_value=start_ts,
            max_value="today",
            format=config.display.date_format,
            help="Download data up to this date (exclusive).",
        )

intervals = st.pills(
    label="Interval",
    key="interval_download",
    options=list_intervals(asset_type),
    selection_mode="multi",
    default=Interval.get_default(),
    help=(
        "The frequency of the data points to download. Note that full history is "
        "only available for intervals >= 1d."
    ),
)

is_enabled = assets and start_ts and latest_ts and intervals

if is_enabled:
    BYTES_PER_ROW = 150  # Estimated memory required per OHLC bar
    ROWS_PER_SECOND = 40_000  # Estimated number of rows downloaded per second

    logokit_key = config.display.logokit_api_key

    data = []
    total_rows = 0
    with st.expander("Download overview"):
        for asset in assets:
            # Determine the download range per asset
            asset_start = datetime.fromtimestamp(asset.earliest_ts, tz=tz)
            asset_end = datetime.fromtimestamp(asset.latest_ts, tz=tz)

            if not full_history:
                asset_start = max(start_ts, asset_start)
                asset_end = min(end_ts, asset_end)

            # Add row to dataframe
            row = {"Symbol": asset.symbol, "Start": asset_start, "End": asset_end}

            if logokit_key:
                row["Logo"] = _get_logokit_url(asset, logokit_key)

            if asset.asset_type != AssetType.Forex:
                row["Name"] = asset.name

            if asset.asset_type in (AssetType.Stocks, AssetType.Etf):
                if country_code := getattr(asset.quote, "country_code", None):
                    row["Country"] = f"https://flagcdn.com/h80/{country_code}.png"
                else:
                    row["Country"] = ""
                row["Exchange"] = asset.exchange
                row["Currency"] = str(asset.quote)

            data.append(row)

            # Calculate metrics
            delta_minutes = max((asset_end - asset_start).total_seconds() / 60, 1)
            for interval in intervals:
                total_rows += max(int(delta_minutes // interval.minutes()), 1)

        data = pd.DataFrame(data)

        ts_format = config.display.datetime_format() if is_intraday else config.display.date_format
        column_config = {
            "Symbol": st.column_config.TextColumn("Symbol", width="small"),
            "Start": st.column_config.DatetimeColumn("Start", format=ts_format),
            "End": st.column_config.DatetimeColumn("End", format=ts_format),
        }

        column_order = ["Symbol", "Start", "End"]

        if logokit_key:
            data = data.set_index("Logo")
            column_config["Logo"] = st.column_config.ImageColumn("", width="small")

        if "Name" in data.columns:
            column_config["Name"] = st.column_config.TextColumn("Name")  # No width = stretch
            column_order.insert(1, "Name")

        if "Country" in data.columns:
            column_config["Country"] = st.column_config.ImageColumn("Country", width="small")
            column_order.insert(2, "Country")

        if "Exchange" in data.columns:
            column_config["Exchange"] = st.column_config.TextColumn("Exchange", width="small")
            column_order.insert(3, "Exchange")

        if "Currency" in data.columns:
            column_config["Currency"] = st.column_config.TextColumn("Currency", width="small")
            column_order.insert(4, "Currency")

        st.dataframe(
            data=data,
            hide_index=logokit_key is None,
            column_config=column_config,
            column_order=column_order,
        )

        estimated_memory = (total_rows * BYTES_PER_ROW) / (1024**2)
        estimated_seconds = total_rows / ROWS_PER_SECOND
        minutes, seconds = divmod(int(estimated_seconds), 60)

        if minutes:
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

        _, col1, col2, col3 = st.columns([1, 2, 2, 3], gap="large")
        col1.metric("Est. Rows", _format_number(total_rows))
        col2.metric("Est. Time", time_str)
        col3.metric("Est. Memory", size_str)

st.divider()

if st.button(
    label="Download",
    icon=":material/get_app:",
    type="primary",
    disabled=not is_enabled,
    width="stretch",
):
    if latest_ts > datetime.now(tz=tz):
        st.error("End date cannot be in the future.", icon=":material/error:")
    elif start_ts > latest_ts:  # ty:ignore[unsupported-operator]
        st.error("Start date must be equal or prior to end date.", icon=":material/error:")
    else:
        with st.spinner("Downloading data..."):
            # TODO: implement download logic
            st.success(
                f"Successfully downloaded {len(assets)} tickers.",
                icon=":material/check_circle:",
            )
