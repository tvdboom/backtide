"""Backtide.

Author: Mavs
Description: Backtest results page.

"""

from datetime import datetime as dt
from typing import TYPE_CHECKING

import pandas as pd
import streamlit as st

from backtide.backtest.utils import _load_benchmark_sidecar
from backtide.config import get_config
from backtide.storage import (
    delete_experiment,
    query_experiment_strategies,
    query_experiments,
)
from backtide.ui.utils import (
    _fmt_duration,
    _fmt_metric,
    _moment_to_strftime,
    _to_pandas,
)
from backtide.utils.utils import _format_price

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

        /* Tighten dividers inside cards */
        hr {
            margin: 0.25rem 0 0.85rem 0 !important;
        }
    </style>
    """,
    unsafe_allow_html=True,
)


def _fmt_pct(value: float | None, *, signed: bool = False) -> str:
    """Format a fraction as a percentage string with magnitude-adaptive decimals."""
    if value is None or pd.isna(value):
        value = 0.0
    return _fmt_metric(value * 100, signed=signed, suffix="%")


def _fmt_ts(ts: float) -> str:
    """Format a UNIX timestamp using the configured datetime format."""
    return dt.fromtimestamp(int(ts)).strftime(datetime_fmt)


def _tone(value: float | None, *, good_above: float = 0.0, bad_below: float = 0.0) -> str:
    """Return a Streamlit color name ('green' / 'red' / '') based on thresholds."""
    if value is None or pd.isna(value):
        return ""
    if value > good_above:
        return "green"
    if value < bad_below:
        return "red"
    return ""


def _colored_metric(container, label: str, value: str, tone: str = ""):
    """Render an `st.metric` with the value tinted using Streamlit's color palette.

    `tone` should be one of Streamlit's color names (e.g. ``"green"``, ``"red"``)
    or an empty string for the default/neutral color.

    """
    container.metric(label, f":{tone}[{value}]" if tone else value)


def _render_strategy_summary(run: StrategyRunResult, benchmark: dict | None = None):
    """Render compact summary metrics for a single strategy run."""
    st.markdown(f"**:material/psychology: {run.strategy_name}**")
    show_alpha = bool(benchmark and benchmark.get("alpha_per_strategy"))
    if show_alpha:
        mc1, mc2, mc3, mc4, mc5, mc6, mc7 = st.columns([1, 1, 1, 1, 1, 1.2, 1])
    else:
        mc1, mc2, mc3, mc4, mc5, mc6 = st.columns([1, 1, 1, 1, 1, 1.2])

    pnl = run.metrics.get("pnl", 0.0)
    total_return = run.metrics.get("total_return", 0.0)
    cagr = run.metrics.get("cagr", 0.0)
    sharpe = run.metrics.get("sharpe_ratio", 0.0)
    max_dd = run.metrics.get("max_drawdown", 0.0)

    pnl_str = _format_price(pnl, currency=cfg.general.base_currency, compact=True)
    if pnl >= 0:
        pnl_str = f"+{pnl_str}"
    _colored_metric(
        mc1,
        ":material/payments: PnL",
        pnl_str,
        _tone(pnl),
    )
    _colored_metric(
        mc2,
        ":material/stacked_line_chart: Return",
        _fmt_pct(total_return, signed=True),
        _tone(total_return),
    )
    _colored_metric(
        mc3,
        ":material/trending_up: CAGR",
        _fmt_pct(cagr, signed=True),
        _tone(cagr),
    )
    # Sharpe: >1 is good, <0 is bad, in-between is neutral.
    _colored_metric(
        mc4,
        ":material/speed: Sharpe",
        _fmt_metric(sharpe),
        _tone(sharpe, good_above=1.0, bad_below=0.0),
    )
    # Max drawdown: any non-zero drawdown is bad.
    dd_tone = "red" if max_dd and not pd.isna(max_dd) else ""
    _colored_metric(
        mc5,
        ":material/trending_down: Max DD",
        _fmt_pct(max_dd),
        dd_tone,
    )
    n_trades = int(run.metrics.get("n_trades", 0))
    win_rate = run.metrics.get("win_rate", 0.0) or 0.0
    wr_pct = _fmt_metric(win_rate * 100, suffix="%")
    if win_rate > 0.5:
        wr_str = f":green[{wr_pct}]"
    elif win_rate < 0.5:
        wr_str = f":red[{wr_pct}]"
    else:
        wr_str = wr_pct
    _colored_metric(
        mc6,
        ":material/swap_vert: Trades (w/r)",
        f"{n_trades} ({wr_str})",
    )

    if show_alpha:
        alpha = float(
            (benchmark.get("alpha_per_strategy") or {}).get(run.strategy_name, 0.0) or 0.0
        )
        _colored_metric(
            mc7,
            ":material/compare_arrows: Alpha",
            _fmt_pct(alpha, signed=True),
            _tone(alpha),
        )


def _render_experiment_metrics(row: pd.Series):
    """Render the top-level metrics for an experiment row."""
    c1, c2 = st.columns(2)
    total_return = row.get("total_return")
    if total_return is None or pd.isna(total_return):
        total_return = 0.0
    _colored_metric(
        c1,
        ":material/stacked_line_chart: Total Return",
        _fmt_pct(total_return, signed=True),
        _tone(total_return),
    )
    duration = max(0, int(row["finished_at"]) - int(row["started_at"]))
    _colored_metric(c2, ":material/timer: Duration", _fmt_duration(duration))


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
        cls, label = "error", "Failed"
    elif (tr := row.get("total_return")) is None or pd.isna(tr):
        cls, label = "warning", "Succeeded with warnings"
    else:
        cls, label = "success", "Succeeded"

    return f" <span class='status-badge {cls}' title='{label}'></span>"


def _render_tags(tags: str, *, container=None):
    """Render comma-separated tags as small inline pills.

    By default, writes to the current Streamlit container. Pass `container`
    (e.g., a column) to scope the render width.

    """
    if tags:
        if pills := "".join(f"<span class='tag-pill'>{t.strip()}</span>" for t in tags.split(",")):
            target = container if container is not None else st
            target.markdown(pills, unsafe_allow_html=True)


@st.dialog("Confirm deletion", width="medium")
def _confirm_delete_experiment(exp_id: str, name: str):
    """Show a modal asking the user to confirm deletion of an experiment."""
    st.warning(
        f"You are about to **permanently delete** experiment **{name}**.",
        icon=":material/warning:",
    )

    col1, col2 = st.columns(2)

    if col1.button("Cancel", width="stretch"):
        st.rerun()

    if col2.button("Delete", icon=":material/delete:", type="primary", width="stretch"):
        try:
            delete_experiment(exp_id)
        except Exception as ex:  # noqa: BLE001
            st.session_state["_delete_error"] = str(ex)
        else:
            if st.session_state.get("results_expanded") == exp_id:
                st.session_state.pop("results_expanded", None)
            st.session_state["_delete_success"] = name
        st.rerun()


def _render_full_analysis(row: pd.Series):
    """Render plots and tables for a single experiment.

    Parameters
    ----------
    row : pd.Series
        The experiment row previously retrieved from `query_experiments`.

    """
    back, title_col = st.columns([1, 5], vertical_alignment="center")
    if back.button(":material/arrow_back: Back", key="back_to_results", width=150):
        st.session_state.pop("selected_experiment", None)
        st.rerun()

    title_col.markdown(
        f"<h2 style='margin-top:-25px;padding:0;line-height:1'>{row['name'] or row['id']}</h2>",
        unsafe_allow_html=True,
    )

    _render_tags(row["tags"])

    strat_word = "strategy" if row["n_strategies"] == 1 else "strategies"
    st.markdown(
        f"""
        <div style="opacity:0.7;font-size:1.15em;margin-bottom:30px">
            {_fmt_ts(row["started_at"])}
            &nbsp;·&nbsp;
            {row["n_strategies"]} {strat_word}
            &nbsp;·&nbsp;
            {row["status"].capitalize()} {_status_badge(row)}
        </div>
        """,
        unsafe_allow_html=True,
    )

    if row["description"]:
        with st.expander("Description", icon=":material/description:"):
            st.write(row["description"])

    _render_experiment_metrics(row)

    benchmark = _load_benchmark_sidecar(cfg, row["id"])
    if benchmark:
        st.caption(
            f":material/compare_arrows: Benchmark: **{benchmark.get('symbol', '?')}** "
            f"(buy & hold) — return "
            f"{_fmt_pct(benchmark.get('total_return', 0.0), signed=True)}"
        )

    if not (runs := query_experiment_strategies(row["id"])):
        st.info("No strategy runs found for this experiment.")
        return

    tabs = st.tabs([run.strategy_name for run in runs])
    for tab, run in zip(tabs, runs, strict=True):
        with tab:
            _render_strategy_summary(run, benchmark=benchmark)

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
                if benchmark and benchmark.get("equity_curve") and run.equity_curve:
                    # Rescale benchmark curve to match the strategy's starting equity so
                    # the two are visually comparable on the same axis.
                    bench_pts = benchmark["equity_curve"]
                    base_bench = float(bench_pts[0]["equity"]) or 1.0
                    base_strat = float(run.equity_curve[0].equity) or 1.0
                    factor = base_strat / base_bench
                    bench_df = pd.DataFrame(
                        [
                            {
                                "timestamp": dt.fromtimestamp(int(p["timestamp"])),
                                "benchmark": float(p["equity"]) * factor,
                            }
                            for p in bench_pts
                        ]
                    )
                    merged = pd.merge_asof(
                        eq_df.sort_values("timestamp"),
                        bench_df.sort_values("timestamp"),
                        on="timestamp",
                        direction="nearest",
                    )
                    st.line_chart(
                        merged.set_index("timestamp")[["equity", "benchmark"]],
                        color=["#79b8ff", "#f1c40f"],
                    )
                else:
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

selected = st.session_state.get("selected_experiment")
if selected is not None:
    _render_full_analysis(pd.Series(selected))
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

df = _to_pandas(query_experiments(search=search))
if status_filter == "Succeeded":
    df = df[df["status"] == "completed"]
elif status_filter == "Failed":
    df = df[df["status"] == "failed"]

if del_ok := st.session_state.pop("_delete_success", None):
    st.success(f"Deleted experiment **{del_ok}**.", icon=":material/check_circle:")
if del_err := st.session_state.pop("_delete_error", None):
    st.error(f"Failed to delete experiment: {del_err}", icon=":material/error:")

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

# Track which experiment card is expanded (only one at a time).
expanded_id = st.session_state.get("results_expanded")

for _, row in df.iterrows():
    exp_id = row["id"]
    name = row["name"] or exp_id
    is_open = exp_id == expanded_id
    icon = _status_icon(row)

    with st.container(border=True):
        col1, col2, col3 = st.columns([6, 2, 0.7])

        col1.markdown(f"##### {icon}&nbsp;{name}{_status_badge(row)}", unsafe_allow_html=True)
        _render_tags(row["tags"], container=col1)

        if col2.button(
            "Full results",
            key=f"open_analysis_{exp_id}",
            icon=":material/fact_check:",
            type="secondary",
            width="stretch",
        ):
            st.session_state["selected_experiment"] = row.to_dict()
            st.rerun()

        if col3.button(
            "",
            key=f"delete_{exp_id}",
            icon=":material/delete:",
            type="primary",
            width="stretch",
            help="Delete this experiment from the database.",
        ):
            _confirm_delete_experiment(exp_id, name)

        col1, col2 = st.columns([3, 1], vertical_alignment="center")

        strategies = "strategy" if row["n_strategies"] == 1 else "strategies"
        col1.markdown(
            "<span style='display:inline-block;opacity:0.7;font-size:1.05em;"
            "margin-top:-20px'>"
            f":material/calendar_month: {_fmt_ts(row['started_at'])} "
            f"&nbsp;·&nbsp; "
            f":material/timer: {_fmt_duration(row['finished_at'] - row['started_at'])} "
            f"&nbsp;·&nbsp; "
            f":material/psychology: {row['n_strategies']} {strategies}"
            "</span>",
            unsafe_allow_html=True,
        )

        if col2.button(
            "Hide breakdown" if is_open else "Show breakdown",
            key=f"toggle_{exp_id}",
            icon=":material/keyboard_arrow_up:" if is_open else ":material/keyboard_arrow_down:",
            type="tertiary",
            width="stretch",
        ):
            if is_open:
                st.session_state.pop("results_expanded", None)
            else:
                st.session_state["results_expanded"] = exp_id
            st.rerun()

        if is_open:
            st.divider()

            row_benchmark = _load_benchmark_sidecar(cfg, exp_id)
            for i, run in enumerate(query_experiment_strategies(exp_id)):
                if i > 0:
                    st.markdown("<div style='margin-top:1.25rem'></div>", unsafe_allow_html=True)
                _render_strategy_summary(run, benchmark=row_benchmark)
