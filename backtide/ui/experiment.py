"""Backtide.

Author: Mavs
Description: Run a new backtest page.

"""

import ast
from datetime import datetime
import json
import time
import tomllib

from code_editor import code_editor
import streamlit as st
import yaml

from backtide.data import AssetType, Currency, Interval
from backtide.config import get_config
from backtide.ui.utils import (
    _get_asset_type_description,
    _list_symbols,
    _prevent_deselection,
    _to_upper_values,
)
from backtide.utils.constants import (
    INDICATOR_PLACEHOLDER,
    MAX_ASSET_SELECTION,
    ORDER_TYPES,
    STRATEGY_PLACEHOLDER,
    TAG_PATTERN,
)

INDICATORS = [
    "SMA - Simple Moving Average",
    "EMA - Exponential Moving Average",
    "WMA - Weighted Moving Average",
    "RSI - Relative Strength Index",
    "MACD - Moving Avg. Convergence Divergence",
    "BB - Bollinger Bands",
    "ATR - Average True Range",
    "OBV - On-Balance Volume",
    "VWAP - Volume-Weighted Average Price",
    "STOCH - Stochastic Oscillator",
    "CCI - Commodity Channel Index",
    "ADX - Average Directional Index",
]

FEE_MODES = ["Percentage (%)", "Fixed amount", "Percentage + Fixed"]

cfg = get_config()

st.set_page_config(page_title="Backtide - Experiment", layout="centered")
st.title("Experiment", text_alignment="center")

tab1, tab2, tab3, tab4, tab5, tab6, tab7 = st.tabs(
    [
        ":material/dashboard: Overview",
        ":material/analytics: Data",
        ":material/account_balance_wallet: Portfolio",
        ":material/psychology: Strategy",
        ":material/show_chart: Indicators",
        ":material/storefront: Exchange",
        ":material/build: Engine",
    ],
)


# ═════════════════════════════════════════════════════════════════════════════
# 1. Overview
# ═════════════════════════════════════════════════════════════════════════════

with tab1:
    experiment_name = st.text_input(
        label="Experiment name",
        placeholder="Insert name...",
        max_chars=60,
        help=(
            "A human-readable name to identify this experiment (optional). "
            "If no name is filled in, an automatic GUID is assigned instead."
        ),
    )

    tags = st.multiselect(
        label="Tags",
        options=[],
        default=[],
        accept_new_options=True,
        placeholder="Add tags...",
        help=(
            "Add descriptive tags to organize and filter experiments (e.g., intraday, crypto, "
            "mean-reversion)."
        ),
    )

    # Normalize and validate the provided tags
    if tags:
        valid_tags = []
        for tag in tags:
            tag = tag.strip().lower()
            if TAG_PATTERN.fullmatch(tag):
                valid_tags.append(tag)
            else:
                st.error(
                    f"Invalid tag: {tag}. Tags must must be one word with ≤15 chars consisting "
                    f"only of alphanumeric characters, underscores or dashes.",
                )

        tags = sorted(set(valid_tags))

    description = st.text_area(
        label="Description",
        height="stretch",
        max_chars=500,
        placeholder="Add a description...",
        help=(
            "Summarize the purpose and setup of this run to help you understand and compare "
            "results later. Example information to include are strategy assumptions, parameter "
            "choices, data scope, etc..."
        ),
    )

    uploaded = st.file_uploader(
        label="Import configuration",
        type=["toml", "yaml", "yml", "json"],
        help="Upload a TOML, YAML or JSON file to pre-fill the experiment configuration.",
    )

    if uploaded is not None:
        try:
            if uploaded.name.endswith(".json"):
                config = json.load(uploaded)
            elif uploaded.name.endswith(".toml"):
                config = tomllib.loads(uploaded.read().decode("utf-8"))
            else:
                config = yaml.safe_load(uploaded)

            st.session_state["experiment_name"] = config.get("name", "")
            st.session_state["tags"] = config.get("tags", [])
            st.session_state["description"] = config.get("description", "")
            st.success(f"Loaded configuration from `{uploaded.name}`.")
        except (yaml.YAMLError, json.JSONDecodeError, tomllib.TOMLDecodeError, TypeError) as ex:
            st.error(f"Failed to parse file: {ex}")


# ═════════════════════════════════════════════════════════════════════════════
# 2. Data
# ═════════════════════════════════════════════════════════════════════════════

with tab2:
    if not st.session_state.get("asset_type"):
        _cache = st.session_state.get("_cache", {})
        st.session_state.asset_type = _cache.get("asset_type", AssetType.get_default())

    asset_type = st.segmented_control(
        label="Asset type",
        key="asset_type",
        options=AssetType.variants(),
        format_func=lambda at: f"{at.icon()} {at}",
        on_change=_prevent_deselection(
            key="asset_type",
            default=AssetType.get_default(),
            reset=["symbols", "currency", "symbols_download", "currency_download"],
        ),
        help="Select the type of financial asset you want to backtest.",
    )

    # Reload assets when asset type changes
    all_assets = _list_symbols(st.session_state.asset_type)

    # Filter assets based on the selected currency
    if currency := st.session_state.get("currency"):
        filtered_assets = [
            asset
            for asset in all_assets
            if currency == "All" or asset.base == currency or str(asset.quote) == currency
        ]
    else:
        filtered_assets = all_assets

    col1, col2 = st.columns([5, 1], vertical_alignment="bottom")
    symbol_d, currency_d = _get_asset_type_description(st.session_state.asset_type)

    symbols = col1.multiselect(
        label="Symbols",
        key="symbols",
        options=sorted(filtered_assets, key=lambda a: a.symbol),
        format_func=lambda a: (
            f"{a.symbol} - {a.name}"
            if st.session_state.asset_type in (AssetType.Stocks, AssetType.Etf)
            else a.symbol
        ),
        placeholder="Select one or more symbols...",
        max_selections=MAX_ASSET_SELECTION,
        accept_new_options=True,
        on_change=_to_upper_values("symbols"),
        help=symbol_d,
    )

    col2.selectbox(
        label="Currency",
        key="currency",  # Use key to filter tickers
        options=["All", *sorted(dict.fromkeys(str(a.quote) for a in all_assets))],
        placeholder="All",
        help=currency_d,
    )

    full_history = st.toggle(
        label="Use full available history",
        value=True,
        help=(
            "Whether to use the maximum available history for all selected symbols. "
            "If toggled off, select the start and end dates for the simulation."
        ),
    )

    if not full_history:
        col1, col2 = st.columns(2)

        start_date = col1.date_input(
            label="Start date",
            value=None,
            min_value="2000-01-01",
            max_value=datetime.now().date(),
            help=(
                "Run backtest simulation starting from this date. If the historical data "
                "does not go so far back, it starts from the available history for that symbol."
            ),
        )

        end_date = col2.date_input(
            label="End date",
            value="today",
            min_value=start_date,
            max_value="today",
            help="Run backtest simulation up to this date.",
        )
    else:
        start_date = None
        end_date = None

    interval = st.pills(
        label="Interval",
        options=Interval.variants(),
        format_func=lambda x: str(x),
        selection_mode="single",
        default=Interval.get_default(),
        help=(
            "The frequency of the data points. Each interval is one tick of the simulation. "
            "After every tick, the strategy is evaluated and orders are resolved. The interval "
            "greatly influences the simulation's speed."
        ),
    )


# ═════════════════════════════════════════════════════════════════════════════
# 3. Portfolio
# ═════════════════════════════════════════════════════════════════════════════

with tab3:
    base_currency = st.session_state.get("base_currency", cfg.general.base_currency)

    col1, col2 = st.columns([5, 1], vertical_alignment="bottom")

    starting_amount = col1.number_input(
        label="Initial cash",
        min_value=100,
        value=10_000,
        step=1_000,
        placeholder="Insert the initial cash...",
        help="Cash balance available at the start of the simulation.",
    )

    base_currency = col2.selectbox(
        label="Base currency",
        key="base_currency",
        options=Currency.variants(),
        index=Currency.variants().index(base_currency),
        help=(
            "The currency your portfolio is denominated in during the backtest. All trades, "
            "P&L, margin, leverage and position sizing are tracked in this currency. Asset "
            "prices will be converted where needed."
        ),
    )

    with st.expander("Starting positions"):
        st.caption(
            "Pre-load the portfolio with existing holdings at the start of the simulation. "
            "Each row represents one position.",
        )

        if symbols:
            positions_data = st.data_editor(
                data=[
                    {"Symbol": a.symbol if hasattr(a, "symbol") else str(a), "Quantity": 0.0}
                    for a in symbols
                ],
                num_rows="fixed",
                hide_index=True,
                column_config={
                    "Symbol": st.column_config.TextColumn("Symbol", width="medium", disabled=True),
                    "Quantity": st.column_config.NumberColumn(
                        "Quantity",
                        min_value=0.0,
                        format="%.2f",
                    ),
                },
            )
        else:
            st.caption("No symbols selected.")


# ═════════════════════════════════════════════════════════════════════════════
# 4. Strategy
# ═════════════════════════════════════════════════════════════════════════════

with tab4:
    if st.session_state.get("strategy_source") is None:
        st.session_state.strategy_source = "Code editor"

    strategy_source = st.segmented_control(
        label="Strategy source",
        key="strategy_source",
        options=["Code editor", "Upload file"],
        on_change=_prevent_deselection(
            key="strategy_source",
            default="Code editor",
        ),
        help="Provide your strategy as inline code or by uploading a Python file.",
    )

    strategy_code: str | None = None
    if strategy_source == "Code editor":
        st.caption("Write your strategy function below.")
        code_editor_resp = code_editor(
            code=STRATEGY_PLACEHOLDER,
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

        strategy_code = code_editor_resp["text"]
    else:
        uploaded_file = st.file_uploader(
            label="Strategy file",
            type=["py"],
            accept_multiple_files=False,
            help=(
                "Upload a Python file that defines a top-level function with signature: "
                "`strategy(data, state, indicators)`."
            ),
        )

        if uploaded_file is not None:
            strategy_code = uploaded_file.read().decode("utf-8")
            with st.expander("Preview uploaded file"):
                st.code(strategy_code, language="python", line_numbers=True)
        else:
            st.info("No file uploaded yet.", icon=":material/upload_file:")

    if strategy_code:

        def check_strategy_code(code: str) -> bool:
            """Check whether the code contains the expected function."""
            try:
                tree = ast.parse(code)

                for node in tree.body:
                    if isinstance(node, ast.FunctionDef) and node.name == "strategy":
                        if [a.arg for a in node.args.args] == ["data", "state", "indicators"]:
                            return True
                        else:
                            st.error(
                                "Function `strategy` doesn't have signature: "
                                "`strategy(data, state, indicators)`.",
                            )
                            break

                    st.error("No function `strategy(data, state, indicators)` found in the code.")
            except SyntaxError as ex:
                st.error(f"Syntax error:\n\n{ex}")

            return False

        if check_strategy_code(strategy_code):
            # Show success message for 2 seconds
            success = st.success("Strategy successfully saved.")
            time.sleep(1.5)
            success.empty()


# ═════════════════════════════════════════════════════════════════════════════
# 5. Indicators
# ═════════════════════════════════════════════════════════════════════════════

with tab5:
    st.caption(
        "Select built-in indicators and/or provide your own custom indicator function. "
        "Every selected indicator is computed once per interval for each symbol.",
    )

    selected_indicators = st.multiselect(
        label="Built-in indicators",
        options=INDICATORS,
        default=[],
        placeholder="Select indicators...",
        help=(
            "Choose zero or more premade indicators to compute on each bar. "
            "They will be available in your strategy function via the `indicators` argument."
        ),
    )

    st.divider()

    if st.session_state.get("indicator_source") is None:
        st.session_state.indicator_source = "None"

    indicator_source = st.segmented_control(
        label="Custom indicator",
        key="indicator_source",
        options=["None", "Code editor", "Upload file"],
        on_change=_prevent_deselection(
            key="indicator_source",
            default="None",
        ),
        help=(
            "Optionally provide a custom indicator function. Its return values "
            "are merged with the built-in indicators and passed to your strategy."
        ),
    )

    custom_indicator_code: str | None = None
    if indicator_source == "Code editor":
        st.caption("Write your custom indicator function below.")
        indicator_editor_resp = code_editor(
            code=INDICATOR_PLACEHOLDER,
            key="indicator_code_editor",
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

        custom_indicator_code = indicator_editor_resp["text"]
    elif indicator_source == "Upload file":
        indicator_file = st.file_uploader(
            label="Indicator file",
            type=["py"],
            accept_multiple_files=False,
            help=(
                "Upload a Python file that defines a top-level function with signature: "
                "`indicator(data)` returning `dict[str, float]`."
            ),
        )

        if indicator_file is not None:
            custom_indicator_code = indicator_file.read().decode("utf-8")
            with st.expander("Preview uploaded file"):
                st.code(custom_indicator_code, language="python", line_numbers=True)
        else:
            st.info("No file uploaded yet.", icon=":material/upload_file:")

    if custom_indicator_code:

        def check_indicator_code(code: str) -> bool:
            """Check whether the code contains the expected indicator function."""
            try:
                tree = ast.parse(code)

                for node in tree.body:
                    if isinstance(node, ast.FunctionDef) and node.name == "indicator":
                        if [a.arg for a in node.args.args] == ["data"]:
                            return True
                        else:
                            st.error(
                                "Function `indicator` doesn't have signature: "
                                "`indicator(data)`.",
                            )
                            break

                    st.error("No function `indicator(data)` found in the code.")
            except SyntaxError as ex:
                st.error(f"Syntax error:\n\n{ex}")

            return False

        if check_indicator_code(custom_indicator_code):
            success = st.success("Custom indicator saved.")
            time.sleep(1.5)
            success.empty()


# ═════════════════════════════════════════════════════════════════════════════
# 6. Exchange
# ═════════════════════════════════════════════════════════════════════════════

with tab6:
    base_cur = st.session_state.get("base_currency", Currency.get_default())

    with st.container(border=True):
        fee_mode = st.radio(
            label="Fee type",
            options=FEE_MODES,
            index=0,
            horizontal=True,
            help=(
                "How trading fees are calculated. **Percentage** charges a fraction of "
                "the trade notional value. **Fixed amount** charges a flat fee per order. "
                "**Percentage + Fixed** applies both a percentage-based and a flat fee to "
                "every trade."
            ),
        )

        is_pct = fee_mode == FEE_MODES[0]
        is_fixed = fee_mode == FEE_MODES[1]
        is_combo = fee_mode == FEE_MODES[2]

        if is_pct:
            fee_pct = st.number_input(
                label="Fee (% per trade)",
                min_value=0.0,
                max_value=100.0,
                value=0.1,
                step=0.01,
                format="%.4f",
                help=(
                    "Fee charged per executed order, applied as a percentage of the trade's "
                    "notional value."
                ),
            )
            fee_fixed_value = 0.0
        elif is_fixed:
            fee_fixed_value = st.number_input(
                label=f"Fee ({base_cur} per trade)",
                min_value=0.0,
                value=1.0,
                step=0.5,
                format="%.4f",
                help=f"Flat fee charged per executed order in {base_cur}.",
            )
            fee_pct = 0.0
        else:
            col_pct, col_fixed = st.columns(2)

            fee_pct = col_pct.number_input(
                label="Fee (% per trade)",
                min_value=0.0,
                max_value=100.0,
                value=0.1,
                step=0.01,
                format="%.4f",
                help="Percentage portion of the fee, applied to the trade's notional value.",
            )

            fee_fixed_value = col_fixed.number_input(
                label=f"Fee ({base_cur} per trade)",
                min_value=0.0,
                value=1.0,
                step=0.5,
                format="%.4f",
                help=f"Fixed portion of the fee in {base_cur}, added on top of the percentage fee.",
            )

    slippage = st.number_input(
        label="Slippage (% of price per trade)",
        min_value=0.0,
        max_value=100.0,
        value=0.05,
        step=0.01,
        format="%.4f",
        help=(
            "Simulated market impact. Each fill price is moved adversely by this percentage "
            "(buys filled higher, sells filled lower)."
        ),
    )

    with st.container(border=True):
        st.markdown("**Order execution**")

        allowed_order_types = st.multiselect(
            label="Allowed order types",
            options=ORDER_TYPES,
            default=["Market"],
            help=(
                "Which order types the strategy is allowed to submit. "
                "**Market** orders fill immediately at the current price. "
                "**Limit** orders fill only at the specified price or better. "
                "**Stop** orders become market orders once the stop price is hit. "
                "**Stop-Limit** orders become limit orders once the stop price is hit."
            ),
        )

        partial_fills = st.toggle(
            label="Partial fills",
            value=False,
            help=(
                "Simulate partial order fills based on available bar volume. When disabled, "
                "orders are filled entirely or not at all."
            ),
        )

    with st.container(border=True):
        st.markdown("**Position constraints**")

        allow_short_selling = st.toggle(
            label="Allow short selling",
            value=False,
            help="Allow the strategy to open short positions (sell assets not currently held).",
        )

        max_position_size = st.number_input(
            label="Max position size (% of portfolio)",
            min_value=1,
            max_value=100,
            value=100,
            step=5,
            help=(
                "Maximum allocation to a single position as a percentage of total portfolio value. "
                "Set to 100% for no concentration limit."
            ),
        )

        enable_margin = st.toggle(
            label="Enable margin trading",
            value=False,
            help="Allow the strategy to use leverage by borrowing funds.",
        )

        if enable_margin:
            max_leverage = st.number_input(
                label="Max leverage",
                min_value=1.0,
                max_value=10.0,
                value=1.0,
                step=0.5,
                format="%.1f",
                help=(
                    "Maximum leverage ratio. A value of 2.0 means the strategy can borrow "
                    "up to 1× the portfolio value on top of its own capital."
                ),
            )
        else:
            max_leverage = 1.0


# ═════════════════════════════════════════════════════════════════════════════
# 7. Engine
# ═════════════════════════════════════════════════════════════════════════════

with tab7:
    warmup_period = st.number_input(
        label="Warmup period (bars)",
        min_value=0,
        value=0,
        step=1,
        help=(
            "Number of initial bars to skip before the strategy starts executing. "
            "During the warmup window indicators are computed but no orders are placed. "
            "Use this to let moving averages and other lagging indicators stabilize."
        ),
    )

    risk_free_rate = st.number_input(
        label="Risk-free rate (%)",
        min_value=0.0,
        max_value=100.0,
        value=0.0,
        step=0.1,
        format="%.2f",
        help=(
            "Annualized risk-free rate used for computing the Sharpe ratio and other "
            "risk-adjusted performance metrics."
        ),
    )

    benchmark = st.text_input(
        label="Benchmark symbol",
        placeholder="e.g. SPY",
        max_chars=20,
        help=(
            "Optional benchmark ticker for relative performance comparison. Leave empty "
            "to skip benchmark tracking."
        ),
    )

    random_seed = st.number_input(
        label="Random seed",
        min_value=0,
        value=None,
        step=1,
        placeholder="Leave empty for non-deterministic",
        help=(
            "Fixed seed for the random number generator to ensure reproducible results. "
            "Leave empty for non-deterministic execution."
        ),
    )

    trade_on_close = st.toggle(
        label="Trade on close",
        value=False,
        help=(
            "When enabled, orders are filled at the current bar's close price. "
            "When disabled (default), orders are filled at the next bar's open price, "
            "which is more realistic."
        ),
    )

    exclusive_orders = st.toggle(
        label="Exclusive orders",
        value=False,
        help=(
            "When enabled, submitting a new order automatically cancels all pending "
            "orders. Useful for strategies that should only have one active order at a time."
        ),
    )


# ═════════════════════════════════════════════════════════════════════════════
# Launch
# ═════════════════════════════════════════════════════════════════════════════

st.divider()

if st.button(
    label="Run experiment",
    icon=":material/play_circle:",
    type="primary",
    disabled=not (symbols and interval and (full_history or (start_date and end_date))),
    shortcut="Enter",
    width="stretch",
):
    if not full_history and end_date > datetime.now().date():
        st.error("End date cannot be in the future.", icon=":material/error:")
    elif not full_history and start_date > end_date:  # ty:ignore[unsupported-operator]
        st.error("Start date must be equal or prior to end date.", icon=":material/error:")
    else:
        display_name = experiment_name or "(unnamed)"
        base_cur = st.session_state.get("base_currency", Currency.get_default())
        date_range = f"{start_date} → {end_date}" if not full_history else "full history"
        with st.spinner(f'Running "{display_name}"...'):
            # TODO: implement backtest execution logic
            st.success(
                f"Backtest **{display_name}** queued successfully — "
                f"{len(symbols)} symbol(s), {date_range}, "
                f"starting cash {base_cur} {starting_amount:,.2f}.",
                icon=":material/check_circle:",
            )

