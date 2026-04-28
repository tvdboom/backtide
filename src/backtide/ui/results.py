"""Backtide.

Author: Mavs
Description: Backtest results page.

"""

from datetime import datetime as dt
from typing import TYPE_CHECKING

import pandas as pd
import streamlit as st

from backtide.config import get_config
from backtide.storage import query_experiment_strategies, query_experiments
from backtide.ui.utils import _moment_to_strftime, _to_pandas

if TYPE_CHECKING:
    from backtide.backtest import StrategyRunResult


cfg = get_config()
datetime_fmt = _moment_to_strftime(cfg.display.datetime_format())

st.set_page_config(page_title="Backtide - Results")

st.markdown(
    """
    <style>
        .tag-pill {
            display: inline-block;
            padding: 2px 10px;
            margin: 0 4px 4px 0;
            border-radius: 12px;
            background: rgba(135, 180, 255, 0.12);
            color: #79b8ff;
            border: 1px solid rgba(135, 180, 255, 0.5);
            font-size: 0.75em;
            font-weight: 500;
            line-height: 1.4;
        }

        /* Compact metrics */
        [data-testid="stMetricLabel"] {
            font-size: 0.82em;
        }
        [data-testid="stMetricValue"] {
            font-size: 1.3em;
        }

        .status-badge {
            display: inline-block;
            width: 12px;
            height: 12px;
            border-radius: 50%;
            vertical-align: baseline;
            position: relative;
            top: -0.1em;
            margin-left: 4px;
        }
        .status-badge.success { background: #2ecc71; }
        .status-badge.warning { background: #f1c40f; }
        .status-badge.error   { background: #e74c3c; }
    </style>
    """,
    unsafe_allow_html=True,
)


def _fmt_pct(value: float | None, signed: bool = False) -> str:
    """Format a fraction as a percentage string."""
    if value is None or pd.isna(value):
        value = 0.0
    return f"{value * 100:+.2f}%" if signed else f"{value * 100:.2f}%"


def _fmt_ts(ts: float) -> str:
    """Format a UNIX timestamp using the configured datetime format."""
    return dt.fromtimestamp(int(ts)).strftime(datetime_fmt)


def _render_strategy_summary(run: StrategyRunResult) -> None:
    """Render compact summary metrics for a single strategy run."""
    st.markdown(f"**:material/psychology: {run.strategy_name}**")
    mc1, mc2, mc3, mc4 = st.columns(4)
    mc1.metric(":material/stacked_line_chart: Return", _fmt_pct(run.metrics.get("total_return", 0.0), signed=True))
    mc2.metric(":material/speed: Sharpe", f"{run.metrics.get('sharpe_ratio', 0.0):.2f}")
    mc3.metric(":material/trending_down: Max DD", _fmt_pct(run.metrics.get("max_drawdown", 0.0)))
    mc4.metric(
        ":material/swap_vert: Trades",
        f"{int(run.metrics.get('n_trades', 0))} ({run.metrics.get('win_rate', 0.0) * 100:.0f}% wins)",
    )


def _render_experiment_metrics(row: pd.Series) -> None:
    """Render the top-level metrics for an experiment row."""
    c1, c2 = st.columns(2)
    total_return = row.get("total_return")
    if total_return is None or pd.isna(total_return):
        total_return = 0.0
    c1.metric(":material/stacked_line_chart: Total Return", _fmt_pct(total_return, signed=True))
    duration = max(0, int(row["finished_at"]) - int(row["started_at"]))
    c2.metric(":material/timer: Duration", f"{duration}s")


def _status_icon(row: pd.Series) -> str:
    """Return a Material icon shortcode based on the experiment's status / return."""
    if row["status"] == "failed":
        return ":material/error:"

    total_return = row.get("total_return")
    if total_return is None or pd.isna(total_return):
        return ":material/trending_flat:"
    elif total_return > 0:
        return ":material/trending_up:"
    elif total_return < 0:
        return ":material/trending_down:"

    return ":material/trending_flat:"


def _status_badge(row: pd.Series) -> str:
    """Return a small colored status dot for the experiment."""
    if row["status"] == "failed":
        cls = "error"
    elif (tr := row.get("total_return")) is None or pd.isna(tr):
        cls = "warning"
    else:
        cls = "success"

    return f" <span class='status-badge {cls}'></span>"


def _render_tags(tags: str, *, container=None):
    """Render comma-separated tags as small inline pills.

    By default, writes to the current Streamlit container. Pass `container`
    (e.g., a column) to scope the render width.

    """
    if tags:
        if pills := "".join(f"<span class='tag-pill'>{t.strip()}</span>" for t in tags.split(",")):
            target = container if container is not None else st
            target.markdown(pills, unsafe_allow_html=True)


def _render_full_analysis(experiment_id: str) -> None:
    """Render plots and tables for a single experiment."""
    df_exp: pd.DataFrame = query_experiments(None)
    match = df_exp[df_exp["id"] == experiment_id]
    if match.empty:
        st.error("Experiment not found.")
        if st.button("← Back to results"):
            st.session_state.pop("selected_experiment_id", None)
            st.rerun()
        return

    row = match.iloc[0]

    back, title_col = st.columns([1, 5], vertical_alignment="center")
    if back.button("← Back", key="back_to_results"):
        st.session_state.pop("selected_experiment_id", None)
        st.rerun()

    icon = _status_icon(row)
    badge = _status_badge(row)
    title_col.markdown(f"### {icon} &nbsp;{row['name'] or row['id']}{badge}", unsafe_allow_html=True)
    st.caption(
        f"Started {_fmt_ts(row['started_at'])} · "
        f"{int(row['n_strategies'])} strategies · status: {row['status']}"
    )
    _render_tags(row["tags"])

    _render_experiment_metrics(row)

    if row["description"]:
        st.caption(row["description"])

    runs = query_experiment_strategies(experiment_id)
    if not runs:
        st.info("No strategy runs found for this experiment.")
        return

    tabs = st.tabs([run.strategy_name for run in runs])
    for tab, run in zip(tabs, runs):
        with tab:
            _render_strategy_summary(run)

            if run.equity_curve:
                st.markdown("##### Equity curve")
                eq_df = pd.DataFrame(
                    [
                        {
                            "timestamp": dt.fromtimestamp(s.timestamp),
                            "equity": s.equity,
                        }
                        for s in run.equity_curve
                    ]
                )
                st.line_chart(eq_df.set_index("timestamp")["equity"])

            if run.trades:
                st.markdown("##### Trades")
                trades_df = pd.DataFrame(
                    [
                        {
                            "symbol": t.symbol,
                            "qty": t.quantity,
                            "entry": dt.fromtimestamp(t.entry_ts),
                            "exit": dt.fromtimestamp(t.exit_ts),
                            "entry_price": t.entry_price,
                            "exit_price": t.exit_price,
                            "pnl": t.pnl,
                        }
                        for t in run.trades
                    ]
                )
                st.dataframe(trades_df, hide_index=True, width="stretch")
            else:
                st.caption("No closed trades.")


# ─────────────────────────────────────────────────────────────────────────────
# Routing
# ─────────────────────────────────────────────────────────────────────────────

selected = st.session_state.get("selected_experiment_id")
if selected:
    _render_full_analysis(selected)
    st.stop()


# ─────────────────────────────────────────────────────────────────────────────
# Results logic
# ─────────────────────────────────────────────────────────────────────────────

st.subheader("Results", text_alignment="center")
st.write("")

col1, col2 = st.columns([4, 1], vertical_alignment="bottom")

search = col1.text_input(
    label="Search experiments",
    key="experiments_search",
    placeholder="Search by name or tag...",
    help="Case-insensitive substring match against experiment name and tags.",
)

status_filter = col2.selectbox(
    label="Status",
    options=("All", "Succeeded", "Failed"),
    key="experiments_status",
    help="Filter experiments by status.",
)

df = _to_pandas(query_experiments(search))
if status_filter == "Succeeded":
    df = df[df["status"] == "completed"]
elif status_filter == "Failed":
    df = df[df["status"] == "failed"]

if df.empty:
    if search or status_filter != "All":
        st.info("No experiments match your search.", icon=":material/info:")
    else:
        st.info(
            "No experiments yet. Head over to the **Experiment** page to run your first backtest.",
            icon=":material/info:",
        )
        if st.button("Create a new experiment", icon=":material/science:"):
            st.switch_page("experiment.py")
    st.stop()

# Track which experiment cards are expanded (lazy loading of strategy details).
expanded: set[str] = st.session_state.setdefault("results_expanded", set())

for _, row in df.iterrows():
    exp_id = row["id"]
    name = row["name"] or exp_id
    is_open = exp_id in expanded
    icon = _status_icon(row)

    with st.container(border=True):
        col1, col2, col3 = st.columns([4, 2.2, 2])

        col1.markdown(f"##### {icon}&nbsp;{name}{_status_badge(row)}", unsafe_allow_html=True)
        _render_tags(row["tags"], container=col1)

        if col2.button(
            "Hide breakdown" if is_open else "Show breakdown",
            key=f"toggle_{exp_id}",
            icon=":material/keyboard_arrow_up:" if is_open else ":material/keyboard_arrow_down:",
            width="stretch",
        ):
            if is_open:
                expanded.discard(exp_id)
            else:
                expanded.add(exp_id)
            st.rerun()

        if col3.button(
            "Full results",
            key=f"open_analysis_{exp_id}",
            icon=":material/fact_check:",
            type="primary",
            width="stretch",
        ):
            st.session_state["selected_experiment_id"] = exp_id
            st.rerun()

        strat_word = "strategy" if row["n_strategies"] == 1 else "strategies"
        st.markdown(
            f"""
            <div style="opacity:0.7;font-size:1.05em;margin-bottom:{-0.5 if is_open else 1}rem">
                {_fmt_ts(row["started_at"])} &nbsp;·&nbsp; {row["n_strategies"]} {strat_word}
            </div>
            """,
            unsafe_allow_html=True,
        )

        if is_open:
            st.divider()

            for run in query_experiment_strategies(exp_id):
                _render_strategy_summary(run)
