"""Backtide.

Author: Mavs
Description: Run a new backtest page.

"""

import ast
from datetime import datetime
import json
import tomllib
import uuid

from code_editor import code_editor
import streamlit as st
import yaml

from backtide.backtest import (
    CommissionType,
    ConversionPeriod,
    CurrencyConversionMode,
    EmptyBarPolicy,
    IndicatorType,
    OrderType,
    StrategyType,
)
from backtide.config import get_config
from backtide.data import AssetType, Currency, Interval
from backtide.ui.utils import (
    _get_asset_type_description,
    _list_symbols,
    _prevent_deselection,
    _to_upper_values,
)
from backtide.utils.constants import (
    INDICATOR_PLACEHOLDER,
    MAX_ASSET_SELECTION,
    STRATEGY_PLACEHOLDER,
    TAG_PATTERN,
)

cfg = get_config()

# Generate a stable experiment GUID for this session (regenerated only on explicit reset)
if "experiment_guid" not in st.session_state:
    st.session_state.experiment_guid = str(uuid.uuid4())

st.set_page_config(page_title="Backtide - Experiment", layout="centered")
st.title("Experiment", text_alignment="center")

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
)


# ═════════════════════════════════════════════════════════════════════════════
# 1. General
# ═════════════════════════════════════════════════════════════════════════════

with tab1:
    experiment_name = st.text_input(
        label="Experiment name",
        placeholder=st.session_state.experiment_guid,
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
        height=200,
        max_chars=1500,
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
    if st.session_state.get("asset_type") is None:
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
                    {"Symbol": a.symbol if hasattr(a, "symbol") else str(a), "Quantity": 0}
                    for a in symbols
                ],
                num_rows="fixed",
                hide_index=True,
                column_config={
                    "Symbol": st.column_config.TextColumn("Symbol", width="medium", disabled=True),
                    "Quantity": st.column_config.NumberColumn(
                        "Quantity",
                        min_value=0,
                    ),
                },
            )
        else:
            st.caption("No symbols selected.")


# ═════════════════════════════════════════════════════════════════════════════
# 4. Strategy
# ═════════════════════════════════════════════════════════════════════════════

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
        options=StrategyType.variants(),
        format_func=lambda s: s.name,
        default=[],
        placeholder="Select strategies...",
        help="Choose built-in strategies to run alongside your custom ones.",
    )

    if selected_predefined:
        with st.expander("Strategy descriptions", icon=":material/info:"):
            for strategy in selected_predefined:
                category = "Portfolio Rotation" if strategy.is_rotation else "Single asset"
                st.markdown(f"**{strategy.name}** · _{category}_")
                st.caption(strategy.description())

    st.divider()

    if "custom_strategies" not in st.session_state:
        st.session_state.custom_strategies = []

    st.markdown("**Custom strategies**")
    st.caption(
        "Add one or more custom strategy functions. Each strategy is evaluated "
        "independently during the simulation.",
    )

    custom_strategy_codes: list[str] = []

    for i, strategy_entry in enumerate(st.session_state.custom_strategies):
        with st.container(border=True):
            header_col, remove_col = st.columns([5, 1], vertical_alignment="center")
            header_col.text_input(
                label="Strategy name",
                key=f"strategy_name_{i}",
                placeholder=f"Strategy {i + 1}",
                label_visibility="collapsed",
            )

            if remove_col.button(
                label="Remove",
                key=f"remove_strategy_{i}",
                icon=":material/close:",
                type="tertiary",
            ):
                st.session_state.custom_strategies.pop(i)
                st.rerun()

            source = st.segmented_control(
                label="Source",
                key=f"strategy_source_{i}",
                options=[":material/code: Code editor", ":material/upload_file: Upload file"],
                default=strategy_entry.get("source", ":material/code: Code editor"),
                label_visibility="collapsed",
            )

            strategy_code: str | None = None
            if source == ":material/code: Code editor":
                resp = code_editor(
                    code=strategy_entry.get("code") or STRATEGY_PLACEHOLDER,
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
                strategy_code = resp["text"]
            else:
                uploaded_file = st.file_uploader(
                    label="Strategy file",
                    key=f"strategy_file_{i}",
                    type=["py"],
                    accept_multiple_files=False,
                    label_visibility="collapsed",
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
                if _check_strategy_code(strategy_code, i):
                    custom_strategy_codes.append(strategy_code)

    if st.button(
        label="Add strategy",
        icon=":material/add:",
        type="secondary",
    ):
        st.session_state.custom_strategies.append(
            {"source": ":material/code: Code editor", "code": ""},
        )
        st.rerun()


# ═════════════════════════════════════════════════════════════════════════════
# 5. Indicators
# ═════════════════════════════════════════════════════════════════════════════

with tab5:
    st.caption(
        "Indicators are mathematical functions applied to price and volume data that "
        "quantify trends, momentum, volatility and other market characteristics. The "
        "computed values can then be sued in your strategy to make investment decisions. "
        "All selected indicators are computed up-front over the full dataset before the "
        "simulation begins, so they add no per-tick overhead.",
    )

    selected_indicators = st.multiselect(
        label="Built-in indicators",
        options=IndicatorType.variants(),
        format_func=lambda i: f"{i} - {i.name}",
        default=[],
        placeholder="Select indicators...",
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
        "Add one or more indicator functions. Each function's return values "
        "are merged with the built-in indicators and passed to your strategy.",
    )

    custom_indicator_codes: list[str] = []

    for i, indicator_entry in enumerate(st.session_state.custom_indicators):
        with st.container(border=True):
            header_col, remove_col = st.columns([5, 1], vertical_alignment="center")
            header_col.text_input(
                label="Indicator name",
                key=f"indicator_name_{i}",
                placeholder=f"Indicator {i + 1}",
                label_visibility="collapsed",
            )

            if remove_col.button(
                label="Remove",
                key=f"remove_indicator_{i}",
                icon=":material/close:",
                type="tertiary",
            ):
                st.session_state.custom_indicators.pop(i)
                st.rerun()

            source = st.segmented_control(
                label="Source",
                key=f"indicator_source_{i}",
                options=[":material/code: Code editor", ":material/upload_file: Upload file"],
                default=indicator_entry.get("source", ":material/code: Code editor"),
                label_visibility="collapsed",
            )

            code: str | None = None
            if source == ":material/code: Code editor":
                resp = code_editor(
                    code=indicator_entry.get("code") or INDICATOR_PLACEHOLDER,
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
                    type=["py"],
                    accept_multiple_files=False,
                    label_visibility="collapsed",
                    help=(
                        "Upload a Python file that defines a top-level function with signature: "
                        "`indicator(data)` returning `dict[str, float]`."
                    ),
                )

                if indicator_file is not None:
                    code = indicator_file.read().decode("utf-8")
                    with st.expander("Preview uploaded file"):
                        st.code(code, language="python", line_numbers=True)
                else:
                    st.info("No file uploaded yet.", icon=":material/upload_file:")

            if code:
                if _check_indicator_code(code, i):
                    custom_indicator_codes.append(code)

    if st.button(
        label="Add indicator",
        icon=":material/add:",
        type="secondary",
    ):
        st.session_state.custom_indicators.append(
            {"source": ":material/code: Code editor", "code": ""},
        )
        st.rerun()


# ═════════════════════════════════════════════════════════════════════════════
# 6. Exchange
# ═════════════════════════════════════════════════════════════════════════════

with tab6:
    base_cur = st.session_state.get("base_currency", Currency.get_default())

    with st.container(border=True):
        st.markdown("**Commission**")

        col_radio, col_inputs = st.columns([2, 3], vertical_alignment="top")

        with col_radio:
            variants = CommissionType.variants()
            commission_mode = st.radio(
                label="Commission type",
                options=variants,
                index=variants.index(CommissionType.get_default()),
                horizontal=False,
                help=(
                    "How trading commissions are calculated. **Percentage** charges a fraction "
                    "of the trade notional value. **Fixed amount** charges a flat commission per "
                    "order. **Percentage + Fixed** applies both a percentage-based and a flat "
                    "commission to every trade."
                ),
            )

        is_pct = commission_mode == CommissionType("Percentage")
        is_fixed = commission_mode == CommissionType("Fixed")

        with col_inputs:
            if is_pct:
                commission_pct = st.number_input(
                    label="Commission (% per trade)",
                    min_value=0.0,
                    max_value=100.0,
                    value=0.1,
                    step=0.01,
                    format="%.2f",
                    help=(
                        "Commission charged per executed order, applied as a percentage of "
                        "the trade's notional value."
                    ),
                )
                commission_fixed = 0.0
            elif is_fixed:
                commission_fixed = st.number_input(
                    label=f"Commission ({base_cur} per trade)",
                    min_value=0.0,
                    value=1.0,
                    step=0.5,
                    format="%.2f",
                    help=f"Flat commission charged per executed order in {base_cur}.",
                )
                commission_pct = 0.0
            else:
                commission_pct = st.number_input(
                    label="Commission (% per trade)",
                    min_value=0.0,
                    max_value=100.0,
                    value=0.1,
                    step=0.01,
                    format="%.2f",
                    help="Percentage of the commission, applied to the trade's notional value.",
                )
                commission_fixed = st.number_input(
                    label=f"Commission ({base_cur} per trade)",
                    min_value=0.0,
                    value=1.0,
                    step=0.5,
                    format="%.2f",
                    help=(
                        f"Fixed portion of the commission in {base_cur}, added on top of the "
                        "percentage commission."
                    ),
                )

    with st.container(border=True):
        st.markdown("**Slippage**")

        slippage = st.number_input(
            label="Slippage (% of price per trade)",
            min_value=0.0,
            max_value=100.0,
            value=0.05,
            step=0.01,
            format="%.2f",
            help=(
                "Simulated market impact. Each fill price is moved adversely by this percentage "
                "(buys filled higher, sells filled lower)."
            ),
        )

    with st.container(border=True):
        st.markdown("**Order execution**")

        allowed_order_types = st.multiselect(
            label="Allowed order types",
            options=OrderType.variants(),
            default=[OrderType.get_default()],
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
            value=False,
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
            value=True,
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
                min_value=1.0,
                max_value=10.0,
                value=1.0,
                step=0.5,
                format="%.1f",
                help=(
                    "Maximum leverage ratio. A value of 2.0 means the strategy can borrow "
                    "up to 1x the portfolio value on top of its own capital. Exceeding this "
                    "limit raises a hard error."
                ),
            )

            col_im, col_mm = st.columns(2)

            initial_margin = col_im.number_input(
                label="Initial margin (%)",
                min_value=0.0,
                max_value=100.0,
                value=50.0,
                step=5.0,
                format="%.1f",
                help=(
                    "Minimum equity as a percentage of position value required when opening "
                    "a new leveraged position. For example, 50% means you must put up at "
                    "least half the position's value from your own capital."
                ),
            )

            maintenance_margin = col_mm.number_input(
                label="Maintenance margin (%)",
                min_value=0.0,
                max_value=100.0,
                value=25.0,
                step=5.0,
                format="%.1f",
                help=(
                    "Minimum equity as a percentage of position value that must be maintained. "
                    "If equity drops below this threshold a margin call is triggered."
                ),
            )

            margin_interest = st.number_input(
                label="Margin interest rate (% annual)",
                min_value=0.0,
                max_value=100.0,
                value=0.0,
                step=0.5,
                format="%.2f",
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
            value=True,
            help=(
                "Safety guardrail for short positions. When enabled (default), the strategy "
                "may open short positions if it chooses to — the actual decision is made in "
                "your strategy code. When disabled, any attempt to sell assets not currently "
                "held will raise a hard error and abort the simulation."
            ),
        )

        if allow_short_selling:
            borrow_rate = st.number_input(
                label="Borrow rate (% annual)",
                min_value=0.0,
                max_value=100.0,
                value=0.0,
                step=0.5,
                format="%.2f",
                help=(
                    "Annualized cost of borrowing shares for short positions. Accrued daily "
                    "and deducted from the portfolio cash balance."
                ),
            )

    with st.container(border=True):
        st.markdown("**Position limits**")

        max_position_size = st.number_input(
            label="Max position size (% of portfolio)",
            min_value=1,
            max_value=100,
            value=100,
            step=5,
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
            options=variants,
            format_func=lambda x: x.name,
            index=variants.index(CurrencyConversionMode.get_default()),
            help=(
                "Determines how proceeds in a foreign currency are converted back to "
                "the base currency. **Immediately** converts at the time of the trade. "
                "**Hold until threshold** keeps the foreign balance until it reaches a "
                "specified amount. **End of period** batches conversions at a chosen "
                "frequency (day, week or month). **Custom interval** lets you specify "
                "the number of bars between conversions."
            ),
        )

        if conversion_mode == CurrencyConversionMode("HoldUntilThreshold"):
            conversion_threshold = st.number_input(
                label=f"Conversion threshold ({base_cur})",
                min_value=0.0,
                value=1_000.0,
                step=100.0,
                format="%.2f",
                help=(
                    f"Foreign currency balances are converted to {base_cur} once their "
                    f"equivalent value reaches this threshold."
                ),
            )
        elif conversion_mode == CurrencyConversionMode("EndOfPeriod"):
            variants = ConversionPeriod.variants()
            conversion_period = st.selectbox(
                label="Conversion period",
                options=variants,
                index=variants.index(ConversionPeriod.get_default()),
                help="How often foreign currency balances are converted to the base currency.",
            )
        elif conversion_mode == CurrencyConversionMode("CustomInterval"):
            conversion_interval = st.number_input(
                label="Conversion interval (bars)",
                min_value=1,
                value=5,
                step=1,
                help=(
                    "Number of bars between automatic conversions of foreign currency "
                    "balances to the base currency."
                ),
            )


# ═════════════════════════════════════════════════════════════════════════════
# 7. Engine
# ═════════════════════════════════════════════════════════════════════════════

with tab7:
    with st.container(border=True):
        st.markdown("**Timing**")

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

        trade_on_close = st.toggle(
            label="Trade on close",
            value=False,
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

    with st.container(border=True):
        st.markdown("**Execution behaviour**")

        exclusive_orders = st.toggle(
            label="Exclusive orders",
            value=False,
            help=(
                "When enabled, submitting a new order automatically cancels all pending "
                "orders. Useful for strategies that should only have one active order at a time."
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

    with st.container(border=True):
        st.markdown("**Data handling**")

        variants = EmptyBarPolicy.variants()
        empty_bar_policy = st.selectbox(
            label="Empty bar policy",
            options=variants,
            format_func=lambda x: x.name,
            index=variants.index(EmptyBarPolicy.get_default()),
            help=(
                "How to handle bars with no trading activity (e.g. market closures during "
                "intraday backtests, holidays or illiquid periods).\n\n"
                "**Skip** — the bar is dropped entirely; the strategy is not called and "
                "the simulation clock jumps to the next bar with data.\n\n"
                "**Forward-fill** — OHLC values are copied from the last valid bar and "
                "volume is set to zero. The strategy runs as normal, which keeps a "
                "consistent tick cadence (recommended for most use cases).\n\n"
                "**Fill with NaN** — the bar is kept but all fields are set to NaN. "
                "Your strategy must handle missing values explicitly."
            ),
        )
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
        display_name = experiment_name or st.session_state.experiment_guid
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
