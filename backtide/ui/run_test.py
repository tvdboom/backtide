"""Backtide.

Author: Mavs
Description: Run a new backtest page.

"""

from datetime import datetime
from typing import Any

import streamlit as st
from code_editor import code_editor

from backtide.assets.assets import Asset, AssetType
from backtide.constants import MAX_ASSET_SELECTION
from backtide.models.ui import Interval
from backtide.utils.utils import format_compact


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

STRATEGY_PLACEHOLDER = """\
    # Available objects:
    #   data   - dict[str, pd.DataFrame]  (symbol → OHLCV dataframe)
    #   state  - portfolio state snapshot
    #   indicators - pre-computed indicator values (if selected above)
    #
    # Return a list of Order objects, e.g.:
    #   return [Order(symbol="AAPL", side="buy", qty=10)]

    def strategy(data, state, indicators):
        orders = []
        # ── Write your logic here ──────────────────────────

        return orders
    """


FEE_MODES = ["Percentage (%)", "Fixed amount"]
QUOTE_CURRENCIES = ["USD", "EUR", "GBP", "JPY", "CHF", "AUD", "CAD", "USDT"]
STRATEGY_SOURCES = ["Code editor", "Upload file"]


def _section(icon: str, title: str):
    """Render a styled section header."""
    st.markdown(f"### {icon} {title}")
    st.divider()


st.set_page_config(page_title="Backtide - Run test", layout="centered")
st.title("Run test", text_alignment="center")
st.divider()


_section(":material/tune:", "Metadata")

bt_name = st.text_input(
    label="Experiment name",
    placeholder="e.g. SMA crossover - FAANG - 2020/2024",
    max_chars=120,
    help="A human-readable name to identify this backtest experiment.",
)


_section(":material/candlestick_chart:", "Symbols")

asset_type = st.segmented_control(
    label="Asset type",
    options=AssetType,
    key="asset_type",
    format_func=lambda at: f"{at.icon()} {at.value}",
    on_change=_prevent_deselection("asset_type", AssetType.STOCKS),
    help="Select the type of financial asset to run the backtest on.",
)

with st.spinner("Fetching symbols..."):
    all_assets = st.session_state.asset_type.list_symbols()

# Currency pre-filter
if currency := st.session_state.get("bt_currency"):
    assets = [a for a in all_assets if currency == "All" or a.currency == currency]
else:
    assets = all_assets

match st.session_state.asset_type:
    case AssetType.STOCKS:
        symbol_description = (
            "Yahoo stock tickers to include in the backtest. The preloaded options cover "
            "primary listings in major indices; any valid yahoo ticker can be added."
        )
        curr_description = "Filter tickers by their denominated currency."
    case AssetType.FOREX:
        symbol_description = (
            "Currency pairs to include. The preloaded options are frequently traded pairs; "
            "any valid yahoo forex ticker can be added."
        )
        curr_description = "Filter pairs by their quote currency."
    case AssetType.ETF:
        symbol_description = (
            "Yahoo ETF tickers to include. The preloaded options are frequently traded "
            "ETFs and funds; any valid yahoo ETF ticker can be added."
        )
        curr_description = "Filter tickers by their denominated currency."
    case AssetType.CRYPTO:
        symbol_description = (
            "Crypto pairs to include. The preloaded options are frequently traded pairs; "
            "any valid yahoo ticker can be added."
        )
        curr_description = "Filter symbols by their quote currency."

ticker_col, filter_col = st.columns([3, 1], vertical_alignment="bottom")

tickers = ticker_col.multiselect(
    label="Symbols",
    options=sorted(assets, key=lambda x: x.symbol),
    format_func=_format_asset,
    placeholder="Select one or more symbols...",
    max_selections=MAX_ASSET_SELECTION,
    accept_new_options=True,
    help=symbol_description,
)

filter_col.selectbox(
    label="Currency",
    key="bt_currency",
    options=["All", *sorted(dict.fromkeys(a.currency for a in all_assets))],
    placeholder="All",
    help=curr_description,
)

start_date_col, end_date_col, interval_col = st.columns([1, 1, 2])

start_date = start_date_col.date_input(
    label="Start date",
    value=None,
    min_value="1980-01-01",
    max_value=datetime.now().date(),
    help=(
        "Backtest start date (inclusive). If historical data doesn't reach this far "
        "back, the full available history is used instead."
    ),
)

end_date = end_date_col.date_input(
    label="End date",
    value="today",
    min_value=start_date,
    max_value="today",
    help="Backtest end date (inclusive).",
)

intervals = interval_col.pills(
    label="Interval",
    options=Interval,
    format_func=lambda x: x.value,
    selection_mode="multi",
    default=Interval.OneHour,
    help=(
        "Frequency of data points used in the simulation. Full history is only "
        "available for intervals >= 1d."
    ),
)

if tickers and start_date and end_date and intervals:
    n_days = (end_date - start_date).days + 1
    n_points = len(tickers) * sum(max(1, n_days * 24 * 60 / i.to_minutes()) for i in intervals)
    st.info(
        f"Simulation will use {format_compact(n_points)} data entries.", icon=":material/info:"
    )


# ══════════════════════════════════════════════════════════════════════════════
# 3 · PORTFOLIO & EXCHANGE
# ══════════════════════════════════════════════════════════════════════════════

_section(":material/account_balance_wallet:", "Portfolio & Exchange")

amount_col, currency_col = st.columns(2)

starting_amount = amount_col.number_input(
    label="Starting cash",
    min_value=0.0,
    value=10_000.0,
    step=1_000.0,
    format="%.2f",
    help="Initial cash balance available at the start of the simulation.",
)

quote_currency = currency_col.selectbox(
    label="Quote currency",
    options=QUOTE_CURRENCIES,
    index=0,
    help=(
        "The currency in which P&L, fees, and cash balances are denominated. "
        "Asset prices will be converted to this currency where needed."
    ),
)

# Starting positions
with st.expander("Starting positions (optional)"):
    st.caption(
        "Pre-load the portfolio with existing holdings at the start of the simulation. "
        "Each row represents one position."
    )
    positions_data = st.data_editor(
        data=[{"Symbol": "", "Quantity": 0.0, "Avg. cost": 0.0}],
        num_rows="dynamic",
        use_container_width=True,
        column_config={
            "Symbol": st.column_config.TextColumn(
                "Symbol", help="Ticker symbol, e.g. AAPL", width="medium"
            ),
            "Quantity": st.column_config.NumberColumn(
                "Quantity",
                help="Number of units held (negative for short)",
                min_value=None,
                format="%.4f",
            ),
            "Avg. cost": st.column_config.NumberColumn(
                "Avg. cost",
                help=f"Average entry price in {quote_currency}",
                min_value=0.0,
                format="%.4f",
            ),
        },
    )

fee_mode_col, fee_val_col, slippage_col = st.columns(3)

fee_mode = fee_mode_col.radio(
    label="Fee type",
    options=FEE_MODES,
    index=0,
    horizontal=False,
    help=(
        "How trading fees are calculated. *Percentage* charges a fraction of the trade "
        "notional value; *Fixed amount* charges a flat fee per order."
    ),
)

is_pct_fee = fee_mode == FEE_MODES[0]

fee_value = fee_val_col.number_input(
    label=f"Fee ({'%' if is_pct_fee else quote_currency} per trade)",
    min_value=0.0,
    max_value=100.0 if is_pct_fee else None,
    value=0.1 if is_pct_fee else 1.0,
    step=0.01 if is_pct_fee else 0.5,
    format="%.4f",
    help=(
        "Fee charged per executed order. Applied as a percentage of notional value "
        if is_pct_fee
        else f"Fee charged per executed order as a fixed amount in {quote_currency}."
    ),
)

slippage = slippage_col.number_input(
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


# ══════════════════════════════════════════════════════════════════════════════
# 4 · STRATEGY
# ══════════════════════════════════════════════════════════════════════════════

_section(":material/psychology:", "Strategy")

# ── Indicators ───────────────────────────────────────────────────────────────

indicator_toggle_col, indicator_select_col = st.columns([1, 3], vertical_alignment="bottom")

use_indicators = indicator_toggle_col.toggle(
    label="Enable indicators",
    value=True,
    help="Pre-compute technical indicators and make them available inside your strategy function.",
)

if use_indicators:
    selected_indicators = indicator_select_col.multiselect(
        label="Indicators",
        options=INDICATORS,
        default=["SMA - Simple Moving Average", "EMA - Exponential Moving Average"],
        placeholder="Select indicators...",
        help="Chosen indicators are computed before each strategy call and passed via the `indicators` dict.",
    )

    quick_col_all, quick_col_none, _ = st.columns([1, 1, 6])
    if quick_col_all.button("Select all", use_container_width=True):
        selected_indicators = INDICATORS
    if quick_col_none.button("Clear", use_container_width=True):
        selected_indicators = []
else:
    selected_indicators = []

st.markdown("")  # spacing

# ── Strategy source ───────────────────────────────────────────────────────────

strategy_source = st.radio(
    label="Strategy source",
    options=STRATEGY_SOURCES,
    index=0,
    horizontal=True,
    help="Provide your strategy as inline code or by uploading a `.py` file.",
)

strategy_code: str | None = None

if strategy_source == STRATEGY_SOURCES[0]:
    st.caption(
        "Write your strategy function below. The editor supports Python syntax highlighting, "
        "autocompletion hints, and vim/emacs key bindings."
    )
    strategy_code = code_editor(
        value=STRATEGY_PLACEHOLDER,
        language="python",
        theme="tomorrow_night",
        font_size=14,
        tab_size=4,
        key="strategy_editor",
    )

else:
    uploaded_file = st.file_uploader(
        label="Strategy file",
        type=["py"],
        accept_multiple_files=False,
        help="Upload a `.py` file that defines a top-level `strategy(data, state, indicators)` function.",
    )
    if uploaded_file is not None:
        strategy_code = uploaded_file.read().decode("utf-8")
        with st.expander("Preview uploaded file"):
            st.code(strategy_code, language="python", line_numbers=True)
    else:
        st.info("No file uploaded yet.", icon=":material/upload_file:")


# ══════════════════════════════════════════════════════════════════════════════
# LAUNCH
# ══════════════════════════════════════════════════════════════════════════════

st.divider()

ready = all([bt_name, tickers, start_date, end_date, intervals, strategy_code])

if not ready:
    missing = [
        label
        for label, ok in [
            ("experiment name", bool(bt_name)),
            ("symbols", bool(tickers)),
            ("start date", bool(start_date)),
            ("intervals", bool(intervals)),
            ("strategy", bool(strategy_code)),
        ]
        if not ok
    ]
    st.warning(
        f"Please provide: {', '.join(missing)}.",
        icon=":material/warning:",
    )

if st.button(
    label="Run Backtest",
    icon=":material/play_circle:",
    type="primary",
    disabled=not ready,
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
