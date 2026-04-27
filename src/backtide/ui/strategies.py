"""Backtide.

Author: Mavs
Description: Strategy management page.

"""

from pathlib import Path

from code_editor import code_editor
import streamlit as st

from backtide.config import get_config
from backtide.strategies import BUILTIN_STRATEGIES
from backtide.strategies.utils import (
    _build_custom_strategy,
    _check_strategy_code,
    _get_strategy_label,
    _is_builtin_strategy,
    _load_stored_strategies,
    _save_strategy,
)
from backtide.ui.utils import (
    _CODE_OPTIONS,
    _default,
    _persist,
)
from backtide.utils.constants import INVALID_FILENAME_CHARS

# ─────────────────────────────────────────────────────────────────────────────
# Helper functionalities
# ─────────────────────────────────────────────────────────────────────────────

# Parameter schemas for built-in strategies.
STRATEGY_PARAMS: dict[str, dict[str, tuple]] = {
    "Adaptive RSI": {
        "min_period": ("Min period", 8, 2, 100, 1, "Minimum adaptive RSI period."),
        "max_period": ("Max period", 28, 2, 100, 1, "Maximum adaptive RSI period."),
    },
    "AlphaRSI Pro": {
        "period": ("Period", 14, 2, 500, 1, "RSI look-back period."),
        "vol_window": ("Vol. window", 20, 2, 500, 1, "Volatility window for level adjustment."),
    },
    "Bollinger Mean Reversion": {
        "period": ("Period", 20, 2, 500, 1, "Number of bars for the moving average."),
        "std_dev": (
            "Std. deviations",
            2.0,
            0.1,
            10.0,
            0.1,
            "Number of standard deviations for the bands.",
        ),
    },
    "Buy & Hold": {},
    "Double Top": {
        "lookback": ("Lookback", 60, 10, 500, 1, "Number of bars to search for the pattern."),
    },
    "Hybrid AlphaRSI": {
        "min_period": ("Min period", 8, 2, 100, 1, "Minimum adaptive RSI period."),
        "max_period": ("Max period", 28, 2, 100, 1, "Maximum adaptive RSI period."),
        "vol_window": ("Vol. window", 20, 2, 500, 1, "Volatility window for level adjustment."),
    },
    "MACD": {
        "fast_period": ("Fast period", 12, 2, 500, 1, "Number of bars for the fast EMA."),
        "slow_period": ("Slow period", 26, 2, 500, 1, "Number of bars for the slow EMA."),
        "signal_period": (
            "Signal period",
            9,
            2,
            500,
            1,
            "Number of bars for the signal line EMA.",
        ),
    },
    "Momentum": {
        "period": ("Period", 14, 2, 500, 1, "Look-back period for momentum."),
        "ma_period": ("MA period", 50, 2, 500, 1, "Moving average period for the trend filter."),
    },
    "Multi Bollinger Rotation": {
        "period": ("Period", 20, 2, 500, 1, "Bollinger Band moving average period."),
        "std_dev": ("Std. deviations", 2.0, 0.1, 10.0, 0.1, "Number of standard deviations."),
        "top_k": ("Top K", 5, 1, 100, 1, "Number of top-ranked assets to hold."),
        "rebalance_interval": ("Rebalance", 20, 1, 500, 1, "Bars between rebalancing."),
    },
    "Risk Averse": {
        "vol_period": ("Vol. period", 14, 2, 500, 1, "ATR look-back for volatility filter."),
        "breakout_period": ("Breakout period", 20, 2, 500, 1, "Bars for the new-high condition."),
    },
    "ROC": {
        "period": ("Period", 12, 2, 500, 1, "ROC look-back period."),
    },
    "ROC Rotation": {
        "period": ("Period", 12, 2, 500, 1, "ROC look-back period for ranking."),
        "top_k": ("Top K", 5, 1, 100, 1, "Number of top-ranked assets to hold."),
        "rebalance_interval": ("Rebalance", 20, 1, 500, 1, "Bars between rebalancing."),
    },
    "RSI": {
        "rsi_period": ("RSI period", 14, 2, 500, 1, "RSI look-back period."),
        "bb_period": ("BB period", 20, 2, 500, 1, "Bollinger Band moving average period."),
        "bb_std": ("BB std", 2.0, 0.1, 10.0, 0.1, "Number of standard deviations for the bands."),
    },
    "RSRS": {
        "period": ("Period", 18, 2, 500, 1, "Look-back window for the linear regression."),
    },
    "RSRS Rotation": {
        "period": ("Period", 18, 2, 500, 1, "RSRS look-back window for ranking."),
        "top_k": ("Top K", 5, 1, 100, 1, "Number of top-ranked assets to hold."),
        "rebalance_interval": ("Rebalance", 20, 1, 500, 1, "Bars between rebalancing."),
    },
    "SMA (Crossover)": {
        "fast_period": ("Fast period", 20, 2, 500, 1, "Fast moving average period."),
        "slow_period": ("Slow period", 50, 2, 500, 1, "Slow moving average period."),
    },
    "SMA (Naive)": {
        "period": ("Period", 20, 2, 500, 1, "Moving average period."),
    },
    "Triple RSI Rotation": {
        "short_period": ("Short period", 5, 2, 500, 1, "Short-term RSI period."),
        "medium_period": ("Medium period", 14, 2, 500, 1, "Medium-term RSI period."),
        "long_period": ("Long period", 28, 2, 500, 1, "Long-term RSI period."),
        "top_k": ("Top K", 5, 1, 100, 1, "Number of top-ranked assets to hold."),
        "rebalance_interval": ("Rebalance", 20, 1, 500, 1, "Bars between rebalancing."),
    },
    "Turtle Trading": {
        "entry_period": ("Entry period", 20, 2, 500, 1, "Bars for the entry breakout."),
        "exit_period": ("Exit period", 10, 2, 500, 1, "Bars for the exit breakdown."),
        "atr_period": ("ATR period", 20, 2, 500, 1, "ATR period for position sizing."),
    },
    "VCP": {
        "lookback": ("Lookback", 60, 10, 500, 1, "Bars to detect the contraction pattern."),
        "contractions": ("Contractions", 3, 1, 20, 1, "Minimum number of contracting ranges."),
    },
}

code_placeholder = lambda t: (
    f'''\
from backtide.strategies import BaseStrategy
from backtide.backtest import Order, Portfolio, State


class MyStrategy(BaseStrategy):
    def evaluate(self, data, portfolio, state, indicators):
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : {t}
            Historical OHLCV data with columns 'symbol', 'open', 'high',
            'low', 'close', 'adj_close' 'volume'.

        portfolio : Portfolio
            Current portfolio holdings (cash and positions).

        state : State
            Current simulation state.

        indicators: {t} | None
            Indicators calculated on the historical data. None if no
            indicators were selected.

        Returns
        -------
        list[Order]
            Orders to place this tick.

        """
        orders = []

        # ── Write your logic here ────────────────────────



        # ───────────────────────────────────────────────────

        return orders


MyStrategy()'''
)

# ─────────────────────────────────────────────────────────────────────────────
# Page interface
# ─────────────────────────────────────────────────────────────────────────────

st.set_page_config(page_title="Backtide - Strategies")

cfg = get_config()

storage_path = Path(cfg.data.storage_path) / "strategies"
storage_path.mkdir(parents=True, exist_ok=True)

st.subheader("Strategies", text_alignment="center")
st.write("")

if stored_strat := _load_stored_strategies(cfg):
    for name, strat in stored_strat.items():
        with st.container(border=True):
            col1, col2 = st.columns([5, 1], vertical_alignment="center")
            col1.markdown(_get_strategy_label(name, strat))

            if col2.button(
                label="Delete",
                key=f"delete_strat_{name}",
                icon=":material/delete:",
                type="tertiary",
            ):
                Path(storage_path, f"{name}.pkl").unlink(missing_ok=True)
                st.rerun()

            # Show source code expander for custom strategies
            if not _is_builtin_strategy(strat):
                with st.expander(
                    label="Source code",
                    key=(exp_key := f"strat_source_code_{name}"),
                    icon=":material/code:",
                    expanded=bool(_default(exp_key)),
                    on_change=lambda k=exp_key: _persist(k),
                ):
                    resp = code_editor(
                        code=(source := getattr(strat, "_source_code", "")),
                        key=f"edit_strat_code_{name}",
                        response_mode="debounce",
                        buttons=[
                            {
                                "name": "Copy",
                                "feather": "Copy",
                                "hasText": True,
                                "commands": ["copyAll"],
                                "style": {"top": "0.46rem", "right": "0.4rem"},
                            },
                        ],
                    )

                    edited_code = resp["text"]

                    if edited_code and edited_code != source:
                        if err := _check_strategy_code(edited_code):
                            st.error(err)
                        else:
                            if st.button(
                                "Save changes",
                                key=f"save_edit_strat_{name}",
                                icon=":material/save:",
                                type="primary",
                            ):
                                try:
                                    new_instance = _build_custom_strategy(edited_code)
                                    _save_strategy(new_instance, name, cfg)
                                    st.session_state[f"_{exp_key}"] = False
                                    st.rerun()
                                except Exception as ex:  # noqa: BLE001
                                    st.error(f"Failed to rebuild strategy. {ex}")
else:
    st.info("There are no saved strategies.", icon=":material/info:")


col1, col2 = st.columns(2)

if col1.button(
    "Add built-in",
    icon=":material/psychology:",
    type="secondary",
    width="stretch",
):
    st.session_state["_add_strategy_mode"] = "builtin"

if col2.button(
    "Add custom",
    icon=":material/code:",
    type="secondary",
    width="stretch",
):
    st.session_state["_add_strategy_mode"] = "custom"

mode = st.session_state.get("_add_strategy_mode")

if mode == "builtin":
    with st.container(border=True):
        col1, col2 = st.columns([3, 2])

        def _on_builtin_type_change(k):
            """Sync name to the newly selected strategy's name."""
            _persist(k)
            if new_strat := st.session_state.get(k):
                st.session_state["new_builtin_strat_name"] = new_strat.name
                st.session_state["_new_builtin_strat_name"] = new_strat.name

        strat = col1.selectbox(
            label="Strategy",
            key=(key := "new_builtin_strat_type"),
            options=BUILTIN_STRATEGIES,
            index=BUILTIN_STRATEGIES.index(_default(key, BUILTIN_STRATEGIES[0])),
            format_func=lambda s: s.name,
            on_change=lambda k=key: _on_builtin_type_change(k),
        )

        strat_name = col2.text_input(
            label="Name",
            key=(key := "new_builtin_strat_name"),
            value=_default(key, strat.name),
            max_chars=40,
            on_change=lambda k=key: _persist(k),
            help="A unique name for this strategy.",
        )

        params = {}
        if params_schema := STRATEGY_PARAMS.get(strat.name):
            cols = st.columns(len(params_schema))
            for col, (param, (label, default, min_v, max_v, step, help_text)) in zip(
                cols, params_schema.items(), strict=True
            ):
                params[param] = col.number_input(
                    label=label,
                    key=f"new_builtin_strat_{param}",
                    value=default,
                    min_value=min_v,
                    max_value=max_v,
                    step=step,
                    format="%.1f" if isinstance(default, float) else None,
                    help=help_text,
                )

        category = "Multi-Asset" if strat.is_multi_asset else "Single Asset"
        st.markdown(f"_{category}_")
        st.caption(strat.description())

        name_error = None
        if not strat_name:
            name_error = "Name cannot be empty."
        elif chars := INVALID_FILENAME_CHARS.findall(strat_name):
            name_error = (
                f"The following characters are not allowed in strategy names: "
                f"**{' '.join(sorted(set(chars)))}** "
            )
        elif strat_name in stored_strat:
            name_error = f"A strategy with name **{strat_name}** already exists."

        if name_error:
            st.error(name_error)

        col1, col2 = st.columns(2)

        if col1.button(
            "Save",
            icon=":material/save:",
            type="primary",
            disabled=bool(name_error),
            width="stretch",
        ):
            cast_params = {}
            if params_schema:
                for k, v in params.items():
                    schema = params_schema.get(k)
                    if schema and isinstance(schema[1], int):
                        cast_params[k] = int(v)
                    else:
                        cast_params[k] = v

            _save_strategy(strat(**cast_params), strat_name, cfg)
            st.session_state.pop("_add_strategy_mode", None)
            st.rerun()

        if col2.button("Cancel", icon=":material/close:", width="stretch"):
            st.session_state.pop("_add_strategy_mode", None)
            st.rerun()

elif mode == "custom":
    with st.container(border=True):
        strat_name = st.text_input(
            label="Name",
            key=(key := "new_strat_custom_name"),
            value=_default(key, "MyStrategy"),
            max_chars=40,
            placeholder="Add a name...",
            on_change=lambda k=key: _persist(k),
            help="A unique name for this custom strategy.",
        )

        source = st.segmented_control(
            label="Source",
            key=(key := "new_custom_strat_source"),
            required=True,
            options=_CODE_OPTIONS,
            default=_default(key, _CODE_OPTIONS[0]),
            label_visibility="collapsed",
            on_change=lambda k=key: _persist(k),
        )

        code: str | None = None
        if source == _CODE_OPTIONS[0]:
            resp = code_editor(
                key=(key := "strat_custom_code_editor"),
                code=_default(key, code_placeholder(cfg.data.dataframe_library.class_name)),
                response_mode="debounce",
                buttons=[
                    {
                        "name": "Copy",
                        "feather": "Copy",
                        "hasText": True,
                        "commands": ["copyAll"],
                        "style": {"top": "0.46rem", "right": "0.4rem"},
                    },
                ],
            )

            st.session_state[f"_{key}"] = resp["text"]
            code = resp["text"]
        else:
            st.caption(
                "The uploaded file must contain a class that inherits from `BaseStrategy` "
                "with an `evaluate(self, data, portfolio, state, indicators)` method."
            )

            strategy_file = st.file_uploader(
                label="Strategy file",
                key="new_custom_strat_file",
                type="py",
                accept_multiple_files=False,
                label_visibility="collapsed",
            )

            if strategy_file is not None:
                code = strategy_file.read().decode("utf-8")
                with st.expander("Preview uploaded file"):
                    st.code(code, language="python", line_numbers=True)
            else:
                st.info("No file uploaded yet.", icon=":material/upload_file:")

        if code:
            if err := _check_strategy_code(code):
                st.error(err)

        name_error = None
        if not strat_name:
            name_error = "Name cannot be empty."
        elif INVALID_FILENAME_CHARS.findall(strat_name):
            chars = INVALID_FILENAME_CHARS.findall(strat_name)
            name_error = f"Invalid characters: {' '.join(repr(c) for c in sorted(set(chars)))}"
        elif strat_name in stored_strat:
            name_error = f"A strategy with name **{strat_name}** already exists."

        if name_error:
            st.error(name_error)

        col1, col2 = st.columns(2)

        if col1.button(
            "Save",
            icon=":material/save:",
            type="primary",
            disabled=bool(name_error) or not code,
            width="stretch",
        ):
            try:
                instance = _build_custom_strategy(code)  # ty: ignore[invalid-argument-type]
                _save_strategy(instance, strat_name, cfg)
                st.session_state.pop("_add_strategy_mode", None)
                st.rerun()
            except Exception as ex:  # noqa: BLE001
                st.error(f"Failed to build strategy. {ex}")

        if col2.button("Cancel", icon=":material/close:", width="stretch"):
            st.session_state.pop("_add_strategy_mode", None)
            st.rerun()
