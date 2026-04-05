"""Backtide.

Author: Mavs
Description: Page to download new data.

"""

from datetime import datetime
from zoneinfo import ZoneInfo

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import (
    AssetType,
    Currency,
    Exchange,
    Interval,
    list_assets,
    get_download_info,
)
from backtide.ui.utils import (
    _fmt_number,
    _get_asset_type_description,
    _get_logokit_url,
    _prevent_deselection,
    _to_upper_values,
)
from backtide.utils.constants import MAX_ASSET_SELECTION, MAX_PRELOADED_ASSETS


# ─────────────────────────────────────────────────────────────────────────────
# Helper functions
# ─────────────────────────────────────────────────────────────────────────────

def draw_asset_df(assets: list[Asset]) -> int:
    """Draw a Streamlit dataframe of a list of assets.

    The display includes asset metadata and images.

    Parameters
    ----------
    assets : list[Asset]
        Assets to display in the table.

    Returns
    -------
    int
        Number of rows expected to be downloaded for the equested assets.

    """
    data = []
    total_rows = 0
    for asset in assets:
        # Determine the download range per asset
        asset_start = datetime.fromtimestamp(asset.earliest_ts[Interval.OneDay], tz=tz)
        asset_end = datetime.fromtimestamp(asset.latest_ts[Interval.OneDay], tz=tz)

        if not is_intraday:
            asset_start = asset_start.date()
            asset_end = asset_end.date()

        if not full_history:
            asset_start = max(start_ts, asset_start)
            asset_end = min(end_ts, asset_end)

        # Add row to dataframe
        row = {"Symbol": asset.symbol, "Start": asset_start, "End": asset_end}

        if logokit_key and asset.asset_type.is_equity:
            row["Logo"] = _get_logokit_url(asset, logokit_key)

        if asset.asset_type.is_equity:
            row["Name"] = asset.name
            if isinstance(asset.exchange, Exchange):
                row["Country"] = get_flag(asset.exchange.country.alpha2)
            else:
                row["Country"] = ""
            row["Exchange"] = str(asset.exchange)
            row["Currency"] = str(asset.quote)
        elif asset.asset_type == AssetType.Forex or logokit_key:
            if isinstance(asset.base, Currency):
                row["Base"] = get_flag(asset.base.country.alpha2)
            else:
                row["Base"] = _get_logokit_url(asset, logokit_key)
            if isinstance(asset.quote, Currency):
                row["Quote"] = get_flag(asset.quote.country.alpha2)
            else:
                row["Quote"] = _get_logokit_url(asset, logokit_key, use_quote=True)

        data.append(row)

        # Calculate estimated number of rows for this asset per interval
        delta_minutes = max((asset_end - asset_start).total_seconds() / 60, 1)
        delta_days = (asset_end - asset_start).days

        for interval in intervals:
            if asset_type in (AssetType.Stocks, AssetType.Etf):
                # Stocks / ETFs: 8/5
                if interval.is_intraday():
                    effective_minutes = delta_minutes * (5 / 7) * (8 / 24)
                    total_rows += max(int(effective_minutes // interval.minutes()), 1)
                else:
                    trading_days = delta_days * (5 / 7)
                    total_rows += max(int(trading_days // (interval.minutes() / 1440)), 1)

            elif asset_type == AssetType.Forex:
                # Forex: 24/5
                if interval.is_intraday():
                    effective_minutes = delta_minutes * (5 / 7)
                    total_rows += max(int(effective_minutes // interval.minutes()), 1)
                else:
                    trading_days = delta_days * (5 / 7)
                    total_rows += max(int(trading_days // (interval.minutes() / 1440)), 1)

            else:
                # Crypto: 24/7
                total_rows += max(int(delta_minutes // interval.minutes()), 1)

    data = pd.DataFrame(data)

    ts_format = cfg.display.datetime_format() if is_intraday else cfg.display.date_format
    column_config = {
        "Symbol": st.column_config.TextColumn("Symbol", width="small"),
        "Start": st.column_config.DatetimeColumn("Start", format=ts_format),
        "End": st.column_config.DatetimeColumn("End", format=ts_format),
    }

    column_order = ["Symbol", "Start", "End"]

    if "Logo" in data.columns:
        data = data.set_index("Logo")
        column_config["Logo"] = st.column_config.ImageColumn("", width="small", pinned=True)

    if "Name" in data.columns:
        column_config["Name"] = st.column_config.TextColumn()  # No width = stretch
        column_order.insert(1, "Name")

    if "Country" in data.columns:
        column_config["Country"] = st.column_config.ImageColumn(width=60)
        column_order.insert(2, "Country")

    if "Exchange" in data.columns:
        column_config["Exchange"] = st.column_config.TextColumn(width="small")
        column_order.insert(3, "Exchange")

    if "Currency" in data.columns:
        column_config["Currency"] = st.column_config.TextColumn(width="small")
        column_order.insert(4, "Currency")

    if "Base" in data.columns:
        column_config["Base"] = st.column_config.ImageColumn(width=-50)
        column_order.insert(0, "Base")

    if "Quote" in data.columns:
        column_config["Quote"] = st.column_config.ImageColumn(width=-50)
        column_order.insert(2, "Quote")

    st.dataframe(
        data=data,
        height="stretch",
        hide_index=data.index.name is None,
        column_config=column_config,
        column_order=column_order,
    )

    return total_rows


# ─────────────────────────────────────────────────────────────────────────────
# Streamlit page
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
if cfg.display.timezone:
    tz = ZoneInfo(cfg.display.timezone)
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

symbols = col1.multiselect(
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

intervals = st.session_state.get("interval_download", [])

try:
    # Convert custom symbols to assets and add triangulation currencies
    download_info = get_download_info(symbols, asset_type, intervals)
    assets = download_info.assets
except RuntimeError as ex:
    assets = []
    st.error(ex, icon="❌")

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
        "Whether to download the maximum available history for all selected symbols and FX rates. "
        "If toggled off, select the start and end download dates."
    ),
)

if assets:
    earliest_ts = datetime.fromtimestamp(min(min(a.earliest_ts.values()) for a in assets), tz=tz)
    latest_ts = datetime.fromtimestamp(max(max(a.latest_ts.values()) for a in assets), tz=tz)
else:
    earliest_ts = datetime(2000, 1, 1, tzinfo=tz)
    latest_ts = datetime.now(tz=tz)

is_intraday = any(interval.is_intraday() for interval in intervals)

if full_history:
    start_ts = earliest_ts
    end_ts = latest_ts
else:
    col1, col2 = st.columns(2)

    if is_intraday:
        # Clamp datetime steps between 15min and 1h
        step = min(max(15, min(i.minutes() for i in intervals)), 60) * 60

        # Use datetime widgets when there are intraday intervals
        start_ts = col1.datetime_input(
            label="Start date",
            value=earliest_ts,
            min_value=earliest_ts,
            max_value=datetime.now(tz=tz),
            step=step,
            format=cfg.display.date_format,
            help="Download data starting from this timestamp (inclusive).",
        ).replace(tzinfo=tz)

        end_ts = col2.datetime_input(
            label="End date",
            value=latest_ts,
            min_value=start_ts,
            max_value=datetime.now(tz=tz),
            step=step,
            format=cfg.display.date_format,
            help="Download data up to this timestamp (exclusive).",
        ).replace(tzinfo=tz)
    else:
        # Use date widgets when there are no intraday intervals
        start_ts = col1.date_input(
            label="Start date",
            value=earliest_ts,
            min_value=earliest_ts,
            max_value="today",
            format=cfg.display.date_format,
            help="Download data starting from this date (inclusive).",
        )

        end_ts = col2.date_input(
            label="End date",
            value=latest_ts,
            min_value=start_ts,
            max_value="today",
            format=cfg.display.date_format,
            help="Download data up to this date (exclusive).",
        )

intervals = st.pills(
    label="Interval",
    key="interval_download",
    options=Interval.variants(),
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

    logokit_key = cfg.display.logokit_api_key
    get_flag = lambda code: f"https://flagcdn.com/80x60/{code.lower()}.png"

    with st.expander("Download overview", icon=":material/archive:", expanded=True):
        total_rows = 0

        total_rows += draw_asset_df(assets)
        if download_info.legs:
            total_rows += draw_asset_df(download_info.legs)

        estimated_memory = (total_rows * BYTES_PER_ROW) / (1024**2)
        estimated_seconds = int(total_rows / ROWS_PER_SECOND)

        hours, remainder = divmod(estimated_seconds, 3600)
        minutes, seconds = divmod(remainder, 60)

        if hours:
            time_str = f"{hours}h {minutes}m"
        elif minutes:
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

        col1, col2, col3 = st.columns(3)
        col1.metric(":material/table_rows: Estimated rows", _fmt_number(total_rows), border=True)
        col2.metric(":material/timer: Estimated time", time_str, border=True)
        col3.metric(":material/memory: Estimated memory", size_str, border=True)

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
