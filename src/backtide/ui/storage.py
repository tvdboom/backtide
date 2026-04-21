"""Backtide.

Author: Mavs
Description: Overview of the stored data page.

"""

from zoneinfo import ZoneInfo

import pandas as pd
import streamlit as st

from backtide.core.config import get_config
from backtide.core.data import InstrumentType, Interval
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
            "Name": raw["name"].str.replace(r"\s+", " ", regex=True),
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


def _open_analysis(df: pd.DataFrame):
    """Navigate to the analysis page with pre-selected symbols and interval."""
    st.session_state["_symbols"] = df["Symbol"].unique().tolist()
    st.session_state["_interval"] = Interval(df["Interval"].mode()[0])
    st.switch_page("analysis.py")


@st.dialog("Confirm deletion", width="medium")
def _confirm_delete(df: pd.DataFrame):
    """Show a modal asking the user to confirm deletion of selected series."""
    st.warning(
        "You are about to **permanently delete** the following series.",
        icon=":material/warning:",
    )

    with st.container(height=200):
        st.markdown("\n".join([f"* {r['Symbol']}  -  {r['Interval']}" for _, r in df.iterrows()]))

    col1, col2 = st.columns(2)

    if col1.button("Cancel", width="stretch"):
        st.rerun()

    if col2.button("Delete", width="stretch", type="primary", icon=":material/delete:"):
        delete_symbols(
            series=[(r["Symbol"], r["Interval"], r["Provider"]) for _, r in df.iterrows()]
        )
        st.cache_data.clear()
        st.rerun()


# ─────────────────────────────────────────────────────────────────────────────
# Storage interface
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
tz = _get_timezone(cfg.display.timezone)
logokit_key = cfg.display.logokit_api_key

st.set_page_config(page_title="Backtide - Storage")

st.subheader("Storage", text_alignment="center")
st.write("")


all_series = _load_storage_df(cfg.display.date_format, tz, logokit_key)

if all_series.empty:
    st.info(
        "The database is empty. Head over to the **Download** page to fetch some market data.",
        icon=":material/info:",
    )
    st.stop()

metrics_container = st.container()

column_config = {
    "Symbol": st.column_config.TextColumn(pinned=True),
    "Name": st.column_config.TextColumn(width="medium"),
    "Bars": st.column_config.NumberColumn(format="%d"),
    "Price": st.column_config.LineChartColumn(help="Closing price for the last 365 intervals."),
}

if logokit_key:
    column_config["Logo"] = st.column_config.ImageColumn(label="", width="small")

columns = all_series.columns.drop(["Instrument type", "Provider"]).tolist()
if not all_series["Instrument type"].isin(["stocks", "etf"]).any():
    columns.remove("Name")

event = st.dataframe(
    all_series,
    height="stretch",
    column_config=column_config,
    column_order=columns,
    hide_index=all_series.index.name is None,
    selection_mode="multi-row",
    on_select="rerun",
)

indices = event.selection.rows if event and event.selection else None  # ty: ignore[unresolved-attribute]
selected_rows = all_series.iloc[indices] if indices else all_series

with metrics_container:
    col1, col2, col3 = st.columns(3)
    col1.metric(
        label=":material/numbers: Number of symbols",
        value=selected_rows["Symbol"].nunique(),
        border=True,
    )
    col2.metric(
        label=":material/view_list: Number of series",
        value=_fmt_number(len(selected_rows)),
        border=True,
    )
    col3.metric(
        label=":material/candlestick_chart: Total bars",
        value=_fmt_number(selected_rows["Bars"].sum()),
        border=True,
    )

if indices:
    col1, col2, _ = st.columns([2, 2, 3.9])

    if col1.button(
        label=f"Analyze {len(indices)} series",
        icon=":material/assessment:",
        type="secondary",
    ):
        _open_analysis(selected_rows)

    if col2.button(f"Delete {len(indices)} series", icon=":material/delete:", type="primary"):
        _confirm_delete(selected_rows)
