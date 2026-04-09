"""Backtide.

Author: Mavs
Description: Overview of the stored data page.

"""

from datetime import datetime as dt
from zoneinfo import ZoneInfo

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import AssetType
from backtide.core.storage import delete_rows, get_summary
from backtide.ui.utils import _fmt_number, _get_logokit_url, _parse_date


# ─────────────────────────────────────────────────────────────────────────────
# Helper functionalities
# ─────────────────────────────────────────────────────────────────────────────

@st.dialog("Confirm deletion", width="medium")
def _confirm_delete(series: list[dict[str, str]]):
    """Show a modal asking the user to confirm deletion of selected series."""
    text = "\n".join([f"* {g['Symbol']}  -  {g['Interval']}" for g in series])
    st.warning(
        f"You are about to **permanently delete** the following series:\n\n{text}",
        icon=":material/warning:",
    )

    col1, col2 = st.columns(2)

    if col1.button("Cancel", width="stretch"):
        st.rerun()

    if col2.button("Delete", width="stretch", type="primary", icon=":material/delete:"):
        for g in series:
            delete_rows(g["Symbol"], interval=g["Interval"], provider=g["Provider"])
        st.rerun()


# ─────────────────────────────────────────────────────────────────────────────
# Storage interface
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
if cfg.display.timezone:
    tz = ZoneInfo(cfg.display.timezone)
else:
    tz = dt.now().astimezone().tzinfo

st.set_page_config(page_title="Backtide - Storage")

st.title("Storage", text_alignment="center")

st.text(
    "Overview of all OHLCV data stored in the local database. Each row represents one (symbol "
    "- interval) data series. Select one or more rows to delete the series from the database.",
)

st.divider()


summaries = get_summary()

if not summaries:
    st.info(
        "There database is empty. Head over to the **Download** page to fetch some market data.",
        icon=":material/info:",
    )
    st.stop()


rows = []
for s in summaries:
    rows.append(
        {
            "Symbol": s.symbol,
            "Interval": s.interval,
            "Asset type": s.asset_type,
            "Provider": s.provider,
            "First date": _parse_date(s.first_ts, cfg.display.date_format, tz),
            "Last date": _parse_date(s.last_ts, cfg.display.date_format, tz),
            "Bars": s.n_rows,
            "Price": s.sparkline if s.sparkline else None,
        },
    )

df = pd.DataFrame(rows)

col1, col2, col3 = st.columns(3)
col1.metric(":material/trending_up: Number of symbols", df["Symbol"].nunique(), border=True)
col2.metric(":material/view_list: Number of series", _fmt_number(len(df)), border=True)
col3.metric(":material/candlestick_chart: Total bars", _fmt_number(df["Bars"].sum()), border=True)

column_config = {
    "Bars": st.column_config.NumberColumn(format="%d"),
    "Price": st.column_config.LineChartColumn(help="Closing price for the last 365 intervals."),
}

if logokit_key := cfg.display.logokit_api_key:
    df.index = pd.Index(
        data=[_get_logokit_url(s.symbol, AssetType(s.asset_type), logokit_key) for s in summaries],
        name="Logo",
    )
    column_config["Logo"] = st.column_config.ImageColumn(label="", width="small")

event = st.dataframe(
    df.drop("Provider", axis=1).sort_values(["Symbol", "Interval"], ascending=True),
    height="stretch",
    column_config=column_config,
    hide_index=df.index.name is None,
    selection_mode="multi-row",
    on_select="rerun",
)

if indices := event.selection.rows if event and event.selection else None:
    if st.button(f"Delete {len(indices)} series", type="primary", icon=":material/delete:"):
        _confirm_delete([rows[i] for i in indices])
