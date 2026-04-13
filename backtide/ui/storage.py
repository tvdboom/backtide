"""Backtide.

Author: Mavs
Description: Overview of the stored data page.

"""

from datetime import datetime as dt
from zoneinfo import ZoneInfo

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import InstrumentType
from backtide.core.storage import delete_symbols, get_bars_summary
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
        delete_symbols(series=[(g["Symbol"], g["Interval"], g["Provider"]) for g in series])
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

st.divider()

bars_df = get_bars_summary()

if bars_df.empty:
    st.info(
        "There database is empty. Head over to the **Download** page to fetch some market data.",
        icon=":material/info:",
    )
    st.stop()

rows = [
    {
        "Symbol": r["symbol"],
        "Interval": r["interval"],
        "Instrument type": r["instrument_type"],
        "Provider": r["provider"],
        "First date": _parse_date(int(r["first_ts"]), cfg.display.date_format, tz),
        "Last date": _parse_date(int(r["last_ts"]), cfg.display.date_format, tz),
        "Bars": int(r["n_rows"]),
        "Price": r["sparkline"] if r["sparkline"] else None,
    }
    for _, r in bars_df.iterrows()
]

df = pd.DataFrame(rows)

metrics_container = st.container()

column_config = {
    "Bars": st.column_config.NumberColumn(format="%d"),
    "Price": st.column_config.LineChartColumn(help="Closing price for the last 365 intervals."),
}

if logokit_key := cfg.display.logokit_api_key:
    df.index = pd.Index(
        data=[
            _get_logokit_url(row["Symbol"], InstrumentType(row["Instrument type"]), logokit_key)
            for _, row in df.iterrows()
        ],
        name="Logo",
    )
    column_config["Logo"] = st.column_config.ImageColumn(label="", width="small")

event = st.dataframe(
    df.sort_values(["Symbol", "Interval"], ascending=True),
    height="stretch",
    column_config=column_config,
    hide_index=df.index.name is None,
    selection_mode="multi-row",
    on_select="rerun",
)

indices = event.selection.rows if event and event.selection else None
selected = df.iloc[indices] if indices else df

with metrics_container:
    col1, col2, col3 = st.columns(3)
    col1.metric(
        ":material/trending_up: Number of symbols",
        selected["Symbol"].nunique(),
        border=True,
    )
    col2.metric(":material/view_list: Number of series", _fmt_number(len(selected)), border=True)
    col3.metric(
        ":material/candlestick_chart: Total bars",
        _fmt_number(selected["Bars"].sum()),
        border=True,
    )

if indices:
    if st.button(f"Delete {len(indices)} series", type="primary", icon=":material/delete:"):
        _confirm_delete([rows[i] for i in indices])
