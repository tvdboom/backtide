"""Backtide.

Author: Mavs
Description: Page to download new data.

"""

from datetime import datetime as dt
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
    _moment_to_strftime,
    _get_asset_type_description,
    _get_logokit_url,
    _prevent_deselection,
    _to_upper_values,
)
from backtide.utils.constants import MAX_ASSET_SELECTION, MAX_PRELOADED_ASSETS


# ─────────────────────────────────────────────────────────────────────────────
# Helper functions
# ─────────────────────────────────────────────────────────────────────────────
CARD_CSS = """
<style>
  .section {
    font-size: 12px;
    font-weight: 600;
    color: #888;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    margin: 18px 0 8px;
  }

 .card {
      border: 1px solid rgba(0,0,0, 0.2);
      border-radius: 12px;
      padding: 1.2rem 1.4rem;
      margin-bottom: 10px;
    }

  .card-header {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 12px;
  }

  .flag {
    height: 22px;
    border-radius: 3px;
  }

  .title {
    display: flex;
    flex-direction: column;
  }

  .symbol {
    font-size: 22px;
    font-weight: 700;
  }

  .name {
    font-size: 18px;
    opacity: 0.7;
  }

  .meta-right {
    margin-left: auto;
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .badge {
    font-size: 16px;
    padding: 3px 8px;
    border-radius: 6px;
    background: rgba(250,250,250,0.07);
    border: 1px solid rgba(250,250,250,0.1);
    white-space: nowrap;
  }

  .badge.leg {
    background: rgba(99,179,237,0.12);
    color: #63b3ed;
  }

  .intervals {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border-top: 1px solid rgba(250,250,250,0.08);
    padding-top: 10px;
  }

  .interval-row {
    display: grid;
    grid-template-columns: 40px 200px auto;
    gap: 12px;
    font-size: 13px;
  }

  .iv-label {
    font-weight: 600;
    font-size: 18px;
    opacity: 0.7;
  }

  .iv-range {
    font-size: 18px;
  }

  .iv-rows {
    font-size: 18px;
    opacity: 0.6;
  }

  .legs-row {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid rgba(250,250,250,0.08);
  }
</style>
"""

def render_download_cards(assets) -> int:
    html = f'<div class="section"></div>'

    total_rows = 0
    for asset in assets:
        interval_rows = ""
        for interval in Interval.variants():
            start_iv = asset.earliest_ts.get(interval)
            end_iv = asset.latest_ts.get(interval)
            if not (start_iv and end_iv):
                continue

            iv_start = dt.fromtimestamp(start_iv, tz=tz).date()
            iv_end = dt.fromtimestamp(end_iv, tz=tz).date()
            if not full_history:
                iv_start = max(start_ts, iv_start)
                iv_end = min(end_ts, iv_end)

            # Estimate rows for this interval
            delta_minutes = max((iv_end - iv_start).total_seconds() / 60, 1)
            delta_days = (iv_end - iv_start).days

            if asset.asset_type.is_equity:
                # Stocks / ETFs: 8/5
                if interval.is_intraday():
                    rows = max(int(delta_minutes * (5 / 7) * (8 / 24) // interval.minutes()), 1)
                else:
                    rows = max(int(delta_days * (5 / 7) // (interval.minutes() / 1440)), 1)
            elif asset_type == AssetType.Forex:
                # Forex: 24/5
                if interval.is_intraday():
                    rows = max(int(delta_minutes * (5 / 7) // interval.minutes()), 1)
                else:
                    rows = max(int(delta_days * (5 / 7) // (interval.minutes() / 1440)), 1)
            else:
                # Crypto: 24/7
                rows = max(int(delta_minutes // interval.minutes()), 1)

            total_rows += rows

            parse_date = lambda x: x.strftime(_moment_to_strftime(cfg.display.date_format))

            interval_rows += f"""
                <div class="interval-row">
                    <span class="iv-label">{interval}</span>
                    <span class="iv-range">{parse_date(iv_start)} → {parse_date(iv_end)}</span>
                    <span class="iv-rows">~{_fmt_number(rows)} rows</span>
                </div>"""

        # Header badges
        exchange = str(asset.exchange)
        quote = str(asset.quote)

        if logokit_key:
            logo = _get_logokit_url(asset, logokit_key)
        else:
            logo = ""

        flag = f'<img src="{logo}" style="height:64px;border-radius:6px;">'
        name = asset.name if asset.asset_type.is_equity else ""

        legs = ""
        if asset.legs:
            badges = "".join(f'<span class="badge leg">{leg}</span>' for leg in asset.legs)
            legs = f'<div class="legs-row"><span style="font-size:16px">via</span>{badges}</div>'

        html += f"""
            <div class="card">
              <div class="card-header">
                {flag}
                <div><div class="symbol">{asset.symbol}</div><div class="name">{name}</div></div>
                <span class="badge" style="margin-left:auto;">{exchange}</span>
                <span class="badge">{quote}</span>
              </div>
              <div class="intervals">{interval_rows}</div>
              {legs}
            </div>"""

    return html, total_rows


def draw_asset_df(assets: list[AssetMeta]) -> int:
    """Draw a Streamlit dataframe of a list of assets.

    The display includes asset metadata and images.

    Parameters
    ----------
    assets : list[AssetMeta]
        Assets to display in the table.

    Returns
    -------
    int
        Number of rows expected to be downloaded for the equested assets.

    """
    data = []
    total_rows = 0
    for asset in assets:
        # Add row to dataframe
        row = {"Symbol": asset.symbol}

        range_lines = []
        for interval in Interval.variants():
            start_iv = asset.earliest_ts.get(interval)
            end_iv = asset.latest_ts.get(interval)
            if not (start_iv and end_iv):
                continue

            iv_start = dt.fromtimestamp(start_iv, tz=tz).date()
            iv_end = dt.fromtimestamp(end_iv, tz=tz).date()

            if not full_history:
                iv_start = max(start_ts, iv_start)
                iv_end = min(end_ts, iv_end)

            # Estimate rows for this interval
            delta_minutes = max((iv_end - iv_start).total_seconds() / 60, 1)
            delta_days = (iv_end - iv_start).days

            if asset_type in (AssetType.Stocks, AssetType.Etf):
                # Stocks / ETFs: 8/5
                if interval.is_intraday():
                    rows = max(int(delta_minutes * (5 / 7) * (8 / 24) // interval.minutes()), 1)
                else:
                    rows = max(int(delta_days * (5 / 7) // (interval.minutes() / 1440)), 1)
            elif asset_type == AssetType.Forex:
                # Forex: 24/5
                if interval.is_intraday():
                    rows = max(int(delta_minutes * (5 / 7) // interval.minutes()), 1)
                else:
                    rows = max(int(delta_days * (5 / 7) // (interval.minutes() / 1440)), 1)
            else:
                # Crypto: 24/7
                rows = max(int(delta_minutes // interval.minutes()), 1)

            total_rows += rows

            date_str = f"{iv_start} → {iv_end}"
            rows_str = f"~{_fmt_number(rows)} rows"

            if len(intervals) == 1:
                line = f"{date_str}  ({rows_str})"
            else:
                line = f"{interval}  {date_str}  ({rows_str})"

            range_lines.append(line)

        row["Range"] = "\n".join(range_lines)

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

    data = pd.DataFrame(data)

    column_config = {
        "Symbol": st.column_config.TextColumn(width="small"),
        "Range": st.column_config.TextColumn(),
    }

    column_order = ["Symbol", "Range"]

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
    tz = dt.now().astimezone().tzinfo

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


@st.cache_data(ttl=3600, show_spinner="Loading assets...")
def list_symbols(asset_type: AssetType):
    """Cache the major symbols per asset type."""
    return list_assets(asset_type, MAX_PRELOADED_ASSETS)


all_assets = list_symbols(st.session_state.asset_type_download)

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
    st.error(ex, icon=":material/error:")

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

if assets and intervals:
    earliest_ts = dt.fromtimestamp(min(min(a.earliest_ts.values()) for a in assets), tz=tz).date()
    latest_ts = dt.fromtimestamp(max(max(a.latest_ts.values()) for a in assets), tz=tz).date()
else:
    earliest_ts = dt(2000, 1, 1, tzinfo=tz).date()
    latest_ts = dt.now(tz=tz).date()

if full_history:
    start_ts = earliest_ts
    end_ts = latest_ts
else:
    col1, col2 = st.columns(2)

    # Use date widgets when there are no intraday intervals
    start_ts = col1.date_input(
        label="Start date",
        value=earliest_ts,
        min_value=earliest_ts,
        max_value="today",
        format=cfg.display.date_format,
        help=(
            "Download data starting from this date (inclusive). A download can start later "
            "if the provider doesn't have the data this far back, but it can't start earlier.",
        ),
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

        # total_rows += draw_asset_df(assets)
        # if download_info.legs:
        #     total_rows += draw_asset_df(download_info.legs)
        css = f"<style>{CARD_CSS}</style>"
        html, rows = render_download_cards(assets)

        total_rows += rows

        # if download_info.legs:
        #     html += render_download_cards(download_info.legs, intervals, "Conversion legs")

        st.markdown(css + html, unsafe_allow_html=True)

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
    if latest_ts > dt.now(tz=tz).date():
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
