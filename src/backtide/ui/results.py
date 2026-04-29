"""Backtide.

Author: Mavs
Description: Backtest results page.

"""

from datetime import datetime as dt
from pathlib import Path
from typing import TYPE_CHECKING

import pandas as pd
import streamlit as st

from backtide.backtest import ExperimentConfig
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
from backtide.utils.constants import BENCHMARK_NAME
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

        /* Full-results page experiment title */
        .experiment-title {
            text-align: center;
            font-size: 2em;
            font-weight: 700;
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

    `tone` should be one of Streamlit's color names (e.g., "green", "red") or an
     empty string for the default/neutral color.

    """
    container.metric(label, f":{tone}[{value}]" if tone else value)


def _split_benchmark(runs: list[StrategyRunResult]) -> tuple[list, StrategyRunResult | None]:
    """Split `runs` into (user_runs, benchmark_run).

    The benchmark run is identified by a strategy name matching the pattern
    ``Benchmark(<symbol>)``, regardless of its position in the list.

    """
    for i, run in enumerate(runs):
        if BENCHMARK_NAME.match(run.strategy_name):
            return [*runs[:i], *runs[i + 1:]], run

    return runs, None


def _render_strategy_summary(run: StrategyRunResult):
    """Render compact summary metrics for a single strategy run."""
    mc1, mc2, mc3, mc4, mc5, mc6, mc7 = st.columns([0.8, 0.8, 1, 1, 1, 1, 1.2])

    pnl = run.metrics.get("pnl", 0.0)
    total_return = run.metrics.get("total_return", 0.0)
    cagr = run.metrics.get("cagr", 0.0)
    alpha = run.metrics.get("alpha", 0.0)
    sharpe = run.metrics.get("sharpe_ratio", 0.0)
    max_dd = run.metrics.get("max_drawdown", 0.0)

    # Sharpe is the headline risk-adjusted metric: leads the row.
    _colored_metric(
        mc1,
        ":material/military_tech: Sharpe",
        _fmt_metric(sharpe),
        _tone(sharpe, good_above=1.0, bad_below=0.0),  # >1 good, <0 bad
    )

    pnl_str = _format_price(pnl, currency=cfg.general.base_currency, compact=True)
    _colored_metric(
        mc2,
        ":material/payments: P&L",
        f"{'+' if pnl > 0 else ''}{pnl_str}",
        _tone(pnl),
    )
    _colored_metric(
        mc3,
        ":material/stacked_line_chart: Return",
        _fmt_pct(total_return, signed=True),
        _tone(total_return),
    )
    _colored_metric(
        mc4,
        ":material/trending_up: CAGR",
        _fmt_pct(cagr, signed=True),
        _tone(cagr),
    )
    _colored_metric(
        mc5,
        ":material/compare_arrows: Alpha",
        _fmt_pct(alpha, signed=True),
        _tone(alpha),
    )
    _colored_metric(
        mc6,
        ":material/trending_down: Max DD",
        _fmt_pct(max_dd),
        "red" if max_dd and not pd.isna(max_dd) else "",  # Any non-zero drawdown is bad.
    )
    n_trades = int(run.metrics.get("n_trades", 0))
    win_rate = run.metrics.get("win_rate", 0.0)
    wr_pct = _fmt_metric(win_rate * 100, suffix="%")
    if win_rate > 0.5:
        wr_str = f":green[{wr_pct}]"
    elif win_rate < 0.5:
        wr_str = f":red[{wr_pct}]"
    else:
        wr_str = wr_pct
    _colored_metric(mc7, ":material/swap_vert: Trades (w/r)", f"{n_trades} ({wr_str})")


def _status_icon(row: pd.Series) -> str:
    """Return a Material icon shortcode based on the experiment's status / Sharpe."""
    if row["status"] == "failed":
        return ":material/error:"

    sharpe = row.get("best_sharpe")
    if sharpe is None or pd.isna(sharpe):
        return ":material/trending_flat:"
    elif sharpe >= 1.0:
        return ":material/trending_up:"
    elif sharpe < 0.0:
        return ":material/trending_down:"

    return ":material/trending_flat:"


def _status_badge(row: pd.Series) -> str:
    """Return a small colored status dot for the experiment."""
    if row["status"] == "failed":
        cls, label = "error", "Failed"
    elif (s := row.get("best_sharpe")) is None or pd.isna(s):
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
            if (sel := st.session_state.get("selected_experiment")) and sel.get("id") == exp_id:
                st.session_state.pop("selected_experiment", None)
            st.session_state["_delete_success"] = name
        st.rerun()


def _render_full_analysis(row: pd.Series):
    """Render plots and tables for a single experiment.

    Parameters
    ----------
    row : pd.Series
        The experiment row previously retrieved from `query_experiments`.

    """
    exp_id = row["id"]
    name = row["name"] or exp_id
    cfg_path = Path(cfg.data.storage_path) / "experiments" / f"{exp_id}.toml"
    export_disabled = not cfg_path.is_file()

    with st.container(key="full_results_toolbar"):
        col1, col2, col3, col4, col5 = st.columns([1.2, 5.3, 0.7, 0.7, 0.7], gap="xxsmall")

        if col1.button(
            ":material/arrow_back: Back",
            key="back_to_results",
            width="stretch",
        ):
            st.session_state.pop("selected_experiment", None)
            st.rerun()

        with col3.popover(
            "",
            icon=":material/description:",
            help=(
                "Show the experiment description."
                if row["description"]
                else "No description for this experiment."
            ),
            disabled=not row["description"],
        ):
            if row["description"]:
                st.markdown(f"**{name}**")
                st.write(row["description"])

        if col4.button(
            "",
            key=f"export_full_{exp_id}",
            icon=":material/upload:",
            type="secondary",
            width="stretch",
            disabled=export_disabled,
            help=(
                "Open this experiment's configuration in the **Experiment** page."
                if not export_disabled
                else "No saved configuration found for this experiment."
            ),
        ):
            try:
                exp_cfg = ExperimentConfig.from_toml(cfg_path.read_text(encoding="utf-8"))
            except Exception as ex:  # noqa: BLE001
                st.session_state["_delete_error"] = f"Failed to load configuration: {ex}"
                st.rerun()
            else:
                st.session_state["_pending_experiment_config"] = exp_cfg
                st.switch_page("experiment.py")

        if col5.button(
            "",
            key=f"delete_full_{exp_id}",
            icon=":material/delete:",
            type="primary",
            width="stretch",
            help="Delete this experiment from the database.",
        ):
            _confirm_delete_experiment(exp_id, name)

    st.markdown(f"<div class='experiment-title'>{name}</div>", unsafe_allow_html=True)

    if row["tags"]:
        pills = " ".join(
            f"<span class='tag-pill'>{t.strip()}</span>" for t in row["tags"].split(",")
        )
        st.markdown(
            f"<div style='text-align:center;margin:0.35rem'>{pills}</div>",
            unsafe_allow_html=True,
        )

    st.markdown("")

    c1, c2, c3, c4 = st.columns(4)

    sharpe = row.get("best_sharpe")
    if sharpe is None or pd.isna(sharpe):
        sharpe = 0.0
    _colored_metric(
        c1,
        ":material/military_tech: Best Sharpe",
        _fmt_metric(sharpe),
        _tone(sharpe, good_above=1.0, bad_below=0.0),
    )

    duration = max(0, int(row["finished_at"]) - int(row["started_at"]))
    _colored_metric(c2, ":material/timer: Run duration", _fmt_duration(duration))
    _colored_metric(c3, ":material/event: Started at", _fmt_ts(row["started_at"]))
    _colored_metric(c4, ":material/event_available: Finished at", _fmt_ts(row["finished_at"]))

    # ── Simulation context row (loaded from the persisted config TOML) ──
    cfg_path = Path(cfg.data.storage_path) / "experiments" / f"{row['id']}.toml"
    if not cfg_path.is_file():
        return

    try:
        exp_cfg = ExperimentConfig.from_toml(cfg_path.read_text(encoding="utf-8"))
    except Exception:  # noqa: BLE001
        return

    start = str(exp_cfg.data.start_date) if exp_cfg.data.start_date else None
    end = str(exp_cfg.data.end_date) if exp_cfg.data.end_date else None

    period_str = "Full history"
    length_str = "—"
    if start and end:
        try:
            d0 = dt.fromisoformat(start).date()
            d1 = dt.fromisoformat(end).date()
        except ValueError:
            period_str = f"{start} → {end}"
        else:
            date_fmt = _moment_to_strftime(cfg.display.date_format)
            period_str = f"{d0.strftime(date_fmt)} → {d1.strftime(date_fmt)}"
            n_days = max(0, (d1 - d0).days)
            length_str = f"{n_days} day{'s' if n_days != 1 else ''}"
    elif start:
        period_str = f"from {start}"
    elif end:
        period_str = f"until {end}"

    n_symbols = len(exp_cfg.data.symbols)
    symbols_str = (
        ", ".join(exp_cfg.data.symbols[:3])
        + (f" (+{n_symbols - 3})" if n_symbols > 3 else "")
        if n_symbols
        else "—"
    )

    c1, c2, c3, c4 = st.columns(4)
    _colored_metric(c1, ":material/date_range: Period", period_str)
    _colored_metric(c2, ":material/calendar_month: Length", length_str)
    _colored_metric(c3, ":material/schedule: Interval", str(exp_cfg.data.interval))
    _colored_metric(c4, ":material/finance: Symbols", f"{n_symbols} · {symbols_str}")

    st.markdown("")

    runs = query_experiment_strategies(row["id"])
    tabs = st.tabs([f"**{run.strategy_name}**" for run in runs])
    for tab, run in zip(tabs, runs, strict=True):
        with tab:
            _render_strategy_summary(run)

            if run.equity_curve:
                st.markdown("**Equity curve**")
                eq_df = pd.DataFrame(
                    [
                        {
                            "timestamp": dt.fromtimestamp(s.timestamp),
                            "equity": s.equity,
                        }
                        for s in run.equity_curve
                    ]
                )
                if benchmark is not None and benchmark.equity_curve and run.equity_curve:
                    # Rescale benchmark curve to match the strategy's starting equity so
                    # the two are visually comparable on the same axis.
                    base_bench = float(benchmark.equity_curve[0].equity) or 1.0
                    base_strat = float(run.equity_curve[0].equity) or 1.0
                    factor = base_strat / base_bench
                    bench_df = pd.DataFrame(
                        [
                            {
                                "timestamp": dt.fromtimestamp(int(s.timestamp)),
                                "benchmark": float(s.equity) * factor,
                            }
                            for s in benchmark.equity_curve
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

# Another page can request a specific experiment to be opened. We resolve it to
# a full row here so the rest of this page only deals with `selected_experiment`.
if sel_id := st.session_state.pop("selected_experiment_id", None):
    df = _to_pandas(query_experiments(sel_id))
    if not df.empty:
        st.session_state["selected_experiment"] = df.iloc[0].to_dict()
    else:
        st.session_state.pop("selected_experiment", None)
        st.error(f"Experiment **{sel_id}** not found.", icon=":material/error:")

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
        col1, col2, col3, col4 = st.columns([6, 2, 0.7, 0.7], gap="xxsmall")

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

        cfg_path = Path(cfg.data.storage_path) / "experiments" / f"{exp_id}.toml"
        export_disabled = not cfg_path.is_file()
        if col3.button(
            "",
            key=f"export_{exp_id}",
            icon=":material/upload:",
            type="secondary",
            width="stretch",
            disabled=export_disabled,
            help=(
                "Open this experiment's configuration in the **Experiment** page."
                if not export_disabled
                else "No saved configuration found for this experiment."
            ),
        ):
            try:
                exp_cfg = ExperimentConfig.from_toml(cfg_path.read_text(encoding="utf-8"))
            except Exception as ex:  # noqa: BLE001
                st.session_state["_delete_error"] = f"Failed to load configuration: {ex}"
                st.rerun()
            else:
                st.session_state["_pending_experiment_config"] = exp_cfg
                st.switch_page("experiment.py")

        if col4.button(
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
            "<span style='display:inline-block;opacity:0.7;font-size:1.05em;margin-top:-20px'>"
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

            for i, run in enumerate(query_experiment_strategies(exp_id)):
                if i > 0:
                    st.markdown("<div style='margin-top:1.25rem'></div>", unsafe_allow_html=True)
                st.markdown(f"**:material/psychology: {run.strategy_name}**")
                _render_strategy_summary(run)
