"""Backtide.

Author: Mavs
Description: Overview of the stored data page.

"""

from zoneinfo import ZoneInfo

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import InstrumentType
from backtide.core.storage import delete_symbols
from backtide.ui.utils import (
    _fmt_number,
    _get_logokit_url,
    _get_timezone,
    _parse_date,
    _query_bars_summary,
)

# ─────────────────────────────────────────────────────────────────────────────
# Helper functionalities
# ─────────────────────────────────────────────────────────────────────────────


@st.cache_data(show_spinner="Loading bars from database...")
def _load_storage_df(date_fmt: str, tz: ZoneInfo, logokit_key: str | None) -> pd.DataFrame:
    """Load and cache the stored data from the database."""
    raw = _query_bars_summary()
    df = pd.DataFrame(
        {
            "Symbol": raw["symbol"],
            "Name": raw["name"],
            "Interval": raw["interval"],
            "Instrument type": raw["instrument_type"],
            "Provider": raw["provider"],
            "First date": raw["first_ts"].astype(int).map(lambda x: _parse_date(x, date_fmt, tz)),
            "Last date": raw["last_ts"].astype(int).map(lambda x: _parse_date(x, date_fmt, tz)),
            "Bars": raw["n_rows"].astype(int),
            "Price": raw["sparkline"].tolist(),  # pyarrow to list so streamlit can serialize
        },
    )

    df = df.sort_values(["Symbol", "Interval"], ascending=True).reset_index(drop=True)

    if logokit_key:
        df.index = pd.Index(
            data=df.apply(
                lambda row: _get_logokit_url(
                    row["Symbol"], InstrumentType(row["Instrument type"]), logokit_key
                ),
                axis=1,
            ),
            name="Logo",
        )

    return df


@st.dialog("Confirm deletion", width="medium")
def _confirm_delete(series: list[pd.Series]):
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
        st.cache_data.clear()
        st.rerun()


# ─────────────────────────────────────────────────────────────────────────────
# Storage interface
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
tz = _get_timezone(cfg.display.timezone)
logokit_key = cfg.display.logokit_api_key

st.set_page_config(page_title="Backtide - Storage")

st.title("Storage", text_alignment="center")

st.divider()


bars_df = _load_storage_df(cfg.display.date_format, tz, logokit_key)

if bars_df.empty:
    st.info(
        "There database is empty. Head over to the **Download** page to fetch some market data.",
        icon=":material/info:",
    )
    st.stop()

metrics_container = st.container()

column_config = {
    "Name": st.column_config.TextColumn(width="medium"),
    "Instrument type": st.column_config.TextColumn(width="small"),
    "Bars": st.column_config.NumberColumn(format="%d"),
    "Price": st.column_config.LineChartColumn(help="Closing price for the last 365 intervals."),
}

if logokit_key:
    column_config["Logo"] = st.column_config.ImageColumn(label="", width="small")

event = st.dataframe(
    bars_df,
    height="stretch",
    column_config=column_config,
    hide_index=bars_df.index.name is None,
    selection_mode="multi-row",
    on_select="rerun",
)

indices = event.selection.rows if event and event.selection else None  # ty: ignore[unresolved-attribute]
selected = bars_df.iloc[indices] if indices else bars_df

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
        _confirm_delete([bars_df.iloc[i] for i in indices])
