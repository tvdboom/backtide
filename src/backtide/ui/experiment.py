"""Backtide.

Author: Mavs
Description: Run a new backtest page.

"""

from datetime import datetime as dt
import json
import logging
import tomllib
from typing import Any
import uuid

import streamlit as st
from streamlit.runtime.state import SessionStateProxy
import yaml

from backtide.backtest import (
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
    OrderType,
    PortfolioExpConfig,
    StrategyExpConfig,
    run_experiment,
)
from backtide.config import get_config
from backtide.core.data import resolve_profiles
from backtide.data import Currency, InstrumentProfile, InstrumentType
from backtide.indicators.utils import _get_indicator_label, _load_stored_indicators
from backtide.storage import query_instruments
from backtide.strategies.utils import (
    _get_strategy_label,
    _load_stored_strategies,
    _resolve_auto_indicators,
)
from backtide.ui.utils import (
    _CARD_CSS,
    _clear_state,
    _default,
    _default_benchmark,
    _draw_cards,
    _get_instrument_type_description,
    _get_timezone,
    _list_instruments,
    _persist,
    _query_bars_summary,
    _to_upper_values,
)
from backtide.utils.constants import (
    INVALID_FILENAME_CHARS,
    MAX_INSTRUMENT_SELECTION,
    TAG_PATTERN,
)

# Disable streamlit warnings spawned by the thread running _build_experiment_config
logging.getLogger("streamlit.runtime.scriptrunner_utils.script_run_context").setLevel(
    logging.ERROR
)
# ─────────────────────────────────────────────────────────────────────────────
# Helper functions
# ─────────────────────────────────────────────────────────────────────────────


def _apply_config_to_state(exp: ExperimentConfig, state: dict[str, Any] | SessionStateProxy):
    """Write all fields of `exp` into the session state."""

    def _set(key: str, value: Any):
        """Set the widget and its shadow key."""
        state[key] = value
        state[f"_{key}"] = value

    _set("experiment_name", INVALID_FILENAME_CHARS.sub("", exp.general.name))
    _set("tags", exp.general.tags)
    _set("description", exp.general.description)

    _set("instrument_type", exp.data.instrument_type)
    _set("symbols", exp.data.symbols)
    _set("full_history", exp.data.full_history)
    if not exp.data.full_history:
        if exp.data.start_date:
            _set("start_date", dt.fromisoformat(str(exp.data.start_date)).date())
        if exp.data.end_date:
            _set("end_date", dt.fromisoformat(str(exp.data.end_date)).date())
    _set("interval", exp.data.interval)

    _set("initial_cash", exp.portfolio.initial_cash)
    _set("base_currency", exp.portfolio.base_currency)
    _set("starting_pos", exp.portfolio.starting_positions)

    _set("strategies", exp.strategy.strategies)
    _set("indicators", exp.indicators.indicators)

    for key in (
        "commission_type",
        "commission_pct",
        "commission_fixed",
        "slippage",
        "allowed_order_types",
        "partial_fills",
        "allow_margin",
        "max_leverage",
        "initial_margin",
        "maintenance_margin",
        "margin_interest",
        "allow_short_selling",
        "borrow_rate",
        "max_position_size",
        "conversion_mode",
    ):
        _set(key, getattr(exp.exchange, key))
    for key in ("conversion_threshold", "conversion_period", "conversion_interval"):
        if getattr(exp.exchange, key) is not None:
            _set(key, getattr(exp.exchange, key))

    for key in (
        "warmup_period",
        "trade_on_close",
        "risk_free_rate",
        "exclusive_orders",
        "random_seed",
        "empty_bar_policy",
    ):
        _set(key, getattr(exp.engine, key))

    _set("benchmark", exp.strategy.benchmark)


def _build_config_toml(
    state: dict[str, Any] | SessionStateProxy,
    experiment_name: str,
    default: ExperimentConfig,
) -> str:
    """Build an ExperimentConfig from widget state and return it as TOML."""

    def _get(key: str, fallback: Any) -> Any:
        """Pull `key` from `state`, but coerce `None` to `fallback`."""
        v = state.get(key, state.get(f"_{key}", fallback))
        return fallback if v is None else v

    def _opt(key: str) -> Any:
        """Return the live or shadow value for `key`, or `None`."""
        return state.get(key, state.get(f"_{key}"))

    cfg = ExperimentConfig(
        general=GeneralExpConfig(
            name=experiment_name or "",
            tags=_get("tags", default.general.tags),
            description=_get("description", default.general.description),
        ),
        data=DataExpConfig(
            instrument_type=_get("instrument_type", default.data.instrument_type),
            symbols=[
                s.symbol if hasattr(s, "symbol") else str(s)
                for s in _get("symbols", default.data.symbols)
            ],
            full_history=_get("full_history", default.data.full_history),
            start_date=str(_opt("start_date")) if _opt("start_date") else None,
            end_date=str(_opt("end_date")) if _opt("end_date") else None,
            interval=_get("interval", default.data.interval),
        ),
        portfolio=PortfolioExpConfig(
            initial_cash=_get("initial_cash", default.portfolio.initial_cash),
            base_currency=_get("base_currency", default.portfolio.base_currency),
            starting_positions=_get("starting_pos", default.portfolio.starting_positions),
        ),
        strategy=StrategyExpConfig(
            strategies=list(_get("strategies", [])),
            benchmark=_get("benchmark", default.strategy.benchmark),
        ),
        indicators=IndicatorExpConfig(indicators=list(_get("indicators", []))),
        exchange=ExchangeExpConfig(
            commission_type=_get("commission_type", default.exchange.commission_type),
            commission_pct=_get("commission_pct", default.exchange.commission_pct),
            commission_fixed=_get("commission_fixed", default.exchange.commission_fixed),
            slippage=_get("slippage", default.exchange.slippage),
            allowed_order_types=_get("allowed_order_types", default.exchange.allowed_order_types),
            partial_fills=_get("partial_fills", default.exchange.partial_fills),
            allow_margin=_get("allow_margin", default.exchange.allow_margin),
            max_leverage=_get("max_leverage", default.exchange.max_leverage),
            initial_margin=_get("initial_margin", default.exchange.initial_margin),
            maintenance_margin=_get("maintenance_margin", default.exchange.maintenance_margin),
            margin_interest=_get("margin_interest", default.exchange.margin_interest),
            allow_short_selling=_get("allow_short_selling", default.exchange.allow_short_selling),
            borrow_rate=_get("borrow_rate", default.exchange.borrow_rate),
            max_position_size=_get("max_position_size", default.exchange.max_position_size),
            conversion_mode=_get("conversion_mode", default.exchange.conversion_mode),
            conversion_threshold=_opt("conversion_threshold"),
            conversion_period=_opt("conversion_period"),
            conversion_interval=_opt("conversion_interval"),
        ),
        engine=EngineExpConfig(
            warmup_period=_get("warmup_period", default.engine.warmup_period),
            trade_on_close=_get("trade_on_close", default.engine.trade_on_close),
            risk_free_rate=_get("risk_free_rate", default.engine.risk_free_rate),
            exclusive_orders=_get("exclusive_orders", default.engine.exclusive_orders),
            random_seed=_opt("random_seed"),
            empty_bar_policy=_get("empty_bar_policy", default.engine.empty_bar_policy),
        ),
    )
    return cfg.to_toml()


def _parse_config_upload(upload: Any) -> ExperimentConfig:
    """Parse an uploaded config file into an ExperimentConfig."""
    if upload.name.endswith(".json"):
        raw = json.load(upload)
    elif upload.name.endswith(".toml"):
        raw = tomllib.loads(upload.read().decode("utf-8"))
    else:
        raw = yaml.safe_load(upload)

    return ExperimentConfig.from_dict(raw)


def _build_experiment_config() -> str:
    """Return the current experiment configuration in as a toml string."""
    return _build_config_toml(
        state=st.session_state,
        experiment_name=experiment_name or st.session_state.experiment_id,
        default=exp,
    )


def _on_config_upload():
    """Set the experiment config based on an uploaded file."""
    if upload := st.session_state.get("config_upload"):
        try:
            exp = _parse_config_upload(upload)
            _apply_config_to_state(exp, st.session_state)
            st.session_state["_success"] = f"Loaded configuration from `{upload.name}`."
        except Exception as ex:  # noqa: BLE001
            st.session_state["_error"] = f"Failed to parse file: {ex}"


# ─────────────────────────────────────────────────────────────────────────────
# Experiment interface
# ─────────────────────────────────────────────────────────────────────────────

cfg = get_config()
tz = _get_timezone(cfg.display.timezone)

# A pending config can be injected by another page to pre-fill the form.
if pending := st.session_state.pop("_pending_experiment_config", None):
    try:
        _apply_config_to_state(pending, st.session_state)
    except Exception as _ex:  # noqa: BLE001
        st.session_state["_error"] = f"Failed to apply configuration: {_ex}"
    else:
        st.session_state["_success"] = (
            f"Loaded configuration from experiment **{st.session_state.experiment_name}**."
        )

    # Force a fresh experiment_id so the imported run isn't confused with the original.
    st.session_state.pop("experiment_id", None)

# Single source of truth for all default widget values.
exp: ExperimentConfig = _default("config", ExperimentConfig())

st.set_page_config(page_title="Backtide - Experiment")

st.subheader("Experiment", text_alignment="center")
st.write("")


tab1, tab2, tab3, tab4, tab5, tab6, tab7 = st.tabs(
    [
        ":material/dashboard: General",
        ":material/analytics: Data",
        ":material/account_balance_wallet: Portfolio",
        ":material/psychology: Strategies",
        ":material/show_chart: Indicators",
        ":material/storefront: Exchange",
        ":material/build: Engine",
    ],
    key=(key := "experiment_tabs"),
    default=_default(key),
    on_change=lambda k=key: _persist(k),
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
            f"**{' '.join(sorted(set(chars)))}** "
        )
        experiment_name = None

    col2.download_button(
        label="Download configuration",
        data=_build_experiment_config(),
        file_name=f"{experiment_name or st.session_state.experiment_id}.toml",
        mime="application/toml",
        icon=":material/download:",
        type="secondary",
        on_click="ignore",
        width="stretch",
        disabled=experiment_name is None,
        help="Persist the current experiment configuration to disk.",
    )

    tags = st.multiselect(  # ty : ignore[no-matching-overload]
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

    if _import_msg := st.session_state.pop("_success", None):
        st.success(_import_msg)
    if _import_err := st.session_state.pop("_error", None):
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
        default=_default(key, exp.data.instrument_type),
        format_func=lambda x: f"{x.icon()} {x}",
        on_change=lambda k=key: (
            _clear_state("symbols", default=[]),
            _clear_state("currency", "benchmark", "benchmark_mode"),
            _persist(k),
        ),
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

    symbols = col1.multiselect(  # ty : ignore[no-matching-overload]
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
        on_change=lambda k=key: (  # ty: ignore[invalid-argument-type]
            _clear_state("symbols", default=[]),
            _clear_state("currency", "start_date", "end_date"),
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
        value=_default(key, fallback=exp.data.full_history),
        on_change=lambda k=key: _persist(k),
        help=(
            "Whether to use the maximum available history for all selected symbols. "
            "If toggled off, select the start and end dates for the simulation."
        ),
    )

    profiles = direct = []
    interval = _default("interval", exp.data.interval)

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
            min_value=start_ts,
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
        default=_default(key, exp.data.interval),
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
                estimate_rows=not use_storage,
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
        value=_default("initial_cash", exp.portfolio.initial_cash),
        step=1_000,
        placeholder="Insert the initial cash...",
        on_change=lambda: _persist("initial_cash"),
        help="Cash balance available at the start of the simulation.",
    )

    base_currency = col2.selectbox(  # ty: ignore[no-matching-overload]
        label="Base currency",
        key="base_currency",
        options=Currency.variants(),
        index=Currency.variants().index(_default("base_currency", cfg.general.base_currency)),
        on_change=lambda: (_clear_state("benchmark", "benchmark_mode"), _persist("base_currency")),
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
            is_crypto = instrument_type == InstrumentType.Crypto
            positions = st.data_editor(
                data=[
                    {
                        "Symbol": p.symbol,
                        "Quantity": float(_default("starting_pos", {}).get(p.symbol, 0.0)),
                    }
                    for p in direct
                ],
                num_rows="fixed",
                hide_index=True,
                column_config={
                    "Symbol": st.column_config.TextColumn(width="medium", disabled=True),
                    "Quantity": st.column_config.NumberColumn(
                        min_value=0.0,
                        step=None if is_crypto else 1,
                        format="%.6f" if is_crypto else "%d",
                    ),
                },
            )

            st.session_state["_starting_pos"] = {
                r["Symbol"]: (float(r["Quantity"]) if is_crypto else int(r["Quantity"]))
                for r in positions
            }
        else:
            st.caption("No symbols selected.")


# ─────────────────────────────────────────────────────────────────────────────
# 4. Strategy
# ─────────────────────────────────────────────────────────────────────────────

with tab4:
    st.caption(
        "Strategies define the trading logic that is evaluated at every interval of the "
        "simulation. Select one or more strategies to include in the experiment. You can "
        "create predefined (built-in) or custom strategies on the **Strategies** page.",
    )

    stored_strat = _load_stored_strategies(cfg)
    selected_strats = []
    err_benchmark = None

    if not stored_strat:
        st.info(
            "No saved strategies yet. Create a strategy on the **Strategies** page first.",
            icon=":material/info:",
        )
    else:
        bench_none = ":material/block: None"
        bench_symbol = ":material/toll: Symbol"
        bench_strategy = ":material/psychology: Strategy"

        default = bench_none if instrument_type == InstrumentType.Forex else bench_symbol
        bench_mode = st.segmented_control(  # ty: ignore[no-matching-overload]
            label="Benchmark",
            key=(key := "benchmark_mode"),
            required=True,
            options=[bench_none, bench_symbol, bench_strategy],
            default=_default(key, default),
            on_change=lambda k=key: (_clear_state("benchmark"), _persist(k)),
            help=(
                "Choose how the benchmark is determined. **None** disables the benchmark. "
                "**Symbol** uses a passive buy-and-hold on a symbol. **Strategy** uses one "
                "of the selected strategies as the benchmark."
            ),
        )

        if bench_mode == bench_symbol:
            if default_sym := _default_benchmark(
                base_currency, instrument_type, list(all_instruments)
            ):
                # Benchmarks for an equity experiment may be either a stock or an ETF
                if instrument_type.is_equity:
                    if instrument_type == InstrumentType.Stocks:
                        other = InstrumentType.Etf
                    else:
                        other = InstrumentType.Stocks

                    if _default("use_storage", fallback=False):
                        bench_universe = {
                            **all_instruments,
                            **{
                                x.symbol: x
                                for x in query_instruments(other, cfg.data.providers[other])
                            },
                        }
                    else:
                        bench_universe = {**all_instruments, **_list_instruments(other)}
                else:
                    bench_universe = all_instruments

                if not _default("benchmark"):
                    st.session_state["benchmark"] = default_sym
                    st.session_state["_benchmark"] = default_sym

                options = sorted({*bench_universe.keys(), _default("benchmark")})
                benchmark = st.selectbox(
                    label="Benchmark symbol",
                    key=(key := "benchmark"),
                    options=options,
                    index=options.index(_default("benchmark")),
                    format_func=lambda s: (
                        f"{s} - {bench_universe[s].name}"
                        if s in bench_universe and bench_universe[s].instrument_type.is_equity
                        else s
                    ),
                    accept_new_options=not _default("use_storage", fallback=False),
                    on_change=lambda k=key: _persist(k),
                    help=(
                        "Symbol used as a passive (buy-and-hold) baseline from which the "
                        "alpha (excess return) of every other strategy other strategy is "
                        "computed. The default value is a ETF consisting of major equities "
                        "for the selected base currency, e.g., SPY (S&P 500) for USD."
                    ),
                )

                # Validate the benchmark symbol
                if benchmark and benchmark not in bench_universe:
                    for it in (InstrumentType.Stocks, InstrumentType.Etf):
                        try:
                            resolve_profiles(benchmark, it, interval, verbose=False)
                            break
                        except RuntimeError as ex:
                            err_benchmark = ex

                    if err_benchmark:
                        st.error(err_benchmark, icon=":material/error:")

        elif bench_mode == bench_strategy:
            strat_options = _default("strategies", [])
            if strat_options:
                bench_strat = st.selectbox(
                    label="Benchmark strategy",
                    key=(key := "benchmark"),
                    options=strat_options,
                    index=strat_options.index(_default("benchmark")),
                    on_change=lambda k=key: _persist(k),
                    help=(
                        "Use the selected strategy as the benchmark. Alpha is computed "
                        "relative to this strategy's results."
                    ),
                )
            else:
                st.info(
                    "Select at least one strategy below to use as benchmark.",
                    icon=":material/info:",
                )
                st.session_state["benchmark"] = None
                st.session_state["_benchmark"] = None

        else:
            st.session_state["benchmark"] = None
            st.session_state["_benchmark"] = None

        st.write("")

        selected_strats = st.multiselect(
            label="Strategies",
            key=(key := "strategies"),
            options=stored_strat,
            default=_default(key, []),
            placeholder="Select from the saved strategies...",
            on_change=lambda k=key: _persist(k),
            help="Choose which strategies to use in this experiment.",
        )

        # Show a summary of selected strategies
        for name in selected_strats:
            st.caption(_get_strategy_label(name, stored_strat[name]))

    if st.button("Create a new strategy", icon=":material/add:", type="secondary"):
        st.switch_page("strategies.py")


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

    if stored_ind := _load_stored_indicators(cfg):
        selected_ind = st.multiselect(
            label="Indicators",
            key=(key := "indicators"),
            options=stored_ind,
            default=_default(key, []),
            placeholder="Select from the saved indicators...",
            on_change=lambda k=key: _persist(k),
            help="Choose which indicators to use in this experiment.",
        )

        # Show a summary of selected indicators
        for name in selected_ind:
            st.caption(_get_indicator_label(name, stored_ind[name]))
    else:
        st.info(
            "No saved indicators yet. Create an indicator on the **Indicators** page first.",
            icon=":material/info:",
        )

    # Add indicators auto-included by the built-in strategies
    if selected_strats:
        if auto_i := _resolve_auto_indicators([stored_strat[n] for n in selected_strats]):
            for name, ind, src in auto_i:
                label = _get_indicator_label(name.removeprefix("__auto_"), ind)
                st.caption(f"{label} · :material/auto_awesome: _Injected by strategy **{src}**_")

    if st.button("Create a new indicator", icon=":material/add:", type="secondary"):
        st.switch_page("indicators.py")


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
                index=variants.index(_default(key, exp.exchange.commission_type)),
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
            value=_default(key, exp.exchange.commission_pct),
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
            value=_default(key, exp.exchange.commission_fixed),
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
            value=_default(key, exp.exchange.slippage),
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
            default=_default(key, exp.exchange.allowed_order_types),
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
            value=_default(key, fallback=exp.exchange.partial_fills),
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
            key=(key := "allow_margin"),
            value=_default(key, fallback=exp.exchange.allow_margin),
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
                value=_default(key, exp.exchange.max_leverage),
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
                value=_default(key, exp.exchange.initial_margin),
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
                value=_default(key, exp.exchange.maintenance_margin),
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
                value=_default(key, exp.exchange.margin_interest),
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
            max_leverage = exp.exchange.max_leverage

    with st.container(border=True):
        st.markdown("**Short selling**")

        allow_short_selling = st.toggle(
            label="Allow short selling",
            key=(key := "allow_short_selling"),
            value=_default(key, fallback=exp.exchange.allow_short_selling),
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
                value=_default(key, exp.exchange.borrow_rate),
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
            value=_default(key, exp.exchange.max_position_size),
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
            index=variants.index(_default(key, exp.exchange.conversion_mode)),
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
            key=(key := "warmup_period"),
            value=_default(key, exp.engine.warmup_period),
            min_value=0,
            step=1,
            on_change=lambda k=key: _persist(k),
            help=(
                "Number of initial bars to skip before the strategy starts executing. "
                "During the warmup window indicators are computed but no orders are placed. "
                "Use this to let moving averages and other lagging indicators stabilize."
            ),
        )

        trade_on_close = st.toggle(
            label="Trade on close",
            key=(key := "trade_on_close"),
            value=_default(key, fallback=exp.engine.trade_on_close),
            on_change=lambda k=key: _persist(k),
            help=(
                "When enabled, orders are filled at the current bar's close price. "
                "When disabled (default), orders are filled at the next bar's open price, "
                "which is more realistic."
            ),
        )

    with st.container(border=True):
        st.markdown("**Metrics**")

        risk_free_rate = st.number_input(
            label="Risk-free rate (%)",
            key=(key := "risk_free_rate"),
            value=_default(key, exp.engine.risk_free_rate),
            min_value=0.0,
            max_value=100.0,
            step=0.1,
            format="%.2f",
            on_change=lambda k=key: _persist(k),
            help=(
                "Annualized risk-free rate used for computing the Sharpe ratio and other "
                "risk-adjusted performance metrics."
            ),
        )

    with st.container(border=True):
        st.markdown("**Execution behaviour**")

        exclusive_orders = st.toggle(
            label="Exclusive orders",
            key=(key := "exclusive_orders"),
            value=_default(key, fallback=exp.engine.exclusive_orders),
            on_change=lambda k=key: _persist(k),
            help=(
                "When enabled, submitting a new order automatically cancels all pending "
                "orders. Useful for strategies that should only have one active order at a time."
            ),
        )

        random_seed = st.number_input(
            label="Random seed",
            key=(key := "random_seed"),
            value=_default(key),
            min_value=0,
            step=1,
            placeholder="Leave empty for non-deterministic",
            on_change=lambda k=key: _persist(k),
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
            key=(key := "empty_bar_policy"),
            options=variants,
            index=variants.index(_default(key, exp.engine.empty_bar_policy)),
            format_func=lambda x: x.name,
            on_change=lambda k=key: _persist(k),
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


# ─────────────────────────────────────────────────────────────────────────────
# Experiment logic
# ─────────────────────────────────────────────────────────────────────────────

running_experiment = st.session_state.get("running_experiment", False)

# At least one non-benchmark strategy must be selected
bench_val = st.session_state.get("benchmark") or _default("benchmark", None)
non_bench = [s for s in selected_strats if s != bench_val or bench_val not in selected_strats]

can_run = profiles and start_ts and latest_ts and not err_benchmark and non_bench

if not running_experiment and not can_run:
    missing = []
    if not symbols:
        missing.append("select at least one **symbol**")
    if not non_bench:
        missing.append("select at least one non-benchmark **strategy**")
    if err_benchmark:
        missing.append("fix the **benchmark** symbol error")
    if not start_ts or not latest_ts:
        missing.append("configure a valid **date range**")
    if missing:
        st.info(
            "To run the experiment, " + ", ".join(missing) + ".",
            icon=":material/info:",
        )

button_slot = st.empty()
if button_slot.button(
    label="Run experiment",
    key="running_experiment",
    icon=":material/play_circle:",
    type="primary",
    disabled=not can_run or running_experiment,
    shortcut="Enter",
    width="stretch",
):
    # Hide the run button while the experiment is executing.
    button_slot.empty()

    cfg_str = _build_experiment_config()
    exp_cfg = ExperimentConfig.from_toml(cfg_str)

    with st.spinner(f"Running experiment **{experiment_name}**..."):
        try:
            # ``run_experiment`` auto-injects the indicators required by the
            # selected strategies and runs a Buy & Hold benchmark side-car
            # whenever ``strategy.benchmark`` is set. Both happen for any
            # caller — UI, CLI, or Python script.
            result = run_experiment(exp_cfg, verbose=False)
        except Exception as ex:  # noqa: BLE001
            st.error(f"Experiment failed: {ex}", icon=":material/error:")
        else:
            n_strats = len(result.strategies)

            if result.status == "completed" and not result.warnings:
                st.success(
                    f"Experiment **{experiment_name}** finished successfully. "
                    f"Evaluated {n_strats} strateg{'y' if n_strats == 1 else 'ies'}.",
                    icon=":material/check_circle:",
                )
            elif result.status == "completed":
                st.warning(
                    f"Experiment **{experiment_name}** finished with {len(result.warnings)} "
                    f"warning{'s' if len(result.warnings) > 1 else ''}. Evaluated {n_strats} "
                    f"{'strategy' if n_strats == 1 else 'strategies'}.\n\n"
                    + "\n".join(f"- {w}" for w in result.warnings),
                    icon=":material/warning:",
                )
            else:
                st.error(
                    f"Experiment **{experiment_name}** failed.\n\n"
                    + "\n".join(f"- {w}" for w in result.warnings),
                    icon=":material/error:",
                )

            st.button(
                label="View results",
                icon=":material/fact_check:",
                type="primary",
                shortcut="Enter",
                width="stretch",
                on_click=lambda: st.session_state.update(to_results=result.experiment_id),
            )

if st.session_state.get("to_results"):
    st.session_state["selected_experiment_id"] = st.session_state.pop("to_results")
    st.switch_page("results.py")
