"""Backtide.

Author: Mavs
Description: Backtest results page.

"""

from datetime import datetime as dt
from pathlib import Path
from typing import TYPE_CHECKING

import pandas as pd
import streamlit as st

from backtide.analysis import (
    plot_cash_holdings,
    plot_mae_mfe,
    plot_pnl,
    plot_pnl_histogram,
    plot_position_size,
    plot_price,
    plot_rolling_returns,
    plot_rolling_sharpe,
    plot_trade_duration,
    plot_trade_pnl,
)
from backtide.analysis.utils import GREEN, RED, YELLOW, _is_benchmark
from backtide.backtest import ExperimentConfig
from backtide.config import get_config
from backtide.storage import (
    delete_experiment,
    query_bars,
    query_experiments,
    query_instruments,
    query_strategy_runs,
)
from backtide.ui.utils import (
    _default,
    _fmt_duration,
    _fmt_metric,
    _fmt_period,
    _get_logokit_url,
    _persist,
    _to_pandas,
)
from backtide.utils.constants import BENCHMARK_NAME
from backtide.utils.utils import _format_price, _moment_to_strftime

if TYPE_CHECKING:
    from backtide.backtest import RunResult


cfg = get_config()
datetime_fmt = _moment_to_strftime(cfg.display.datetime_format())

st.set_page_config(page_title="Backtide - Results")

st.markdown(
    f"""
    <style>
        .tag-pill {{
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
        }}

        /* Compact metrics */
        [data-testid="stMetricLabel"] {{
            font-size: 0.82em;
        }}
        [data-testid="stMetricValue"] {{
            font-size: 1.3em;
        }}

        div[data-testid="stPopoverBody"]:has(.wide-marker) {{
            width: 50vw !important;
            max-width: 50vw !important;
        }}

        .status-badge {{
            display: inline-block;
            width: 12px;
            height: 12px;
            border-radius: 50%;
            vertical-align: baseline;
            position: relative;
            top: -0.1em;
            margin-left: 4px;
        }}
        .status-badge.success {{ background: {GREEN}; }}
        .status-badge.warning {{ background: {YELLOW}; }}
        .status-badge.error   {{ background: {RED}; }}

        /* Tighten dividers inside cards */
        hr {{
            margin: 0.25rem 0 0.85rem 0 !important;
        }}

        /* Full-results page experiment title */
        .experiment-title {{
            margin-top: 20px;
            text-align: center;
            font-size: 2.2em;
            font-weight: 700;
        }}

        /* Center the strategy segmented-control on the full-results page.
           Aggressive: shrink every nested wrapper to its content width and
           auto-margin it, so the button row ends up centered no matter
           what BaseWeb/Streamlit re-renders the control as. */
        .st-key-strategy_picker,
        .st-key-strategy_picker [data-testid="stVerticalBlock"] {{
            align-items: center !important;
        }}
        .st-key-strategy_picker [data-testid="stElementContainer"],
        .st-key-strategy_picker [data-testid="stSegmentedControl"],
        .st-key-strategy_picker [data-baseweb="button-group"] {{
            width: fit-content !important;
            max-width: 100% !important;
            margin-left: auto !important;
            margin-right: auto !important;
            flex: 0 0 auto !important;
        }}
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


def _render_run_metrics(run: RunResult):
    """Render compact summary metrics for a single strategy run."""
    if err := getattr(run, "error", None):
        st.error(
            f"**{run.strategy_name}** failed during execution:\n\n```\n{err}\n```",
            icon=":material/error:",
        )
        if not run.equity_curve and not run.trades:
            return

    mc1, mc2, mc3, mc4, mc5, mc6, mc7 = st.columns([0.8, 0.9, 0.9, 0.9, 0.9, 0.9, 1.2])

    sharpe = run.metrics["sharpe"]
    pnl = run.metrics["pnl"]
    total_return = run.metrics["total_return"]
    cagr = run.metrics["cagr"]
    alpha = run.metrics.get("alpha")
    max_dd = run.metrics["max_dd"]
    n_trades = int(run.metrics["n_trades"])
    win_rate = run.metrics["win_rate"]

    _colored_metric(
        mc1,
        ":material/military_tech: Sharpe",
        _fmt_metric(sharpe),
        _tone(sharpe, good_above=1.0, bad_below=0.0),
    )
    _colored_metric(
        mc2,
        ":material/payments: PnL",
        f"{'+' if pnl > 0 else ''}{_format_price(pnl, currency=run.base_currency, compact=True)}",
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
        "--" if _is_benchmark(run) or alpha is None else _fmt_pct(alpha, signed=True),
        _tone(alpha),
    )
    _colored_metric(
        mc6,
        ":material/trending_down: Max DD",
        _fmt_pct(max_dd),
        "red" if max_dd and not pd.isna(max_dd) else "",  # Any non-zero drawdown is bad.
    )

    wr_pct = _fmt_metric(win_rate * 100, suffix="%")
    if win_rate > 0.5:
        wr_str = f":color[{wr_pct}]{{foreground='{GREEN}'}}"
    elif win_rate < 0.5:
        wr_str = f":color[{wr_pct}]{{foreground='{RED}'}}"
    else:
        wr_str = wr_pct
    _colored_metric(mc7, ":material/swap_vert: Trades (w/r)", f"{n_trades} ({wr_str})")


def _status_icon(row: pd.Series) -> str:
    """Return a Material icon shortcode based on the experiment's Sharpe ratio."""
    status = row.get("status")
    if status == "failed":
        return ":material/error:"
    if status == "partial":
        return ":material/warning:"

    sharpe = row.get("best_sharpe")
    if sharpe is None or pd.isna(sharpe):
        return ":material/help:"
    elif sharpe < 0.0:
        return ":material/sentiment_very_dissatisfied:"
    elif sharpe < 0.5:
        return ":material/sentiment_dissatisfied:"
    elif sharpe < 1.0:
        return ":material/sentiment_neutral:"
    elif sharpe < 1.5:
        return ":material/sentiment_satisfied:"
    elif sharpe < 2.0:
        return ":material/sentiment_very_satisfied:"
    elif sharpe < 3.0:
        return ":material/military_tech:"
    else:
        return ":material/trophy:"


def _status_badge(row: pd.Series) -> str:
    """Return a small colored status dot for the experiment."""
    status = row.get("status")
    if status == "failed":
        cls, label = "error", "Failed"
    elif status == "partial":
        cls, label = "warning", "Some strategies failed"
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
            st.session_state["_error"] = f"Failed to delete experiment: {ex}."
        else:
            if st.session_state.get("results_expanded") == exp_id:
                st.session_state.pop("results_expanded", None)
            if (sel := st.session_state.get("selected_experiment")) and sel.get("id") == exp_id:
                st.session_state.pop("selected_experiment", None)
            st.session_state["_success"] = name
        st.rerun()


def _render_analysis_tabs(runs: list[RunResult], exp_cfg: ExperimentConfig):
    """Render the experiment-level (multi-run) analysis-plot tabs.

    Only plots that overlay every strategy on the same axes are rendered
    here. Single-run plots (MAE/MFE, Position size, Price) live inside
    each per-strategy tab so they can use the strategy as their context
    rather than an extra widget. The PnL tab is the first/default tab;
    the rest follow alphabetically.

    """
    if not runs:
        return

    all_labels = [
        ":material/payments: PnL",
        ":material/account_balance_wallet: Cash",
        ":material/bar_chart: PnL histogram",
        ":material/stacked_line_chart: Rolling returns",
        ":material/military_tech: Rolling Sharpe",
        ":material/timer: Trade duration",
        ":material/swap_vert: Trade PnL",
    ]
    tabs = st.tabs(
        all_labels,
        key=(key := "plot_tabs_results"),
        default=_default(key),
        on_change=lambda k=key: _persist(k),
    )

    # Build a lookup from label → tab widget for safe indexing
    tab_map = dict(zip(all_labels, tabs, strict=True))

    # Determine active tab index for lazy rendering
    active_tab = st.session_state.get(key, all_labels[0])

    # Currency labels are now resolved by each plot from the runs' own
    # `base_currency` attribute (set by the engine), so we no longer need
    # to pass `currency=...` here.

    with tab_map[all_labels[0]]:
        if active_tab == all_labels[0]:
            col1, col2 = st.columns([10, 1])
            col1.caption("Cumulative profit and loss over time for each strategy.")
            with col2.popover(":material/tune:", help="PnL chart options"):
                normalize = st.toggle(
                    "Normalize",
                    key=(key := "results_pnl_normalize"),
                    value=_default(key, fallback=False),
                    on_change=lambda k=key: _persist(k),
                    help="Show PnL and drawdown in percentage terms.",
                )
                drawdown = st.toggle(
                    "Show drawdown",
                    key=(key := "results_pnl_drawdown"),
                    value=_default(key, fallback=True),
                    on_change=lambda k=key: _persist(k),
                    help="Show a second panel with strategy drawdown.",
                )

            with st.spinner("Loading plot..."):
                st.plotly_chart(
                    plot_pnl(runs, normalize=normalize, drawdown=drawdown, display=None),
                    width="stretch",
                )

    with tab_map[all_labels[1]]:
        if active_tab == all_labels[1]:
            col1, col2 = st.columns([10, 1])
            col1.caption("Cash balance timeline split by strategy and settlement currency.")
            with col2.popover(":material/tune:"):
                st.caption("No options available for this plot.")

            with st.spinner("Loading plot..."):
                st.plotly_chart(plot_cash_holdings(runs, display=None), width="stretch")

    with tab_map[all_labels[2]]:
        if active_tab == all_labels[2]:
            col1, col2 = st.columns([10, 1])
            col1.caption("Distribution of realized trade PnL across strategies.")
            with col2.popover(":material/tune:", help="PnL histogram options"):
                set_bins = st.toggle(
                    "Set bins",
                    key=(key := "results_pnl_histogram_set_bins"),
                    value=_default(key, fallback=False),
                    on_change=lambda k=key: _persist(k),
                    help="Enable a custom number of histogram bins.",
                )
                if set_bins:
                    bins = st.slider(
                        "Bins",
                        min_value=5,
                        max_value=100,
                        step=1,
                        key=(key := "results_pnl_histogram_bins"),
                        value=_default(key, fallback=40),
                        on_change=lambda k=key: _persist(k),
                        help="Set the number of histogram bins.",
                    )
                else:
                    bins = None

            with st.spinner("Loading plot..."):
                st.plotly_chart(plot_pnl_histogram(runs, bins=bins, display=None), width="stretch")

    with tab_map[all_labels[3]]:
        if active_tab == all_labels[3]:
            col1, col2 = st.columns([10, 1])
            col1.caption("Rolling return trend to compare momentum over time.")
            with col2.popover(":material/tune:", help="Rolling returns options"):
                window = st.slider(
                    "Window",
                    min_value=2,
                    max_value=365,
                    step=1,
                    key=(key := "results_rolling_returns_window"),
                    value=_default(key, fallback=30),
                    on_change=lambda k=key: _persist(k),
                    help="Number of bars used for the rolling return window.",
                )

            with st.spinner("Loading plot..."):
                st.plotly_chart(plot_rolling_returns(runs, window, display=None), width="stretch")

    with tab_map[all_labels[4]]:
        if active_tab == all_labels[4]:
            col1, col2 = st.columns([10, 1])
            col1.caption("Rolling Sharpe ratio showing risk-adjusted performance.")
            with col2.popover(":material/tune:", help="Rolling Sharpe options"):
                window = st.slider(
                    "Window",
                    min_value=2,
                    max_value=365,
                    step=1,
                    key=(key := "results_rolling_returns_window"),
                    value=_default(key, fallback=60),
                    on_change=lambda k=key: _persist(k),
                    help="Number of bars used for the rolling return window.",
                )

            # Number of bars per year for the experiment's interval
            ppy = int(365 * 24 * 60 / exp_cfg.data.interval.minutes())

            with st.spinner("Loading plot..."):
                st.plotly_chart(
                    plot_rolling_sharpe(runs, window, ppy, display=None),
                    width="stretch",
                )

    with tab_map[all_labels[5]]:
        if active_tab == all_labels[5]:
            col1, col2 = st.columns([10, 1])
            col1.caption("Distribution of trade holding periods.")
            with col2.popover(":material/tune:"):
                unit = st.pills(
                    "Unit",
                    key=(key := "results_trade_duration_unit"),
                    required=True,
                    options=["auto", "minutes", "hours", "days"],
                    default=_default(key, fallback="auto"),
                    on_change=lambda k=key: _persist(k),
                    help="Time unit used on the x-axis.",
                )
                set_bins = st.toggle(
                    "Set bins",
                    key=(key := "results_trade_duration_set_bins"),
                    value=_default(key, fallback=False),
                    on_change=lambda k=key: _persist(k),
                    help="Enable a custom number of histogram bins.",
                )
                if set_bins:
                    bins = st.slider(
                        "Bins",
                        min_value=5,
                        max_value=100,
                        step=1,
                        key=(key := "results_trade_duration_bins"),
                        value=_default(key, fallback=40),
                        on_change=lambda k=key: _persist(k),
                        help="Set the number of histogram bins.",
                    )
                else:
                    bins = None

            with st.spinner("Loading plot..."):
                st.plotly_chart(
                    plot_trade_duration(runs, bins=bins, unit=unit or "auto", display=None),
                    width="stretch",
                )

    with tab_map[all_labels[6]]:
        if active_tab == all_labels[6]:
            col1, col2 = st.columns([10, 1])
            col1.caption("Per-trade PnL profile for each strategy.")
            with col2.popover(":material/tune:"):
                st.caption("No options available for this plot.")

            with st.spinner("Loading plot..."):
                st.plotly_chart(plot_trade_pnl(runs, display=None), width="stretch")


def _render_strategy_plots(run: RunResult, exp_cfg: ExperimentConfig):
    """Render per-strategy plots."""
    interval = str(exp_cfg.data.interval)

    labels = [
        ":material/compare_arrows: MAE / MFE",
        ":material/inventory: Position size",
        ":material/show_chart: Price",
    ]

    # Use a per-strategy key so each tab group remembers its active selection.
    tabs = st.tabs(
        labels,
        key=(key := f"plot_tabs_strategy_{run.strategy_name}"),
        default=_default(key),
        on_change=lambda k=key: _persist(k),
    )

    label_to_tab = dict(zip(labels, tabs, strict=True))
    active_tab = st.session_state.get(key, labels[0])

    with label_to_tab[labels[0]]:
        if active_tab == labels[0]:
            c1, c2 = st.columns([10, 1])
            c1.caption("Maximum adverse/favorable excursion per trade.")
            with c2.popover(":material/tune:"):
                traded = sorted({t.symbol for t in run.trades})
                mae_mfe_symbols = st.multiselect(
                    label="Symbols",
                    key=(key := f"mae_mfe_symbols_{run.strategy_name}"),
                    options=traded,
                    default=traded,
                    placeholder="Choose symbols...",
                    on_change=lambda k=key: _persist(k),
                    help="Select which symbols to show in the plot.",
                )
                mae_mfe_symbols = mae_mfe_symbols or traded

            with st.spinner("Loading plot..."):
                st.plotly_chart(
                    plot_mae_mfe(run, interval=interval, symbols=mae_mfe_symbols, display=None),
                    width="stretch",
                )

    with label_to_tab[labels[1]]:
        if active_tab == labels[1]:
            c1, c2 = st.columns([10, 1])
            c1.caption("Position size evolution through time.")
            with c2.popover(":material/tune:"):
                options = sorted({o.order.symbol for o in run.orders if o.status == "filled"})
                symbols = st.multiselect(
                    label="Symbols",
                    key=(key := f"position_size_symbols_{run.strategy_name}"),
                    options=options,
                    default=options,
                    placeholder="Choose symbols...",
                    on_change=lambda k=key: _persist(k),
                    help="Select which symbols to show in the plot.",
                )

            with st.spinner("Loading plot..."):
                st.plotly_chart(
                    plot_position_size(run, symbols=symbols or options, display=None),
                    width="stretch",
                )

    with label_to_tab[labels[2]]:
        if active_tab == labels[2]:
            c1, c2 = st.columns([10, 1])
            c1.caption("Price action with strategy context for the selected symbol.")

            if not (traded := sorted({t.symbol for t in run.trades})):
                traded = exp_cfg.data.symbols
            if not traded:
                st.info("No symbols available for this run.")
            else:
                with c2.popover(":material/tune:"):
                    symbol = st.selectbox(
                        label="Symbol",
                        key=(key := f"price_sym_{run.strategy_name}"),
                        options=traded,
                        on_change=lambda k=key: _persist(k),
                    )

                if len(df := query_bars(symbol=symbol, interval=interval)) == 0:
                    st.info(f"No price data available for **{symbol}**.")
                else:
                    with st.spinner("Loading plot..."):
                        st.plotly_chart(
                            plot_price(df, run=run, display=None),
                            width="stretch",
                        )


def _render_full_analysis(row: pd.Series):
    """Render plots and tables for a single experiment.

    Parameters
    ----------
    row : pd.Series
        The experiment row previously retrieved from `query_experiments`.

    """
    exp_id = row["id"]
    name = row["name"] or exp_id
    cfg_path = Path(cfg.data.storage_path) / "experiments" / exp_id / "config.toml"
    log_path = Path(cfg.data.storage_path) / "experiments" / exp_id / "logs.txt"

    try:
        exp_cfg = ExperimentConfig.from_toml(cfg_path.read_text(encoding="utf-8"))
    except Exception as ex:  # noqa: BLE001
        st.session_state["_error"] = f"Failed to load configuration: {ex}"
        st.rerun()

    with st.container(key="full_results_toolbar"):
        col1, col2, _, col4, col5, col6 = st.columns([1.2, 0.7, 4.6, 0.7, 0.7, 0.7], gap="xxsmall")

        if col1.button(
            ":material/arrow_back: Back",
            key="back_to_results",
            width="stretch",
        ):
            st.session_state.pop("selected_experiment", None)
            st.rerun()

        with col2.popover(
            "",
            icon=":material/article:",
            disabled=not log_path.is_file(),
            help=(
                "Show the engine logs captured for this experiment."
                if log_path.is_file()
                else "No log file found for this experiment."
            ),
        ):
            st.html("<span class='wide-marker' style='display:none'></span>")
            try:
                log_text = log_path.read_text(encoding="utf-8", errors="replace")
            except OSError as ex:
                st.markdown(f"**Logs — {name}**")
                st.error(f"Failed to read log file: {ex}")
            else:
                col1, col2 = st.columns([2.4, 1])
                col1.markdown(f"**Logs — {name}**")
                col2.download_button(
                    "Download",
                    data=log_text,
                    file_name=f"{name}-logs.txt",
                    mime="text/plain",
                    icon=":material/download:",
                    key=f"logs_{exp_id}",
                    width="stretch",
                )
                if log_text.strip():
                    st.code(log_text, language="log", line_numbers=True)
                else:
                    st.info("Log file is empty.")

        with col4.popover(
            "",
            icon=":material/description:",
            disabled=not row["description"],
            help=(
                "Show the experiment description."
                if row["description"]
                else "No description for this experiment."
            ),
        ):
            st.markdown(f"**{name}**")
            st.write(row["description"])

        if col5.button(
            "",
            key=f"export_full_{exp_id}",
            icon=":material/upload:",
            type="secondary",
            width="stretch",
            disabled=not cfg_path.is_file(),
            help=(
                "Open this experiment's configuration in the **Experiment** page."
                if cfg_path.is_file()
                else "No saved configuration found for this experiment."
            ),
        ):
            st.session_state["_pending_experiment_config"] = exp_cfg
            st.switch_page("experiment.py")

        if col6.button(
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

    c1, c2, c3, c4 = st.columns([0.8, 1.2, 1, 1])

    sharpe = row.get("best_sharpe")
    _colored_metric(
        c1,
        ":material/military_tech: Best Sharpe",
        _fmt_metric(sharpe),
        _tone(sharpe, good_above=1.0, bad_below=0.0),
    )

    _colored_metric(c2, ":material/event: Started at", _fmt_ts(row["started_at"]))
    duration = max(0, int(row["finished_at"]) - int(row["started_at"]))
    _colored_metric(c3, ":material/timer: Duration", _fmt_duration(duration))

    status = str(row.get("status") or "").lower()
    if status == "completed":
        icon, label, tone = ":material/check_circle:", "Succeeded", "green"
    elif status == "partial":
        icon, label, tone = ":material/warning:", "Partial", "orange"
    else:
        icon, label, tone = ":material/cancel:", "Failed", "red"
    _colored_metric(c4, f"{icon} Status", label, tone)

    runs = query_strategy_runs(row["id"])

    if failed_runs := [r for r in runs if getattr(r, "error", None)]:
        names = ", ".join(f"**{r.strategy_name}**" for r in failed_runs)
        if len(failed_runs) == len(runs):
            st.error(
                f"All {len(runs)} strategies failed during execution: {names}. "
                "See the per-strategy tabs below (or the logs popover) for the raised errors.",
                icon=":material/error:",
            )
        else:
            st.warning(
                f"{len(failed_runs)} of {len(runs)} strategies failed during execution: "
                f"{names}. The remaining strategies completed successfully.",
                icon=":material/warning:",
            )

    start_ts = None
    end_ts = None
    for r in runs:
        if not r.equity_curve:
            continue

        first = int(r.equity_curve[0].timestamp)
        last = int(r.equity_curve[-1].timestamp)
        start_ts = first if start_ts is None else min(start_ts, first)
        end_ts = last if end_ts is None else max(end_ts, last)

    length_str = "?"
    date_fmt = _moment_to_strftime(cfg.display.date_format)
    if start_ts is not None and end_ts is not None:
        d0 = dt.fromtimestamp(start_ts).date()
        d1 = dt.fromtimestamp(end_ts).date()
        period_str = f"{d0.strftime(date_fmt)} → {d1.strftime(date_fmt)}"
        length_str = _fmt_period(d0, d1)
    else:
        # Fall back to the requested config range when no equity curves are
        # available (e.g., a failed experiment with no completed runs).
        start = str(exp_cfg.data.start_date) if exp_cfg.data.start_date else None
        end = str(exp_cfg.data.end_date) if exp_cfg.data.end_date else None
        if start and end:
            try:
                d0 = dt.fromisoformat(start).date()
                d1 = dt.fromisoformat(end).date()
            except ValueError:
                period_str = f"{start} → {end}"
            else:
                period_str = f"{d0.strftime(date_fmt)} → {d1.strftime(date_fmt)}"
                length_str = _fmt_period(d0, d1)
        elif start:
            period_str = f"from {start}"
        elif end:
            period_str = f"until {end}"
        else:
            period_str = "Full history"

    n_symbols = len(exp_cfg.data.symbols)
    interval_str = str(exp_cfg.data.interval)

    c1, c2, c3 = st.columns([0.8, 2.2, 1])
    _colored_metric(c1, ":material/finance: Symbols", str(n_symbols))
    _colored_metric(c2, ":material/date_range: Period", f"{period_str} ({length_str})")
    _colored_metric(c3, ":material/schedule: Interval", interval_str)

    st.markdown("")

    _render_analysis_tabs(runs, exp_cfg)

    st.markdown("")
    st.markdown("")

    st.markdown("### Strategies", text_alignment="center")

    with st.container(key="strategy_picker"):
        selected_strategy = st.segmented_control(
            label="Strategies",
            label_visibility="collapsed",
            key=(key := f"selected_strategy_{exp_id}"),
            required=True,
            options=(options := [run.strategy_name for run in runs]),
            default=_default(key, next(r for r in options if r != BENCHMARK_NAME)),
            format_func=lambda x: (
                f":material/{'bar_chart' if x == BENCHMARK_NAME else 'psychology'}: **{x}**"
            ),
            on_change=lambda k=key: _persist(k),
            help="Select the strategy to analyze.",
        )

    logokit_key = cfg.display.logokit_api_key
    run = next(r for r in runs if r.strategy_name == selected_strategy)

    st.markdown("##### Metrics")
    _render_run_metrics(run)
    st.markdown("")

    st.markdown("##### Plots")
    _render_strategy_plots(run, exp_cfg)
    st.markdown("")

    st.markdown("##### Orders")

    if not run.orders:
        st.warning("The strategy didn't execute any orders.", icon=":material/warning:")
        st.stop()

    # Map every traded symbol to the currency it actually settled in.
    symbol_to_ccy, symbol_to_it = {}, {}
    for inst in query_instruments():
        if inst.quote:
            symbol_to_ccy[inst.symbol] = str(inst.quote)
        symbol_to_it[inst.symbol] = inst.instrument_type

    rows = []
    for o in run.orders:
        qty = o.order.quantity
        side = "Buy" if qty > 0 else ("Sell" if qty < 0 else "—")
        px = o.fill_price if o.fill_price is not None else o.order.price

        # Settle each fill in the instrument's quote currency
        # (matches what the engine actually debited / credited).
        quote_ccy = symbol_to_ccy.get(o.order.symbol, run.base_currency)

        total = (px * abs(qty)) if px is not None else None

        rows.append(
            {
                "Datetime": dt.fromtimestamp(o.timestamp),
                "Symbol": o.order.symbol,
                "Type": str(o.order.order_type),
                "Side": side,
                "Qty": abs(qty),
                "Price": _format_price(total, currency=quote_ccy) if total is not None else "—",
                "PnL": _format_price(o.pnl, currency=quote_ccy) if o.pnl is not None else "—",
                "Commission": _format_price(o.commission or 0.0, currency=quote_ccy),
                "Status": o.status,
            }
        )

    df = pd.DataFrame(rows).sort_values("Datetime", ascending=False).reset_index(drop=True)

    if logokit_key:
        df.insert(
            0,
            "Logo",
            df.apply(
                lambda row: _get_logokit_url(
                    row["Symbol"], symbol_to_it[row["Symbol"]], logokit_key
                ),
                axis=1,
            ),
        )

    column_config = {}
    if logokit_key:
        column_config["Logo"] = st.column_config.ImageColumn(label="", width="small")

    def _color_side(val: str) -> str:
        if val == "Buy":
            return f"color: {GREEN}; font-weight: 600;"
        if val == "Sell":
            return f"color: {RED}; font-weight: 600;"
        return ""

    def _color_pnl(val: str | None) -> str:
        if not val:
            return ""
        s = val.replace(",", "")
        if not any(ch.isdigit() for ch in s):
            return ""
        return f"color: {RED};" if "-" in s else f"color: {GREEN};"

    st.dataframe(
        df.style.map(_color_side, subset=["Side"]).map(_color_pnl, subset=["PnL"]),
        width="stretch",
        column_order=["", *df.columns[:-1]],
        column_config=column_config,
        hide_index=True,
    )


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

if ok := st.session_state.pop("_success", None):
    st.success(f"Deleted experiment **{ok}**.", icon=":material/check_circle:")
if err := st.session_state.pop("_error", None):
    st.error(err, icon=":material/error:")

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

        cfg_path = Path(cfg.data.storage_path) / "experiments" / exp_id / "config.toml"
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
                st.session_state["_error"] = f"Failed to load configuration: {ex}"
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
        best = row["best_sharpe"]
        best_str = f"{best:.2f}" if best is not None and not pd.isna(best) else "—"
        col1.markdown(
            "<span style='display:inline-block;opacity:0.7;font-size:1.05em;margin-top:-20px'>"
            f":material/calendar_month: {_fmt_ts(row['started_at'])} "
            f"&nbsp;·&nbsp; "
            f":material/military_tech: {best_str} "
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

            for i, run in enumerate(query_strategy_runs(exp_id)):
                if i > 0:
                    st.markdown("<div style='margin-top:1.25rem'></div>", unsafe_allow_html=True)
                icon = "bar_chart" if _is_benchmark(run) else "psychology"
                st.markdown(f"**:material/{icon}: {run.strategy_name}**")
                _render_run_metrics(run)
