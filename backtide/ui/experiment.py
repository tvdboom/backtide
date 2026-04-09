"""Backtide.

Author: Mavs
Description: Run a new backtest page.

"""

import ast
from datetime import datetime
import json
import time

from code_editor import code_editor
import streamlit as st
import yaml

from backtide.data import AssetType, list_assets
from backtide.ui.utils import (
    _get_asset_type_description,
    _prevent_deselection,
    _to_upper_values,
)
from backtide.utils.constants import (
    MAX_ASSET_SELECTION,
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

FEE_MODES = ["Percentage (%)", "Fixed amount"]

st.set_page_config(page_title="Backtide - Experiment", layout="centered")
st.title("Experiment", text_alignment="center")

st.text(
    """
    Run a new backtest experiment on historical data for one or more symbols. The results
    of the experiment are automatically stored and can be reviewed in the results page.
    """,
)

tab1, tab2, tab3, tab4, tab5, tab6 = st.tabs(
    [
        ":material/dashboard: Overview",
        ":material/analytics: Data",
        ":material/account_balance_wallet: Portfolio",
        ":material/psychology: Strategy",
        ":material/storefront: Exchange",
        ":material/build: Engine",
    ],
)

if not st.session_state.get("asset_type"):
    _cache = st.session_state.get("_cache", {})
    st.session_state.asset_type = _cache.get("asset_type", AssetType.get_default())

if not st.session_state.get(f"all_assets_{asset_type}"):
    with st.spinner("Loading assets..."):
        st.session_state[f"all_assets_{asset_type}"] = list_assets(
            st.session_state.asset_type,
            MAX_PRELOADED_ASSETS,
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
        type=["yaml", "yml", "json"],
        help="Upload a YAML or JSON file to pre-fill the experiment configuration.",
    )

    if uploaded is not None:
        try:
            if uploaded.name.endswith(".json"):
                config = json.load(uploaded)
            else:
                config = yaml.safe_load(uploaded)

            experiment = Experiment(**config)
            st.session_state["experiment_name"] = experiment.name
            st.session_state["tags"] = experiment.tags
            st.session_state["description"] = experiment.description
            st.success(f"Loaded configuration from `{uploaded.name}`.")
        except (yaml.YAMLError, json.JSONDecodeError, TypeError) as ex:
            st.error(f"Failed to parse file: {ex}")


# ═════════════════════════════════════════════════════════════════════════════
# 2. Data
# ═════════════════════════════════════════════════════════════════════════════

with tab2:
    asset_type = st.segmented_control(
        label="Asset type",
        key="asset_type",
        options=AssetType,
        format_func=lambda asset_type: f"{asset_type.icon()} {asset_type.value}",
        on_change=_prevent_deselection(
            key="asset_type",
            default=AssetType.default(),
            reset=["symbols", "currency"],
        ),
        help="Select the type of financial asset you want to backtest.",
    )

    # Filter assets based on the selected currency
    if currency := st.session_state.get("currency"):
        assets = {
            k: v for k, v in all_assets.items() if currency == "All" or v.currency == currency
        }
    else:
        assets = all_assets

    col1, col2 = st.columns([5, 1], vertical_alignment="bottom")
    symbol_d, currency_d = _get_asset_type_description(st.session_state.asset_type)

    symbols = col1.multiselect(
        label="Symbols",
        key="symbols",
        options=sorted([asset.symbol for asset in assets.values()]),
        format_func=lambda x: f"{x} - {assets[x].name}" if asset_type != AssetType.CRYPTO else x,
        placeholder="Select one or more symbols...",
        max_selections=MAX_ASSET_SELECTION,
        accept_new_options=True,
        on_change=_to_upper_values("symbols"),
        help=symbol_d,
    )

    col2.selectbox(
        label="Currency",
        key="currency",  # Use key to filter tickers
        options=[
            "All",
            *sorted(dict.fromkeys(asset.currency for asset in all_assets.values())),
        ],
        placeholder="All",
        help=currency_d,
    )

    col1, col2 = st.columns(2)

    start_date = col1.date_input(
        label="Start date",
        value=None,
        min_value="2000-01-01",
        max_value=datetime.now().date(),
        help=(
            "Run backtest simulation starting from this date (inclusive). If the historical "
            "data does not go so far back, it starts from the available history for that ticker."
        ),
    )

    end_date = col2.date_input(
        label="End date",
        value="today",
        min_value=start_date,
        max_value="today",
        help="Run backtest simulation up to this date (inclusive).",
    )

    interval = st.pills(
        label="Interval",
        options=Interval,
        format_func=lambda x: x.value,
        selection_mode="single",
        default=Interval.OneHour,
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
    col1, col2 = st.columns([5, 1], vertical_alignment="bottom")

    if asset_type != AssetType.CRYPTO:
        base = CURRENCIES["USD"]
        options = CURRENCIES.values()
    else:
        base = CRYPTOS["USDT"]
        options = sorted(list(CURRENCIES.values()) + list(CRYPTOS.values()), key=lambda x: x.name)

    base_currency = st.session_state.get("base_currency", base)
    starting_amount = col1.number_input(
        label="Initial cash",
        min_value=1 / base_currency.decimals,
        value=10_000.0,
        step=1_000.0,
        format="%.2f",
        placeholder="Insert the initial cash...",
        help="Cash balance available at the start of the simulation.",
    )

    base_currency = col2.selectbox(
        label="Base currency",
        key="base_currency",
        options=options,
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
                data=[{"Symbol": symbol, "Quantity": 0.0} for symbol in symbols],
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
                tree = ast.parse(strategy_code)

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
# 5. Exchange
# ═════════════════════════════════════════════════════════════════════════════

# fee_mode_col, fee_val_col, slippage_col = st.columns(3)
#
# fee_mode = fee_mode_col.radio(
#     label="Fee type",
#     options=FEE_MODES,
#     index=0,
#     horizontal=False,
#     help=(
#         "How trading fees are calculated. *Percentage* charges a fraction of the trade "
#         "notional value; *Fixed amount* charges a flat fee per order."
#     ),
# )
#
# is_pct_fee = fee_mode == FEE_MODES[0]
#
# fee_value = fee_val_col.number_input(
#     label=f"Fee ({'%' if is_pct_fee else quote_currency} per trade)",
#     min_value=0.0,
#     max_value=100.0 if is_pct_fee else None,
#     value=0.1 if is_pct_fee else 1.0,
#     step=0.01 if is_pct_fee else 0.5,
#     format="%.4f",
#     help=(
#         "Fee charged per executed order. Applied as a percentage of notional value "
#         if is_pct_fee
#         else f"Fee charged per executed order as a fixed amount in {quote_currency}."
#     ),
# )
#
# slippage = slippage_col.number_input(
#     label="Slippage (% of price per trade)",
#     min_value=0.0,
#     max_value=100.0,
#     value=0.05,
#     step=0.01,
#     format="%.4f",
#     help=(
#         "Simulated market impact. Each fill price is moved adversely by this percentage "
#         "(buys filled higher, sells filled lower)."
#     ),
# )
#
#
# # ══════════════════════════════════════════════════════════════════════════════
# # 4 · STRATEGY
# # ══════════════════════════════════════════════════════════════════════════════
#
# # ── Indicators ───────────────────────────────────────────────────────────────
#
# indicator_toggle_col, indicator_select_col = st.columns([1, 3], vertical_alignment="bottom")
#
# use_indicators = indicator_toggle_col.toggle(
#     label="Enable indicators",
#     value=True,
#     help="Pre-compute technical indicators and make them available inside your strategy function.",
# )
#
# if use_indicators:
#     selected_indicators = indicator_select_col.multiselect(
#         label="Indicators",
#         options=INDICATORS,
#         default=["SMA - Simple Moving Average", "EMA - Exponential Moving Average"],
#         placeholder="Select indicators...",
#         help="Chosen indicators are computed before each strategy call and passed via the `indicators` dict.",
#     )
#
#     quick_col_all, quick_col_none, _ = st.columns([1, 1, 6])
#     if quick_col_all.button("Select all", use_container_width=True):
#         selected_indicators = INDICATORS
#     if quick_col_none.button("Clear", use_container_width=True):
#         selected_indicators = []
# else:
#     selected_indicators = []
#
# st.markdown("")  # spacing
#
#
# # ══════════════════════════════════════════════════════════════════════════════
# # LAUNCH
# # ══════════════════════════════════════════════════════════════════════════════
#
# st.divider()
#
# ready = all([bt_name, tickers, start_date, end_date, intervals, strategy_code])
#
# if not ready:
#     missing = [
#         label
#         for label, ok in [
#             ("experiment name", bool(bt_name)),
#             ("symbols", bool(tickers)),
#             ("start date", bool(start_date)),
#             ("intervals", bool(intervals)),
#             ("strategy", bool(strategy_code)),
#         ]
#         if not ok
#     ]
#     st.warning(
#         f"Please provide: {', '.join(missing)}.",
#         icon=":material/warning:",
#     )

st.divider()

if st.button(
    label="Run experiment",
    icon=":material/play_circle:",
    type="primary",
    disabled=not (symbols and start_date and end_date and interval),
    width="stretch",
):
    if end_date > datetime.now().date():
        st.error("End date cannot be in the future.", icon=":material/error:")
    elif start_date > end_date:  # ty:ignore[unsupported-operator]
        st.error("Start date must be equal or prior to end date.", icon=":material/error:")
    else:
        with st.spinner(f'Running "{bt_name}"...'):
            # TODO: implement backtest execution logic
            st.success(
                f"Backtest **{bt_name}** queued successfully — "
                f"{len(tickers)} symbol(s), {start_date} → {end_date}, "
                f"starting cash {quote_currency} {starting_amount:,.2f}.",
                icon=":material/check_circle:",
            )
