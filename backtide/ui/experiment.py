"""Backtide.

Author: Mavs
Description: Run a new backtest page.

"""

import ast
from datetime import datetime as dt
from datetime import timedelta
import json
import logging
import tomllib
import uuid

from code_editor import code_editor
import streamlit as st
import yaml

from backtide.backtest import (
    CodeSnippet,
    CommissionType,
    ConversionPeriod,
    CurrencyConversionMode,
    DataExpConfig,
    EmptyBarPolicy,
    EngineExpConfig,
    ExchangeExpConfig,
    ExperimentConfig,
    GeneralExpConfig,
    IndicatorExpConfig,
    IndicatorType,
    OrderType,
    PortfolioExpConfig,
    StrategyExpConfig,
    StrategyType,
)
from backtide.config import get_config
from backtide.core.data import resolve_profiles
from backtide.data import Currency, InstrumentProfile, InstrumentType, Interval
from backtide.storage import query_instruments
from backtide.ui.utils import (
    _CARD_CSS,
    _clear_state,
    _default,
    _draw_cards,
    _get_instrument_type_description,
    _get_timezone,
    _list_instruments,
    _persist,
    _query_bars_summary,
    _to_upper_values,
)
from backtide.utils.constants import (
    INDICATOR_PLACEHOLDER,
    INVALID_FILENAME_CHARS,
    MAX_INSTRUMENT_SELECTION,
    STRATEGY_PLACEHOLDER,
    TAG_PATTERN,
)

# Disable streamlit warnings spawned by the thread running _build_experiment_config
logging.getLogger("streamlit.runtime.scriptrunner_utils.script_run_context").setLevel(
    logging.ERROR
)


# ─────────────────────────────────────────────────────────────────────────────
# Helper functions
# ─────────────────────────────────────────────────────────────────────────────

USER_CODE_OPTIONS = [":material/code: Code editor", ":material/upload_file: Upload file"]


def _build_experiment_config() -> str:
    """Return the current experiment configuration in as a toml string."""
    ss = st.session_state

    cfg = ExperimentConfig(
        general=GeneralExpConfig(
            name=experiment_name or ss.experiment_id,
            tags=ss.get("tags", []),
            description=ss.get("description", ""),
        ),
        data=DataExpConfig(
            instrument_type=ss.get("instrument_type", "stocks"),
            symbols=[s.symbol if hasattr(s, "symbol") else str(s) for s in ss.get("symbols", [])],
            full_history=ss.get("full_history", True),
            start_date=str(ss.get("start_date")) if ss.get("start_date") else None,
            end_date=str(ss.get("end_date")) if ss.get("end_date") else None,
            interval=ss.get("interval", "1d"),
        ),
        portfolio=PortfolioExpConfig(
            initial_cash=float(ss.get("initial_cash", 10_000)),
            base_currency=ss.get("base_currency", "USD"),
            starting_positions=ss.get("starting_positions", []),
        ),
        strategy=StrategyExpConfig(
            predefined_strategies=list(ss.get("predefined_strategies", [])),
            custom_strategies=[
                CodeSnippet(
                    name=ss.get(f"strategy_name_{i}", f"Strategy {i + 1}"),
                    code=e.get("code", ""),
                )
                for i, e in enumerate(ss.get("custom_strategies", []))
            ],
        ),
        indicators=IndicatorExpConfig(
            builtin_indicators=list(ss.get("builtin_indicators", [])),
            custom_indicators=[
                CodeSnippet(
                    name=ss.get(f"indicator_name_{i}", f"Indicator {i + 1}"),
                    code=e.get("code", ""),
                )
                for i, e in enumerate(ss.get("custom_indicators", []))
            ],
        ),
        exchange=ExchangeExpConfig(
            commission_type=ss.get("commission_type", "Percentage"),
            commission_pct=float(ss.get("commission_pct", 0.1)),
            commission_fixed=float(ss.get("commission_fixed", 0.0)),
            slippage=float(ss.get("slippage", 0.05)),
            allowed_order_types=list(ss.get("allowed_order_types", ["Market"])),
            partial_fills=ss.get("partial_fills", False),
            allow_margin=ss.get("allow_margin", True),
            max_leverage=float(ss.get("max_leverage", 1.0)),
            initial_margin=float(ss.get("initial_margin", 50.0)),
            maintenance_margin=float(ss.get("maintenance_margin", 25.0)),
            margin_interest=float(ss.get("margin_interest", 0.0)),
            allow_short_selling=ss.get("allow_short_selling", True),
            borrow_rate=float(ss.get("borrow_rate", 0.0)),
            max_position_size=int(ss.get("max_position_size", 100)),
            conversion_mode=ss.get("conversion_mode", "Immediate"),
            conversion_threshold=(
                float(ss["conversion_threshold"])
                if ss.get("conversion_threshold") is not None
                else None
            ),
            conversion_period=(
                ss["conversion_period"] if ss.get("conversion_period") is not None else None
            ),
            conversion_interval=(
                int(ss["conversion_interval"])
                if ss.get("conversion_interval") is not None
                else None
            ),
        ),
        engine=EngineExpConfig(
            warmup_period=int(ss.get("warmup_period", 0)),
            trade_on_close=ss.get("trade_on_close", False),
            risk_free_rate=float(ss.get("risk_free_rate", 0.0)),
            benchmark=ss.get("benchmark", ""),
            exclusive_orders=ss.get("exclusive_orders", False),
            random_seed=int(ss["random_seed"]) if ss.get("random_seed") is not None else None,
            empty_bar_policy=ss.get("empty_bar_policy", "ForwardFill"),
        ),
    )

    return cfg.to_toml()


def _on_config_upload():
    """Set the experiment config based on an uploaded file.

    Because callbacks execute before any widget is instantiated, we can
    freely set session-state keys that are bound to widgets.

    """
    upload = st.session_state.get("config_upload")

    if upload is None:
        # File was cleared by the user.
        return

    try:
        if upload.name.endswith(".json"):
            raw = json.load(upload)
        elif upload.name.endswith(".toml"):
            raw = tomllib.loads(upload.read().decode("utf-8"))
        else:
            raw = yaml.safe_load(upload)

        imported = ExperimentConfig.from_dict(raw)

        # ── General ──────────────────────────────────────────────────────────

        st.session_state["experiment_name"] = INVALID_FILENAME_CHARS.sub("", imported.general.name)
        st.session_state["tags"] = list(imported.general.tags)
        st.session_state["description"] = imported.general.description

        # ── Data ─────────────────────────────────────────────────────────────

        st.session_state["instrument_type"] = imported.data.instrument_type
        st.session_state["symbols"] = list(imported.data.symbols)
        st.session_state["full_history"] = imported.data.full_history
        if not imported.data.full_history:
            if imported.data.start_date:
                st.session_state["start_date"] = dt.fromisoformat(
                    str(imported.data.start_date)
                ).date()
            if imported.data.end_date:
                st.session_state["end_date"] = dt.fromisoformat(str(imported.data.end_date)).date()
        st.session_state["interval"] = imported.data.interval

        # ── Portfolio ────────────────────────────────────────────────────────

        st.session_state["initial_cash"] = int(imported.portfolio.initial_cash)
        st.session_state["base_currency"] = imported.portfolio.base_currency
        st.session_state["positions"] = imported.portfolio.positions

        # ── Strategy ─────────────────────────────────────────────────────────

        st.session_state["predefined_strategies"] = list(imported.strategy.predefined_strategies)
        st.session_state["custom_strategies"] = [
            {"source": USER_CODE_OPTIONS[0], "code": s.code}
            for s in imported.strategy.custom_strategies
        ]
        for i, s in enumerate(imported.strategy.custom_strategies):
            st.session_state[f"strategy_name_{i}"] = s.name

        # ── Indicators ───────────────────────────────────────────────────────

        st.session_state["builtin_indicators"] = list(imported.indicators.builtin_indicators)
        st.session_state["custom_indicators"] = [
            {"source": USER_CODE_OPTIONS[0], "code": s.code}
            for s in imported.indicators.custom_indicators
        ]
        for i, s in enumerate(imported.indicators.custom_indicators):
            st.session_state[f"indicator_name_{i}"] = s.name

        # ── Exchange ─────────────────────────────────────────────────────────

        ex = imported.exchange
        st.session_state["commission_type"] = ex.commission_type
        st.session_state["commission_pct"] = ex.commission_pct
        st.session_state["commission_fixed"] = ex.commission_fixed
        st.session_state["slippage"] = ex.slippage
        st.session_state["allowed_order_types"] = list(ex.allowed_order_types)
        st.session_state["partial_fills"] = ex.partial_fills
        st.session_state["allow_margin"] = ex.allow_margin
        st.session_state["max_leverage"] = ex.max_leverage
        st.session_state["initial_margin"] = ex.initial_margin
        st.session_state["maintenance_margin"] = ex.maintenance_margin
        st.session_state["margin_interest"] = ex.margin_interest
        st.session_state["allow_short_selling"] = ex.allow_short_selling
        st.session_state["borrow_rate"] = ex.borrow_rate
        st.session_state["max_position_size"] = int(ex.max_position_size)
        st.session_state["conversion_mode"] = ex.conversion_mode
        if ex.conversion_threshold is not None:
            st.session_state["conversion_threshold"] = ex.conversion_threshold
        if ex.conversion_period is not None:
            st.session_state["conversion_period"] = ex.conversion_period
        if ex.conversion_interval is not None:
            st.session_state["conversion_interval"] = int(ex.conversion_interval)

        # ── Engine ───────────────────────────────────────────────────────────

        eng = imported.engine
        st.session_state["warmup_period"] = int(eng.warmup_period)
        st.session_state["trade_on_close"] = eng.trade_on_close
        st.session_state["risk_free_rate"] = eng.risk_free_rate
        st.session_state["benchmark"] = eng.benchmark
        st.session_state["exclusive_orders"] = eng.exclusive_orders
        st.session_state["random_seed"] = (
            int(eng.random_seed) if eng.random_seed is not None else None
        )
        st.session_state["empty_bar_policy"] = eng.empty_bar_policy

        st.session_state["_import_success"] = f"Loaded configuration from `{upload.name}`."
    except Exception as ex:  # noqa: BLE001
        st.session_state["_import_error"] = f"Failed to parse file: {ex}"


# ─────────────────────────────────────────────────────────────────────────────
# Experiment interface
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
tz = _get_timezone(cfg.display.timezone)

st.set_page_config(page_title="Backtide - Experiment", layout="centered")
st.title("Experiment", text_alignment="center")

if st.session_state.get("current_tab"):
    st.session_state.tabs = st.session_state.current_tab

tab1, tab2, tab3, tab4, tab5, tab6, tab7 = st.tabs(
    [
        ":material/dashboard: General",
        ":material/analytics: Data",
        ":material/account_balance_wallet: Portfolio",
        ":material/psychology: Strategy",
        ":material/show_chart: Indicators",
        ":material/storefront: Exchange",
        ":material/build: Engine",
    ],
    key="tabs",
    on_change=lambda: st.session_state.update(current_tab=st.session_state.tabs),
)


# ─────────────────────────────────────────────────────────────────────────────
# 1. General
# ─────────────────────────────────────────────────────────────────────────────

with tab1:
    # Generate a stable experiment GUID for this session (regenerated only on explicit reset)
    if "experiment_id" not in st.session_state:
        st.session_state.experiment_id = str(uuid.uuid4())[:8]

    col1, col2 = st.columns([2, 1], vertical_alignment="bottom")

    experiment_name = col1.text_input(
        label="Experiment name",
        key=(key := "experiment_name"),
        value=_default(key),
        placeholder=st.session_state.experiment_id,
        max_chars=40,
        on_change=lambda k=key: _persist(k),
        help=(
            "A human-readable name to identify this experiment (optional). "
            "If no name is filled in, an automatic ID is assigned instead. "
        ),
    )

    experiment_name = experiment_name or st.session_state.experiment_id

    # Validate experiment name for invalid filename characters.
    if chars := INVALID_FILENAME_CHARS.findall(experiment_name):
        st.error(
            f"The following characters are not allowed in experiment names: "
            f"**{' '.join(repr(c) for c in sorted(set(chars)))}** "
        )
        experiment_name = None

    col2.download_button(
        label="Download configuration",
        data=_build_experiment_config,
        file_name=f"{experiment_name or st.session_state.experiment_id}.toml",
        mime="application/toml",
        icon=":material/download:",
        type="secondary",
        on_click="ignore",
        width="stretch",
        disabled=experiment_name is None,
        help="Persist the current experiment configuration to disk.",
    )

    tags = st.multiselect(
        label="Tags",
        key=(key := "tags"),
        options=_default(key, []),
        default=_default(key, []),
        accept_new_options=True,
        placeholder="Add tags...",
        on_change=lambda k=key: (
            st.session_state.update(
                tags=list(dict.fromkeys([tag.strip().lower() for tag in st.session_state.tags]))
            ),
            _persist(k),
        ),
        help=(
            "Add descriptive tags to organize and filter experiments (e.g., intraday, crypto, "
            "mean-reversion)."
        ),
    )

    # Normalize and validate the provided tags
    for tag in tags:
        if not TAG_PATTERN.fullmatch(tag):
            st.error(
                f"Invalid tag: {tag}. Tags must can be at most 20 chars consisting "
                f"only of alphanumeric characters, space, underscore or dash."
            )

    description = st.text_area(
        label="Description",
        key=(key := "description"),
        value=_default(key),
        height=200,
        max_chars=1500,
        placeholder="Add a description...",
        on_change=lambda k=key: _persist(k),
        help=(
            "Summarize the purpose and setup of this run to help you understand and compare "
            "results later. Example information to include are strategy assumptions, parameter "
            "choices, data scope, etc..."
        ),
    )

    st.file_uploader(
        label="Import configuration",
        key="config_upload",
        type=["toml", "yaml", "yml", "json"],
        on_change=_on_config_upload,
        help="Upload a TOML, YAML or JSON file to pre-fill the experiment configuration.",
    )

    if _import_msg := st.session_state.pop("_import_success", None):
        st.success(_import_msg)
    if _import_err := st.session_state.pop("_import_error", None):
        st.error(_import_err)


# ─────────────────────────────────────────────────────────────────────────────
# 2. Data
# ─────────────────────────────────────────────────────────────────────────────

with tab2:
    instrument_type = st.segmented_control(  # ty: ignore[no-matching-overload]
        label="Instrument type",
        key=(key := "instrument_type"),
        required=True,
        options=InstrumentType.variants(),
        default=_default(key, InstrumentType.get_default()),
        format_func=lambda x: f"{x.icon()} {x}",
        on_change=lambda k=key: (_clear_state("symbols", "currency"), _persist(k)),
        help="Select the type of financial instrument you want to backtest.",
    )

    if _default("use_storage", fallback=False):
        provider = cfg.data.providers[instrument_type]
        all_instruments = {x.symbol: x for x in query_instruments(instrument_type, provider)}
    else:
        all_instruments = _list_instruments(instrument_type)

    # Filter instruments based on the selected currency
    if (currency := _default("currency", "All")) != "All":
        fi = {
            k: v
            for k, v in all_instruments.items()
            if v.base == currency or str(v.quote) == currency
        }
    else:
        fi = all_instruments

    col1, col2 = st.columns([5, 1], vertical_alignment="bottom")
    symbol_d, currency_d = _get_instrument_type_description(instrument_type)

    symbols = col1.multiselect(
        label="Symbols",
        key=(key := "symbols"),
        options=sorted(list(fi) + _default(key, [])),
        default=_default(key, []),
        format_func=lambda s: (
            f"{s} - {fi[s].name}" if s in fi and fi[s].instrument_type.is_equity else s
        ),
        placeholder="Select one or more symbols...",
        max_selections=MAX_INSTRUMENT_SELECTION,
        accept_new_options=not _default("use_storage", fallback=False),
        on_change=lambda: (_to_upper_values("symbols"), _persist("symbols")),
        help=symbol_d,
    )

    # Symbols can become 'symbol - name' when changing currency -> extract the symbol
    symbols = [s.split(" - ")[0] if isinstance(s, str) else s for s in symbols]

    options = ["All", *sorted(dict.fromkeys(str(x.quote) for x in all_instruments.values()))]
    col2.selectbox(
        label="Currency",
        key=(key := "currency"),
        options=options,
        index=options.index(_default(key, "All")),
        placeholder="All",
        on_change=lambda k=key: _persist(k),
        help=currency_d,
    )

    if not all_instruments:
        st.info(
            "The database is empty. Head over to the **Download** page to fetch some market data.",
            icon=":material/info:",
        )

    use_storage = st.toggle(
        label="Use stored data",
        key=(key := "use_storage"),
        value=_default(key, fallback=False),
        on_change=lambda k=key: (
            _clear_state("symbols", "currency", "start_date", "end_date"),
            _persist(k),
        ),
        help=(
            "When enabled, the backtest only uses data currently saved in the local "
            "database for the selected symbols and interval. No new data is downloaded. "
            "The date range is determined entirely by what is available in storage."
        ),
    )

    full_history = st.toggle(
        label="Use full available history",
        key=(key := "full_history"),
        value=_default(key, fallback=True),
        on_change=lambda k=key: _persist(k),
        help=(
            "Whether to use the maximum available history for all selected symbols. "
            "If toggled off, select the start and end dates for the simulation."
        ),
    )

    profiles = direct = []
    interval = _default("interval", Interval.get_default())

    summary = None
    raise_missing_interval = None

    try:
        if symbols:
            if use_storage:
                summary = _query_bars_summary()
                summary = summary[summary["interval"] == str(interval)]

                for symbol in symbols:
                    if len(df := summary[summary["symbol"] == symbol]) > 0:
                        row = df.iloc[0]
                        profiles.append(
                            InstrumentProfile(
                                instrument=fi[symbol],
                                earliest_ts={interval: row["first_ts"]},
                                latest_ts={interval: row["last_ts"]},
                                legs=[],
                            )
                        )
                    else:
                        raise_missing_interval = symbol
            else:
                profiles = resolve_profiles(symbols, instrument_type, interval, verbose=False)
                direct = profiles[: len(symbols)]  # Direct profiles (no legs)
    except RuntimeError as ex:
        st.error(ex, icon=":material/error:")

    today = dt.now(tz=tz).date()
    if direct:
        earliest_ts = dt.fromtimestamp(
            min(min(p.earliest_ts.values()) for p in direct), tz=tz
        ).date()
        latest_ts = dt.fromtimestamp(max(max(p.latest_ts.values()) for p in direct), tz=tz).date()

        # Correct latest_ts since some providers return closing bar at 00:00 (so tomorrow)
        latest_ts = min(latest_ts, today)
    else:
        earliest_ts = dt(2000, 1, 1, tzinfo=tz).date()
        latest_ts = today

    if full_history:
        start_ts = earliest_ts
        end_ts = latest_ts
    else:
        col1, col2 = st.columns(2)

        start_ts = col1.date_input(
            label="Start date",
            key=(key := "start_date"),
            value=_default(key, earliest_ts),
            min_value=earliest_ts,
            max_value=latest_ts if use_storage else "today",
            format=cfg.display.date_format,
            on_change=lambda k=key: _persist(k),
            help=(
                "Run backtest simulation starting from this date. If the historical data "
                "does not go so far back, it starts from the available history for that symbol."
            ),
        )

        end_ts = col2.date_input(
            label="End date",
            key=(key := "end_date"),
            value=_default(key, latest_ts if use_storage else "today"),
            min_value=start_ts + timedelta(days=1),
            max_value=latest_ts if use_storage else "today",
            format=cfg.display.date_format,
            on_change=lambda k=key: _persist(k),
            help="Run backtest simulation up to this date.",
        )

    interval = st.pills(
        label="Interval",
        key=(key := "interval"),
        required=True,
        options=cfg.data.providers[instrument_type].intervals(),
        selection_mode="single",
        default=_default(key, Interval.get_default()),
        on_change=lambda k=key: _persist(k),
        help=(
            "The frequency of the data points. Each interval is one tick of the simulation. "
            "After every tick, the strategy is evaluated and orders are resolved. The interval "
            "greatly influences the simulation's speed."
        ),
    )

    if raise_missing_interval:
        st.error(
            f"No data in the database for symbol **{raise_missing_interval}** "
            f"and interval **{interval}**.",
            icon=":material/error:",
        )

    if profiles:
        st.divider()

        with st.expander(
            label="Backtest details",
            key=(key := "details_expander"),
            icon=":material/candlestick_chart:",
            expanded=bool(_default(key)),
            on_change=lambda k=key: _persist(k),
        ):
            html, n_bars = _draw_cards(
                profiles,
                cfg=cfg,
                tz=tz,
                instrument_type=instrument_type,
                full_history=full_history,
                start_ts=start_ts,
                end_ts=end_ts,
                summary=summary,
            )
            st.html(_CARD_CSS + html)


# ─────────────────────────────────────────────────────────────────────────────
# 3. Portfolio
# ─────────────────────────────────────────────────────────────────────────────

with tab3:
    col1, col2 = st.columns([5, 1], vertical_alignment="bottom")

    starting_amount = col1.number_input(
        label="Initial cash",
        key="initial_cash",
        min_value=100,
        value=_default("initial_cash", 10_000),
        step=1_000,
        placeholder="Insert the initial cash...",
        on_change=lambda: _persist("initial_cash"),
        help="Cash balance available at the start of the simulation.",
    )

    base_currency = col2.selectbox(
        label="Base currency",
        key="base_currency",
        options=Currency.variants(),
        index=Currency.variants().index(_default("base_currency", cfg.general.base_currency)),
        on_change=lambda: _persist("base_currency"),
        help=(
            "The currency your portfolio is denominated in during the backtest. All trades, "
            "P&L, margin, leverage and position sizing are tracked in this currency. Instrument "
            "prices will be converted where needed."
        ),
    )

    with st.expander(
        label="Starting positions",
        key=(key := "positions_expander"),
        icon=":material/inventory:",
        expanded=bool(_default(key)),
        on_change=lambda k=key: _persist(k),
    ):
        st.caption(
            "Pre-load the portfolio with existing holdings at the start of the simulation. "
            "Each row represents one position.",
        )

        if direct:
            existing = {r["Symbol"]: r["Quantity"] for r in _default("starting_positions", [])}
            positions = st.data_editor(
                data=[{"Symbol": p.symbol, "Quantity": existing.get(p.symbol, 0)} for p in direct],
                num_rows="fixed",
                hide_index=True,
                column_config={
                    "Symbol": st.column_config.TextColumn(width="medium", disabled=True),
                    "Quantity": st.column_config.NumberColumn(min_value=0),
                },
            )

            st.session_state["_starting_positions"] = positions
        else:
            st.caption("No symbols selected.")


# ─────────────────────────────────────────────────────────────────────────────
# 4. Strategy
# ─────────────────────────────────────────────────────────────────────────────

with tab4:

    def _check_strategy_code(code: str, idx: int) -> bool:
        """Check whether the code contains the expected strategy function."""
        try:
            tree = ast.parse(code)

            for node in tree.body:
                if isinstance(node, ast.FunctionDef) and node.name == "strategy":
                    if [a.arg for a in node.args.args] == ["data", "state", "indicators"]:
                        return True
                    else:
                        st.error(
                            f"**Strategy {idx + 1}:** Function `strategy` doesn't have "
                            f"signature: `strategy(data, state, indicators)`.",
                        )
                        return False

            st.error(
                f"**Strategy {idx + 1}:** No function `strategy(data, state, indicators)` "
                f"found in the code.",
            )
        except SyntaxError as ex:
            st.error(f"**Strategy {idx + 1}:** Syntax error:\n\n{ex}")

        return False

    st.markdown("**Predefined strategies**")
    st.caption(
        "Select one or more built-in strategies to include in the experiment. "
        "Useful for benchmarking against your own strategies.",
    )

    selected_predefined = st.multiselect(
        label="Built-in strategies",
        key=(key := "predefined_strategies"),
        options=StrategyType.variants(),
        format_func=lambda s: s.name,
        default=_default(key, []),
        placeholder="Select strategies...",
        on_change=lambda k=key: _persist(k),
        help="Choose built-in strategies to run alongside your custom ones.",
    )

    if selected_predefined:
        with st.expander(
            label="Strategy descriptions",
            key=(key := "strategy_expander"),
            icon=":material/info:",
            expanded=bool(_default(key)),
            on_change=lambda k=key: _persist(k),
        ):
            for strategy in selected_predefined:
                category = "Portfolio Rotation" if strategy.is_rotation else "Single asset"
                st.markdown(f"**{strategy.name}** · _{category}_")
                st.caption(strategy.description())

    st.divider()

    st.markdown("**Custom strategies**")
    st.caption(
        "Add one or more custom strategy functions. Each strategy is evaluated "
        "independently during the simulation.",
    )

    if "custom_strategies" not in st.session_state:
        st.session_state.custom_strategies = []

    for i, custom_strategy in enumerate(st.session_state.custom_strategies):
        with st.container(border=True):
            col1, col2 = st.columns([5, 1], vertical_alignment="center")

            name = col1.text_input(
                label="Strategy name",
                key=(key := f"strategy_name_{i}"),
                value=_default(key),
                max_chars=40,
                placeholder=f"Strategy {i + 1}",
                label_visibility="collapsed",
                on_change=lambda k=key: _persist(k),
            )

            if col2.button(
                label="Remove",
                key=f"remove_strategy_{i}",
                icon=":material/close:",
                type="tertiary",
            ):
                st.session_state.custom_strategies.pop(i)
                st.rerun()

            source = st.segmented_control(
                label="Source",
                key=(key := f"strategy_source_{i}"),
                required=True,
                options=USER_CODE_OPTIONS,
                default=_default(key, USER_CODE_OPTIONS[0]),
                label_visibility="collapsed",
                on_change=lambda k=key: _persist(k),
            )

            code: str | None = None
            if source == USER_CODE_OPTIONS[0]:
                resp = code_editor(
                    code=custom_strategy.get("code") or STRATEGY_PLACEHOLDER,
                    key=f"strategy_code_editor_{i}",
                    buttons=[
                        {
                            "name": "Copy",
                            "feather": "Copy",
                            "hasText": True,
                            "commands": ["copyAll"],
                            "style": {"top": "0.46rem", "right": "0.4rem"},
                        },
                        {
                            "name": "Save",
                            "feather": "Save",
                            "hasText": True,
                            "commands": ["save-state", ["response", "saved"]],
                            "response": "saved",
                            "style": {"top": "2.25rem", "right": "0.4rem"},
                        },
                    ],
                )
                code = resp["text"]
            else:
                strategy_file = st.file_uploader(
                    label="Strategy file",
                    key=f"strategy_file_{i}",
                    type="py",
                    accept_multiple_files=False,
                    label_visibility="collapsed",
                    help=(
                        "Upload a Python file that defines a top-level function with signature: "
                        "`strategy(data, state, indicators) -> list[Order]`."
                    ),
                )

                if strategy_file is not None:
                    code = strategy_file.read().decode("utf-8")
                    with st.expander("Preview uploaded file"):
                        st.code(code, language="python", line_numbers=True)
                else:
                    st.info("No file uploaded yet.", icon=":material/upload_file:")

            if code:
                _check_strategy_code(code, i)

        st.session_state.custom_strategies[i] = {"name": name, "source": source, "code": code}

    if st.button(
        label="Add strategy",
        icon=":material/add:",
        type="secondary",
    ):
        st.session_state.custom_strategies.append(
            {"name": "", "source": USER_CODE_OPTIONS[0], "code": ""}
        )
        st.rerun()


# ─────────────────────────────────────────────────────────────────────────────
# 5. Indicators
# ─────────────────────────────────────────────────────────────────────────────

with tab5:
    st.caption(
        "Indicators are mathematical functions applied to price and volume data that "
        "quantify trends, momentum, volatility and other market characteristics. The "
        "computed values can then be used in your strategy to make investment decisions. "
        "All selected indicators are computed up-front over the full dataset before the "
        "simulation begins, so they add no per-tick overhead.",
    )

    selected_indicators = st.multiselect(
        label="Built-in indicators",
        key=(key := "builtin_indicators"),
        options=IndicatorType.variants(),
        format_func=lambda i: f"{i} - {i.name}",
        default=_default(key, []),
        placeholder="Select indicators...",
        on_change=lambda k=key: _persist(k),
        help=(
            "Choose zero or more predefined indicators to compute on each bar. They will "
            "be available in your strategy function via the `indicators` argument."
        ),
    )

    st.divider()

    def _check_indicator_code(code: str, idx: int) -> bool:
        """Check whether the code contains the expected indicator function."""
        try:
            tree = ast.parse(code)

            for node in tree.body:
                if isinstance(node, ast.FunctionDef) and node.name == "indicator":
                    if [a.arg for a in node.args.args] == ["data"]:
                        return True
                    else:
                        st.error(
                            f"**Indicator {idx + 1}:** Function `indicator` doesn't have "
                            f"signature: `indicator(data)`.",
                        )
                        return False

            st.error(
                f"**Indicator {idx + 1}:** No function `indicator(data)` found in the code.",
            )
        except SyntaxError as ex:
            st.error(f"**Indicator {idx + 1}:** Syntax error:\n\n{ex}")

        return False

    # Initialize session state for custom indicators
    if "custom_indicators" not in st.session_state:
        st.session_state.custom_indicators = []

    st.markdown("**Custom indicators**")
    st.caption(
        "Add custom indicator functions. The function's return values "
        "are passed to your strategy together with the built-in indicators.",
    )

    for i, custom_indicator in enumerate(st.session_state.custom_indicators):
        with st.container(border=True):
            col1, col2 = st.columns([5, 1], vertical_alignment="center")

            name = col1.text_input(
                label="Indicator name",
                key=(key := f"indicator_name_{i}"),
                value=_default(key),
                max_chars=40,
                placeholder=f"Indicator {i + 1}",
                label_visibility="collapsed",
                on_change=lambda k=key: _persist(k),
            )

            if col2.button(
                label="Remove",
                key=f"remove_indicator_{i}",
                icon=":material/close:",
                type="tertiary",
            ):
                st.session_state.custom_indicators.pop(i)
                st.rerun()

            source = st.segmented_control(
                label="Source",
                key=(key := f"indicator_source_{i}"),
                required=True,
                options=USER_CODE_OPTIONS,
                default=_default(key, USER_CODE_OPTIONS[0]),
                label_visibility="collapsed",
                on_change=lambda k=key: _persist(k),
            )

            code: str | None = None
            if source == USER_CODE_OPTIONS[0]:
                resp = code_editor(
                    code=custom_indicator.get("code") or INDICATOR_PLACEHOLDER,
                    key=f"indicator_code_editor_{i}",
                    buttons=[
                        {
                            "name": "Copy",
                            "feather": "Copy",
                            "hasText": True,
                            "commands": ["copyAll"],
                            "style": {"top": "0.46rem", "right": "0.4rem"},
                        },
                        {
                            "name": "Save",
                            "feather": "Save",
                            "hasText": True,
                            "commands": ["save-state", ["response", "saved"]],
                            "response": "saved",
                            "style": {"top": "2.25rem", "right": "0.4rem"},
                        },
                    ],
                )
                code = resp["text"]
            else:
                indicator_file = st.file_uploader(
                    label="Indicator file",
                    key=f"indicator_file_{i}",
                    type="py",
                    accept_multiple_files=False,
                    label_visibility="collapsed",
                    help=(
                        "Upload a Python file that defines a top-level function with signature: "
                        "`indicator(data) -> dict[str, float]`."
                    ),
                )

                if indicator_file is not None:
                    code = indicator_file.read().decode("utf-8")
                    with st.expander("Preview uploaded file"):
                        st.code(code, language="python", line_numbers=True)
                else:
                    st.info("No file uploaded yet.", icon=":material/upload_file:")

            if code:
                _check_indicator_code(code, i)

        st.session_state.custom_indicators[i] = {"name": name, "source": source, "code": code}

    if st.button(
        label="Add indicator",
        icon=":material/add:",
        type="secondary",
    ):
        st.session_state.custom_indicators.append(
            {"name": "", "source": USER_CODE_OPTIONS[0], "code": ""},
        )
        st.rerun()


# ─────────────────────────────────────────────────────────────────────────────
# 6. Exchange
# ─────────────────────────────────────────────────────────────────────────────

with tab6:
    with st.container(border=True):
        st.markdown("**Commission**")

        col1, col2 = st.columns([2, 3], vertical_alignment="top")

        with col1:
            variants = CommissionType.variants()
            commission_mode = st.radio(
                label="Commission type",
                key=(key := "commission_type"),
                options=variants,
                index=variants.index(_default(key, CommissionType.get_default())),
                horizontal=False,
                on_change=lambda k=key: _persist(k),
                help=(
                    "How trading commissions are calculated. **Percentage** charges a fraction "
                    "of the trade notional value. **Fixed amount** charges a flat commission per "
                    "order. **Percentage + Fixed** applies both a percentage-based and a flat "
                    "commission to every trade."
                ),
            )

        commission_pct_widget = lambda: st.number_input(
            label="Commission (% per trade)",
            key=(key := "commission_pct"),
            value=_default(key, 0.1),
            min_value=0.0,
            max_value=100.0,
            step=0.01,
            format="%.2f",
            on_change=lambda k=key: _persist(k),
            help=(
                "Commission charged per executed order, applied as a percentage of "
                "the trade's notional value."
            ),
        )

        commission_fixed_widget = lambda: st.number_input(
            label=f"Commission ({base_currency} per trade)",
            key=(key := "commission_fixed"),
            value=_default(key, 1.0),
            min_value=0.0,
            step=0.5,
            format="%.2f",
            on_change=lambda k=key: _persist(k),
            help=f"Flat commission charged per executed order in {base_currency}.",
        )

        with col2:
            if commission_mode == CommissionType.Percentage:
                commission_pct = commission_pct_widget()
                commission_fixed = 0.0
            elif commission_mode == CommissionType.Fixed:
                commission_fixed = commission_fixed_widget()
                commission_pct = 0.0
            else:
                commission_pct = commission_pct_widget()
                commission_fixed = commission_fixed_widget()

    with st.container(border=True):
        st.markdown("**Slippage**")

        slippage = st.number_input(
            label="Slippage (% of price per trade)",
            key=(key := "slippage"),
            value=_default(key, 0.05),
            min_value=0.0,
            max_value=100.0,
            step=0.01,
            format="%.2f",
            on_change=lambda k=key: _persist(k),
            help=(
                "Simulated market impact. Each fill price is moved adversely by this percentage "
                "(buys filled higher, sells filled lower)."
            ),
        )

    with st.container(border=True):
        st.markdown("**Order execution**")

        allowed_order_types = st.multiselect(
            label="Allowed order types",
            key=(key := "allowed_order_types"),
            options=OrderType.variants(),
            default=_default(key, [OrderType.get_default()]),
            on_change=lambda k=key: _persist(k),
            help=(
                "Which order types the engine accepts during the simulation. "
                "**Market** orders fill immediately at the current price. "
                "**Limit** orders fill only at the specified price or better. "
                "**Stop-Loss / Take-Profit** become market orders when the trigger price is hit. "
                "**Trailing-Stop** adjusts the stop price as the market moves in your favour. "
                "**Settle-Position** closes an open position at the current market price. "
                "Orders of a type not listed here will raise a hard error."
            ),
        )

        partial_fills = st.toggle(
            label="Partial fills",
            key=(key := "partial_fills"),
            value=_default(key, fallback=False),
            on_change=lambda k=key: _persist(k),
            help=(
                "Simulate partial order fills based on available bar volume. When disabled, "
                "orders are filled entirely or not at all."
            ),
        )

    with st.container(border=True):
        st.markdown("**Margin trading**")

        col_toggle, col_input = st.columns([3, 2], vertical_alignment="center")

        enable_margin = col_toggle.toggle(
            label="Allow margin trading",
            value=_default(key, fallback=True),
            key=(key := "allow_margin"),
            on_change=lambda k=key: _persist(k),
            help=(
                "Safety guardrail for margin usage. When enabled (default), the strategy "
                "may use leverage if it chooses to — the actual decision is made in your "
                "strategy code. When disabled, any attempt to exceed the available cash "
                "balance will raise a hard error and abort the simulation."
            ),
        )

        if enable_margin:
            max_leverage = col_input.number_input(
                label="Max leverage",
                key=(key := "max_leverage"),
                value=_default(key, 1.0),
                min_value=1.0,
                max_value=10.0,
                step=0.5,
                format="%.1f",
                on_change=lambda k=key: _persist(k),
                help=(
                    "Maximum leverage ratio. A value of 2.0 means the strategy can borrow "
                    "up to 1x the portfolio value on top of its own capital. Exceeding this "
                    "limit raises a hard error."
                ),
            )

            col1, col2 = st.columns(2)

            initial_margin = col1.number_input(
                label="Initial margin (%)",
                key=(key := "initial_margin"),
                value=_default(key, 50.0),
                min_value=0.0,
                max_value=100.0,
                step=5.0,
                format="%.1f",
                on_change=lambda k=key: _persist(k),
                help=(
                    "Minimum equity as a percentage of position value required when opening "
                    "a new leveraged position. For example, 50% means you must put up at "
                    "least half the position's value from your own capital."
                ),
            )

            maintenance_margin = col2.number_input(
                label="Maintenance margin (%)",
                key=(key := "maintenance_margin"),
                value=_default(key, 25.0),
                min_value=0.0,
                max_value=100.0,
                step=5.0,
                format="%.1f",
                on_change=lambda k=key: _persist(k),
                help=(
                    "Minimum equity as a percentage of position value that must be maintained. "
                    "If equity drops below this threshold a margin call is triggered."
                ),
            )

            margin_interest = st.number_input(
                label="Margin interest rate (% annual)",
                key=(key := "margin_interest"),
                value=_default(key, 0.0),
                min_value=0.0,
                max_value=100.0,
                step=0.5,
                format="%.2f",
                on_change=lambda k=key: _persist(k),
                help=(
                    "Annualized interest rate charged on borrowed funds. Accrued daily and "
                    "deducted from the portfolio cash balance."
                ),
            )
        else:
            max_leverage = 1.0

    with st.container(border=True):
        st.markdown("**Short selling**")

        allow_short_selling = st.toggle(
            label="Allow short selling",
            key=(key := "allow_short_selling"),
            value=_default(key, fallback=True),
            on_change=lambda k=key: _persist(k),
            help=(
                "Safety guardrail for short positions. When enabled (default), the strategy "
                "may open short positions if it chooses to - the actual decision is made in "
                "your strategy code. When disabled, any attempt to sell positions not currently "
                "held will raise a hard error and abort the simulation."
            ),
        )

        if allow_short_selling:
            borrow_rate = st.number_input(
                label="Borrow rate (% annual)",
                key=(key := "borrow_rate"),
                value=_default(key, 0.0),
                min_value=0.0,
                max_value=100.0,
                step=0.5,
                format="%.2f",
                on_change=lambda k=key: _persist(k),
                help=(
                    "Annualized cost of borrowing shares for short positions. Accrued daily "
                    "and deducted from the portfolio cash balance."
                ),
            )

    with st.container(border=True):
        st.markdown("**Position limits**")

        max_position_size = st.number_input(
            label="Max position size (% of portfolio)",
            key=(key := "max_position_size"),
            value=_default(key, 100),
            min_value=1,
            max_value=100,
            step=5,
            on_change=lambda k=key: _persist(k),
            help=(
                "Maximum allocation to a single position as a percentage of total "
                "portfolio value. Applies to both long and short positions. Set to "
                "100% for no concentration limit. Exceeding this limit raises a hard error."
            ),
        )

    with st.container(border=True):
        st.markdown("**Currency conversion**")

        variants = CurrencyConversionMode.variants()
        conversion_mode = st.selectbox(
            label="Foreign currency handling",
            key=(key := "conversion_mode"),
            options=variants,
            index=variants.index(_default(key, CurrencyConversionMode.get_default())),
            format_func=lambda x: x.name,
            on_change=lambda k=key: _persist(k),
            help=(
                "Determines how proceeds in a foreign currency are converted back to "
                "the base currency. **Immediately** converts at the time of the trade. "
                "**Hold until threshold** keeps the foreign balance until it reaches a "
                "specified amount. **End of period** batches conversions at a chosen "
                "frequency (day, week or month). **Custom interval** lets you specify "
                "the number of bars between conversions."
            ),
        )

        if conversion_mode == CurrencyConversionMode.HoldUntilThreshold:
            conversion_threshold = st.number_input(
                label=f"Conversion threshold ({base_currency})",
                key=(key := "conversion_threshold"),
                value=_default(key, 1_000.0),
                min_value=0.0,
                step=100.0,
                format="%.2f",
                on_change=lambda k=key: _persist(k),
                help=(
                    f"Foreign currency balances are converted to {base_currency} "
                    "once their equivalent value reaches this threshold."
                ),
            )
        elif conversion_mode == CurrencyConversionMode.EndOfPeriod:
            variants = ConversionPeriod.variants()
            conversion_period = st.selectbox(
                label="Conversion period",
                key=(key := "conversion_period"),
                options=variants,
                index=variants.index(_default(key, ConversionPeriod.get_default())),
                on_change=lambda k=key: _persist(k),
                help="How often foreign currency balances are converted to the base currency.",
            )
        elif conversion_mode == CurrencyConversionMode.CustomInterval:
            conversion_interval = st.number_input(
                label="Conversion interval (bars)",
                key=(key := "conversion_interval"),
                value=_default(key, 5),
                min_value=1,
                step=1,
                on_change=lambda k=key: _persist(k),
                help=(
                    "Number of bars between automatic conversions of "
                    "foreign currency balances to the base currency."
                ),
            )


# ─────────────────────────────────────────────────────────────────────────────
# 7. Engine
# ─────────────────────────────────────────────────────────────────────────────

with tab7:
    with st.container(border=True):
        st.markdown("**Timing**")

        warmup_period = st.number_input(
            label="Warmup period (bars)",
            key="warmup_period",
            min_value=0,
            value=_default("warmup_period", 0),
            step=1,
            on_change=lambda: _persist("warmup_period"),
            help=(
                "Number of initial bars to skip before the strategy starts executing. "
                "During the warmup window indicators are computed but no orders are placed. "
                "Use this to let moving averages and other lagging indicators stabilize."
            ),
        )

        trade_on_close = st.toggle(
            label="Trade on close",
            key="trade_on_close",
            value=_default("trade_on_close", fallback=False),
            on_change=lambda: _persist("trade_on_close"),
            help=(
                "When enabled, orders are filled at the current bar's close price. "
                "When disabled (default), orders are filled at the next bar's open price, "
                "which is more realistic."
            ),
        )

    with st.container(border=True):
        st.markdown("**Benchmark & metrics**")

        risk_free_rate = st.number_input(
            label="Risk-free rate (%)",
            key="risk_free_rate",
            min_value=0.0,
            max_value=100.0,
            value=_default("risk_free_rate", 0.0),
            step=0.1,
            format="%.2f",
            on_change=lambda: _persist("risk_free_rate"),
            help=(
                "Annualized risk-free rate used for computing the Sharpe ratio and other "
                "risk-adjusted performance metrics."
            ),
        )

        benchmark = st.text_input(
            label="Benchmark symbol",
            key="benchmark",
            placeholder="e.g. SPY",
            max_chars=20,
            on_change=lambda: _persist("benchmark"),
            help=(
                "Optional benchmark ticker for relative performance comparison. Leave empty "
                "to skip benchmark tracking."
            ),
        )

    with st.container(border=True):
        st.markdown("**Execution behaviour**")

        exclusive_orders = st.toggle(
            label="Exclusive orders",
            key="exclusive_orders",
            value=_default("exclusive_orders", fallback=False),
            on_change=lambda: _persist("exclusive_orders"),
            help=(
                "When enabled, submitting a new order automatically cancels all pending "
                "orders. Useful for strategies that should only have one active order at a time."
            ),
        )

        random_seed = st.number_input(
            label="Random seed",
            key="random_seed",
            min_value=0,
            value=_default("random_seed"),
            step=1,
            placeholder="Leave empty for non-deterministic",
            on_change=lambda: _persist("random_seed"),
            help=(
                "Fixed seed for the random number generator to ensure reproducible results. "
                "Leave empty for non-deterministic execution."
            ),
        )

    with st.container(border=True):
        st.markdown("**Data handling**")

        variants = EmptyBarPolicy.variants()
        empty_bar_policy = st.selectbox(
            label="Empty bar policy",
            key="empty_bar_policy",
            options=variants,
            format_func=lambda x: x.name,
            index=variants.index(_default("empty_bar_policy", EmptyBarPolicy.get_default())),
            on_change=lambda: _persist("empty_bar_policy"),
            help=(
                "How to handle bars with no trading activity (e.g. market closures during "
                "intraday backtests, holidays or illiquid periods).\n\n"
                "**Skip** - the bar is dropped entirely; the strategy is not called and "
                "the simulation clock jumps to the next bar with data.\n\n"
                "**Forward-fill** - OHLC values are copied from the last valid bar and "
                "volume is set to zero. The strategy runs as normal, which keeps a "
                "consistent tick cadence (recommended for most use cases).\n\n"
                "**Fill with NaN** - the bar is kept but all fields are set to NaN. "
                "Your strategy must handle missing values explicitly."
            ),
        )

st.divider()

if st.button(
    label="Run experiment",
    icon=":material/play_circle:",
    type="primary",
    disabled=not (profiles and start_ts and latest_ts),
    shortcut="Enter",
    width="stretch",
):
    with st.spinner(f"Running experiment {experiment_name}..."):
        # TODO: implement backtest execution logic
        st.success(
            f"Backtest **{experiment_name}** queued successfully.",
            icon=":material/check_circle:",
        )
