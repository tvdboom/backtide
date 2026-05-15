"""Backtide.

Author: Mavs
Description: Home dashboard page.

"""

from collections import defaultdict
from datetime import datetime as dt

import pandas as pd
import streamlit as st

from backtide.analysis.utils import GREEN, RED, YELLOW
from backtide.core.config import get_config
from backtide.core.data import Interval, download_bars, resolve_profiles
from backtide.core.storage import query_bars, query_experiments, query_instruments
from backtide.ui.utils import (
    _fmt_number,
    _get_logokit_url,
    _query_bars_summary,
)
from backtide.utils.utils import _get_timezone, _moment_to_strftime, _to_pandas

# ─────────────────────────────────────────────────────────────────────────────
# Config
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
tz = _get_timezone(cfg.display.timezone)
logokit_key = cfg.display.logokit_api_key
datetime_fmt = _moment_to_strftime(cfg.display.datetime_format())

st.set_page_config(page_title="Backtide - Home")

# Handle pending navigation from button clicks (resolved on rerun)
if _nav_target := st.session_state.pop("_home_nav", None):
    st.switch_page(_nav_target)

# ─────────────────────────────────────────────────────────────────────────────
# Custom CSS
# ─────────────────────────────────────────────────────────────────────────────

st.markdown(
    f"""
    <style>
        .home-hero {{
            text-align: center;
            padding: 0.5rem 0 1.2rem;
        }}

        .home-hero h2 {{
            margin: 0;
            font-size: 1.8rem;
            font-weight: 700;
        }}

        .home-hero p {{
            margin: 0.3rem 0 0;
            opacity: 0.6;
            font-size: 1rem;
        }}

        /* Equal-height columns */
        [data-testid="stHorizontalBlock"] {{
            align-items: stretch;
        }}

        [data-testid="stHorizontalBlock"] > [data-testid="stColumn"] {{
            display: flex;
            flex-direction: column;
        }}

        [data-testid="stColumn"] > div:has([data-testid="stVerticalBlock"]) {{
            flex: 1;
        }}

        /* Hover highlight on bordered containers inside columns */
        [data-testid="stColumn"] > div {{
            transition: box-shadow 0.2s ease;
            border-radius: 0.5rem;
        }}

        [data-testid="stColumn"] > div:has(> div > [data-testid="stVerticalBlock"]):hover {{
            box-shadow: 0 4px 16px rgba(0,0,0,0.35);
        }}

        /* Spacing inside containers */
        [data-testid="stColumn"] [data-testid="stVerticalBlock"] {{
            gap: 0.6rem !important;
        }}

        .widget-header {{
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 8px;
        }}

        .widget-logo {{
            height: 56px;
            width: 56px;
            border-radius: 8px;
            object-fit: contain;
        }}

        .widget-symbol {{
            font-size: 1.15rem;
            font-weight: 700;
        }}

        .widget-symbol.truncate {{
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            max-width: 180px;
            display: block;
        }}

        .widget-name {{
            font-size: 0.85rem;
            opacity: 0.6;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            max-width: 180px;
        }}

        .widget-price {{
            font-size: 1.4rem;
            font-weight: 600;
            margin-top: 4px;
        }}

        .widget-change {{
            font-size: 0.9rem;
            font-weight: 600;
        }}

        .widget-change.positive {{ color: {GREEN}; }}
        .widget-change.negative {{ color: {RED}; }}

        .widget-detail {{
            font-size: 0.9rem;
            opacity: 0.7;
            margin-top: 2px;
        }}

        .icon-block {{
            width: 54px;
            height: 54px;
            border-radius: 10px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 1.4rem;
            flex-shrink: 0;
        }}

        .exp-status {{
            display: inline-block;
            font-size: 0.7rem;
            padding: 2px 6px;
            border-radius: 4px;
            text-transform: uppercase;
            letter-spacing: 0.04em;
            font-weight: 600;
            margin-top: 12px;
            margin-bottom: 12px;
        }}

        .exp-status.Success {{
            background: rgba(34,197,94,0.12);
            color: #22c55e;
            border: 1px solid rgba(34,197,94,0.3);
        }}

        .exp-status.Error {{
            background: rgba(239,68,68,0.12);
            color: #ef4444;
            border: 1px solid rgba(239,68,68,0.3);
        }}

        .exp-status.Partial {{
            background: rgba(250,204,21,0.12);
            color: #eab308;
            border: 1px solid rgba(250,204,21,0.3);
        }}

        .nav-title {{
            font-size: 1.1rem;
            font-weight: 700;
        }}

        .nav-desc {{
            font-size: 0.82rem;
            opacity: 0.55;
            margin-top: 1px;
        }}

        .nav-stat {{
            font-size: 1.3rem;
            font-weight: 600;
            margin-top: 8px;
            display: flex;
            align-items: baseline;
            gap: 6px;
        }}

        .nav-stat-icon {{
            font-size: 1rem;
            opacity: 0.65;
            width: 20px;
            text-align: center;
            margin-right: 10px;
        }}

        .nav-stat-label {{
            font-size: 0.85rem;
            opacity: 0.5;
        }}

        .section-label {{
            font-size: 0.75rem;
            font-weight: 600;
            color: #888;
            letter-spacing: 0.08em;
            text-transform: uppercase;
            margin: 1.5rem 0 0.6rem;
        }}
    </style>
    """,
    unsafe_allow_html=True,
)


@st.cache_data(ttl=7200, show_spinner="Updating latest prices...")
def _load_daily_bars(symbols: list[str]) -> pd.DataFrame:
    """Download and return daily bars from the last stored date to today."""
    groups = defaultdict(lambda: ([], 0))
    for sym in symbols:
        if inst := all_instruments.get(sym):
            sym_summary = latest[latest["symbol"] == sym]
            if sym_summary.empty:
                continue

            last_ts = sym_summary.iloc[0]["last_ts"]
            syms, ts = groups[inst.instrument_type]
            syms.append(sym)

            # Use the earliest last_ts so we don't miss data for any symbol.
            groups[inst.instrument_type] = (syms, min(ts, last_ts) if ts else last_ts)

    for it, (syms, start_ts) in groups.items():
        try:
            profiles = resolve_profiles(syms, it, interval="1d", verbose=False)
            download_bars(profiles, start=start_ts, verbose=False)
        except Exception:  # noqa: BLE001
            pass  # Silently skip failures on the home page

    return (
        _to_pandas(query_bars(symbol=symbols))
        .sort_values("open_ts", ascending=False)
        .groupby("symbol", sort=False)
        .head(3)
        .reset_index(drop=True)
    )


# ─────────────────────────────────────────────────────────────────────────────
# Load data
# ─────────────────────────────────────────────────────────────────────────────

all_instruments = {x.symbol: x for x in query_instruments()}
summary = _to_pandas(_query_bars_summary())

if not summary.empty:
    n_symbols = summary["symbol"].nunique()
    n_series = len(summary)
    n_bars = summary["n_rows"].sum()
else:
    n_symbols = n_series = n_bars = 0

experiments = _to_pandas(query_experiments())

# ─────────────────────────────────────────────────────────────────────────────
# Title
# ─────────────────────────────────────────────────────────────────────────────

st.markdown('<div class="home-hero"><h2>Welcome to Backtide</h2></div>', unsafe_allow_html=True)

# ─────────────────────────────────────────────────────────────────────────────
# Recent experiments
# ─────────────────────────────────────────────────────────────────────────────

if not experiments.empty:
    st.markdown('<div class="section-label">Recent experiments</div>', unsafe_allow_html=True)

    experiment_rows = list(experiments.iloc[:3].iterrows())
    for col, (_, row) in zip(st.columns(len(experiment_rows)), experiment_rows, strict=True):
        with col:
            status = row["status"]
            n_strats = row["n_strategies"]
            best_sharpe = row["best_sharpe"]
            started = row["started_at"]

            if best_sharpe is not None and not pd.isna(best_sharpe):
                sharpe_html = f'<div class="widget-detail">🏅 Sharpe: {best_sharpe:.2f}</div>'
            else:
                sharpe_html = ""

            status_lower = status.lower()
            if status_lower == "success":
                color = (f"rgba{GREEN[3:-1]}, 0.15)",)
            elif status_lower == "partial":
                color = (f"rgba{YELLOW[3:-1]}, 0.15)",)
            else:
                color = (f"rgba{RED[3:-1]}, 0.15)",)

            with st.container(border=True):
                st.markdown(
                    f"""
                    <div style="min-height:130px;">
                    <div class="widget-header">
                        <div class="icon-block" style="background:{color};font-size:1.8em">
                            {row["icon"]}
                        </div>
                        <div style="overflow:hidden;">
                            <div class="widget-symbol truncate">{row["name"]}</div>
                            <div class="widget-name">
                                {dt.fromtimestamp(started, tz=tz).strftime(datetime_fmt)}
                            </div>
                        </div>
                    </div>
                    <div class="widget-detail">
                        🧠 {f"{n_strats} strateg{'y' if n_strats == 1 else 'ies'}"}
                    </div>
                    {sharpe_html}
                    <span class="exp-status {status}">{status}</span>
                    </div>
                    """,
                    unsafe_allow_html=True,
                )

                st.markdown("")

                st.button(
                    "View results →",
                    key=row["id"],
                    width="stretch",
                    type="tertiary",
                    on_click=lambda r=row: st.session_state.update(
                        selected_experiment=r.to_dict(),
                        _home_nav="results.py",
                    ),
                )


# ─────────────────────────────────────────────────────────────────────────────
# Most recent symbols
# ─────────────────────────────────────────────────────────────────────────────

if not summary.empty:
    st.markdown('<div class="section-label">Recently used symbols</div>', unsafe_allow_html=True)

    latest = (
        summary.sort_values("last_ts", ascending=False)
        .drop_duplicates(subset="symbol", keep="first")
        .head(3)
    )

    # The widgets show the latest data
    bars = _load_daily_bars(recent_symbols := latest["symbol"].tolist())

    widgets = []
    for sym in recent_symbols:
        if inst := all_instruments.get(sym):
            sym_bars = bars[bars["symbol"] == sym].sort_values("open_ts", ascending=False)

            if sym_bars.empty:
                continue

            last_close = sym_bars.iloc[0]["close"]
            if len(sym_bars) >= 2:
                prev_close = sym_bars.iloc[1]["close"]
                if prev_close and prev_close != 0:
                    change_pct = ((last_close - prev_close) / prev_close) * 100
                else:
                    change_pct = 0.0
            else:
                change_pct = 0.0

            widgets.append(
                {
                    "symbol": sym,
                    "name": inst.name if inst.instrument_type.is_equity else sym,
                    "instrument_type": inst.instrument_type,
                    "price": last_close,
                    "change_pct": change_pct,
                    "quote": str(inst.quote),
                }
            )

    if widgets:
        visible_widgets = widgets[:3]
        for col, w in zip(st.columns(len(visible_widgets)), visible_widgets, strict=True):
            with col:
                change_cls = "positive" if w["change_pct"] >= 0 else "negative"
                change_sign = "+" if w["change_pct"] >= 0 else ""
                change_arrow = "▲" if w["change_pct"] >= 0 else "▼"

                price = w["price"]
                if price >= 1000:
                    price_str = f"{price:,.2f}"
                elif price >= 1:
                    price_str = f"{price:.2f}"
                else:
                    price_str = f"{price:.4f}"

                if logokit_key:
                    url = _get_logokit_url(w["symbol"], w["instrument_type"], logokit_key)
                    logo_html = f'<img src="{url}" class="widget-logo">'
                else:
                    logo_html = ""

                with st.container(border=True):
                    st.markdown(
                        f"""
                        <div class="widget-header">
                            {logo_html}
                            <div>
                                <div class="widget-symbol">{w["symbol"]}</div>
                                <div class="widget-name">{w["name"]}</div>
                            </div>
                        </div>
                        <div class="widget-price">
                            {price_str}
                            <span style="font-size:0.7em;opacity:0.5;">{w["quote"]}</span>
                        </div>
                        <div class="widget-change {change_cls}">
                            {change_arrow} {change_sign}{w["change_pct"]:.2f}%
                        </div>
                        """,
                        unsafe_allow_html=True,
                    )

                    st.markdown("")

                    st.button(
                        "Analyze →",
                        key=f"analyze_{w['symbol']}",
                        width="stretch",
                        type="tertiary",
                        on_click=lambda s=w["symbol"]: st.session_state.update(
                            _symbols=[s],
                            _interval=Interval.get_default(),
                            _home_nav="analysis.py",
                        ),
                    )

# ─────────────────────────────────────────────────────────────────────────────
# Quick navigation
# ─────────────────────────────────────────────────────────────────────────────

st.markdown('<div class="section-label">Quick navigation</div>', unsafe_allow_html=True)

col1, col2, col3 = st.columns(3)
with col1:
    with st.container(border=True):
        st.markdown(
            f"""
            <div class="widget-header">
                <div class="icon-block" style="background:rgba(99,179,237,0.15);">☁️</div>
                <div>
                    <div class="nav-title">Download</div>
                    <div class="nav-desc">Fetch market data</div>
                </div>
            </div>
            <div class="nav-stat">
                <span class="nav-stat-icon">📈</span> {_fmt_number(len(summary))}
                <span class="nav-stat-label"> series</span>
            </div>
            <div class="nav-stat">
                <span class="nav-stat-icon">📦</span> {_fmt_number(n_bars)}
                <span class="nav-stat-label"> bars</span>
            </div>
            """,
            unsafe_allow_html=True,
        )
        st.markdown("")
        st.button(
            label="Go to Download →",
            key="nav_download",
            width="stretch",
            type="tertiary",
            on_click=lambda: st.session_state.update(_home_nav="download.py"),
        )

with col2:
    with st.container(border=True):
        st.markdown(
            f"""
            <div class="widget-header">
                <div class="icon-block" style="background:rgba(99,179,237,0.15);">🧪</div>
                <div>
                    <div class="nav-title">Experiment</div>
                    <div class="nav-desc">Run a new backtest</div>
                </div>
            </div>
            <div class="nav-stat">
                <span class="nav-stat-icon">🔬</span> {len(experiments)}
                <span class="nav-stat-label"> experiments</span>
            </div>
            <div class="nav-stat">
                <span class="nav-stat-icon">📈</span> {experiments["n_strategies"].sum()}
                <span class="nav-stat-label"> strategies</span>
            </div>
            """,
            unsafe_allow_html=True,
        )
        st.markdown("")
        st.button(
            label="Go to Experiment →",
            key="nav_experiment",
            width="stretch",
            type="tertiary",
            on_click=lambda: st.session_state.update(_home_nav="experiment.py"),
        )

with col3:
    with st.container(border=True):
        st.markdown(
            f"""
            <div class="widget-header">
                <div class="icon-block" style="background:rgba(99,179,237,0.15);">📊</div>
                <div>
                    <div class="nav-title">Analysis</div>
                    <div class="nav-desc">Explore the stored data</div>
                </div>
            </div>
            <div class="nav-stat">
                <span class="nav-stat-icon">📌</span> {n_symbols}
                <span class="nav-stat-label"> symbols</span>
            </div>
            <div class="nav-stat" style="visibility:hidden;">
                <span class="nav-stat-icon">&nbsp;</span> &nbsp;
            </div>
            """,
            unsafe_allow_html=True,
        )
        st.markdown("")
        st.button(
            label="Go to Analysis →",
            key="nav_analysis",
            width="stretch",
            type="tertiary",
            on_click=lambda: st.session_state.update(_home_nav="analysis.py"),
        )
