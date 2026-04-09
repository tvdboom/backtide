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
from backtide.ui.utils import (
    _get_asset_type_description,
    _list_symbols,
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

FEE_MODES = ["Percentage (%)", "Fixed amount", "Percentage + Fixed"]

st.set_page_config(page_title="Backtide - Experiment", layout="centered")
st.title("Experiment", text_alignment="center")

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
    col1, col2 = st.columns([5, 1], vertical_alignment="bottom")

    currency_options = Currency.variants()
    base_default = Currency.get_default()

    base_currency = st.session_state.get("base_currency", base_default)
    starting_amount = col1.number_input(
        label="Initial cash",
        min_value=10**-base_currency.decimals,
        value=10_000.0,
        step=1_000.0,
        format="%.2f",
        placeholder="Insert the initial cash...",
        help="Cash balance available at the start of the simulation.",
    )

    base_currency = col2.selectbox(
        label="Base currency",
        key="base_currency",
        options=currency_options,
        format_func=lambda c: f"{c} — {c.name}",
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
# 5. Exchange
# ═════════════════════════════════════════════════════════════════════════════

with tab5:
    base_cur = st.session_state.get("base_currency", Currency.get_default())

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
