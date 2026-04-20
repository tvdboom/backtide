"""Backtide.

Author: Mavs
Description: Indicator management page.

"""

import ast
from dataclasses import asdict, dataclass
import json
from pathlib import Path
from typing import Any

from code_editor import code_editor
import streamlit as st

from backtide.backtest import (
    AverageDirectionalIndex,
    AverageTrueRange,
    BollingerBands,
    CommodityChannelIndex,
    ExponentialMovingAverage,
    MovingAverageConvergenceDivergence,
    OnBalanceVolume,
    RelativeStrengthIndex,
    SimpleMovingAverage,
    StochasticOscillator,
    VolumeWeightedAveragePrice,
    WeightedMovingAverage,
)
from backtide.config import get_config
from backtide.ui.utils import (
    _CODE_OPTIONS,
    _default,
    _load_stored_indicators,
    _persist,
)
from backtide.utils.constants import INVALID_FILENAME_CHARS

# ─────────────────────────────────────────────────────────────────────────────
# Helper functionalities
# ─────────────────────────────────────────────────────────────────────────────

# Predefined indicator objects
PREDEFINED = [
    ExponentialMovingAverage,
    RelativeStrengthIndex,
    SimpleMovingAverage,
    WeightedMovingAverage,
    RelativeStrengthIndex,
    MovingAverageConvergenceDivergence,
    BollingerBands,
    AverageTrueRange,
    OnBalanceVolume,
    WeightedMovingAverage,
    RelativeStrengthIndex,
    MovingAverageConvergenceDivergence,
    BollingerBands,
    AverageTrueRange,
    OnBalanceVolume,
    VolumeWeightedAveragePrice,
    StochasticOscillator,
    CommodityChannelIndex,
    AverageDirectionalIndex,
]

# Parameter schemas for built-in indicators.
INDICATOR_PARAMS: dict[str, dict] = {
    "SMA": {"period": ("Period", 14, 2, 500, 1, "Number of bars for the moving average window.")},
    "EMA": {
        "period": (
            "Period",
            14,
            2,
            500,
            1,
            "Number of bars for the exponential moving average window.",
        )
    },
    "WMA": {
        "period": (
            "Period",
            14,
            2,
            500,
            1,
            "Number of bars for the weighted moving average window.",
        )
    },
    "RSI": {"period": ("Period", 14, 2, 500, 1, "Lookback period for the RSI calculation.")},
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
    "BB": {
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
    "ATR": {"period": ("Period", 14, 2, 500, 1, "Lookback period for the Average True Range.")},
    "OBV": {},
    "VWAP": {},
    "STOCH": {
        "k_period": ("%K period", 14, 2, 500, 1, "Lookback period for the %K line."),
        "d_period": ("%D period", 3, 2, 500, 1, "Smoothing period for the %D line."),
    },
    "CCI": {"period": ("Period", 20, 2, 500, 1, "Lookback period for the CCI calculation.")},
    "ADX": {"period": ("Period", 14, 2, 500, 1, "Lookback period for the ADX calculation.")},
}

CODE_PLACEHOLDER = """\
def indicator(data):
    '''Compute a custom indicator value for the current bar.

    Parameters
    ----------
    data : pd.DataFrame
        Historical OHLCV data up to and including the current bar.

    Returns
    -------
    dict[str, float]
        A mapping of indicator name(s) to their computed value(s).
        Example: {"my_signal": 0.75, "my_trend": 1.0}

    '''
    result = {}

    # ── Write your logic here ──────────────────────────

    return result
"""


@dataclass
class SavedIndicator:
    """A persisted indicator definition."""

    name: str
    builtin: str | None = None
    parameters: dict[str, Any] | None = None
    code: str | None = None


def _save_indicator(ind: SavedIndicator):
    """Persist an indicator definition to disk as JSON."""
    path = Path(cfg.data.storage_path) / "indicators" / f"{ind.name}.json"
    path.write_text(json.dumps(asdict(ind), indent=4), encoding="utf-8")


def _check_indicator_code(code: str) -> str | None:
    """Validate that `code` defines `indicator(data)`."""
    try:
        tree = ast.parse(code)
        for node in tree.body:
            if isinstance(node, ast.FunctionDef) and node.name == "indicator":
                if [a.arg for a in node.args.args] == ["data"]:
                    return None
                return "Function `indicator` doesn't have signature: `indicator(data)`."
        return "No function `indicator(data)` found in the code."
    except SyntaxError as ex:
        return f"Syntax error:\n\n{ex}"


def _make_custom_compute_fn(code: str):
    """Create a callable wrapper from custom indicator code."""
    ns: dict = {}
    exec(code, ns)
    fn = ns.get("indicator")
    if not callable(fn):
        raise ValueError("Custom indicator code must define a callable named 'indicator'.")

    class _CustomIndicator:
        """Wrapper to give custom code the same .compute() interface."""

        def __init__(self, func):
            self._func = func

        def compute(self, df):
            return self._func(df)

    return _CustomIndicator(fn)


# ─────────────────────────────────────────────────────────────────────────────
# Page interface
# ─────────────────────────────────────────────────────────────────────────────

st.set_page_config(page_title="Backtide - Indicators")

cfg = get_config()

storage_path = Path(cfg.data.storage_path) / "indicators"
storage_path.mkdir(parents=True, exist_ok=True)

stored_indicators = _load_stored_indicators(cfg)
stored_names = [ind.name for ind in stored_indicators]

st.subheader("Indicators", text_alignment="center")
st.write("")


if stored_indicators:
    for ind in stored_indicators:
        with st.container(border=True):
            col1, col2 = st.columns([5, 1], vertical_alignment="center")

            if ind.code:
                label = f":material/code: **{ind.name}** · Custom"
            else:
                label = f":material/show_chart: **{ind.name}** · _{ind.builtin}_"
                if ind.parameters:
                    label += f" · {', '.join(f'{k}={v}' for k, v in ind.parameters.items())}"

            col1.markdown(label)

            if col2.button(
                label="Delete",
                key=f"delete_ind_{ind.name}",
                icon=":material/delete:",
                type="tertiary",
            ):
                Path(storage_path, f"{ind.name}.json").unlink(missing_ok=True)
                st.rerun()
else:
    st.info("No saved indicators yet.", icon=":material/info:")


st.divider()

col1, col2 = st.columns(2)

if col1.button(
    "Add built-in",
    icon=":material/show_chart:",
    type="secondary",
    width="stretch",
):
    st.session_state["_add_indicator_mode"] = "builtin"

if col2.button(
    "Add custom",
    icon=":material/code:",
    type="secondary",
    width="stretch",
):
    st.session_state["_add_indicator_mode"] = "custom"

mode = st.session_state.get("_add_indicator_mode")
if mode == "builtin":
    with st.container(border=True):
        col1, col2 = st.columns([3, 2])

        def _on_builtin_type_change(k):
            """Sync name to the newly selected indicator's name."""
            _persist(k)

            if new_ind := st.session_state.get(k):
                st.session_state["new_builtin_name"] = new_ind.name
                st.session_state["_new_builtin_name"] = new_ind.name

        ind = col1.selectbox(
            label="Indicator",
            key=(key := "new_builtin_type"),
            options=PREDEFINED,
            index=PREDEFINED.index(_default(key, PREDEFINED[0])),
            format_func=lambda s: f"{s.acronym} - {s.name}",
            on_change=lambda k=key: _on_builtin_type_change(k),
        )

        ind_name = col2.text_input(
            label="Name",
            key=(key := "new_builtin_name"),
            value=_default(key, ind.name),
            max_chars=40,
            on_change=lambda k=key: _persist(k),
            help="A unique name for this indicator configuration.",
        )

        params = {}
        if params_schema := INDICATOR_PARAMS.get(ind.acronym):
            cols = st.columns(len(params_schema))
            for col, (param, (label, default, min_v, max_v, step, help_text)) in zip(
                cols, params_schema.items(), strict=True
            ):
                params[param] = col.number_input(
                    label=label,
                    key=f"new_builtin_{param}",
                    value=default,
                    min_value=min_v,
                    max_value=max_v,
                    step=step,
                    format="%.1f" if isinstance(default, float) else None,
                    help=help_text,
                )
        else:
            st.caption(f"{ind.name} has no configurable parameters.")

        st.caption(ind.description())

        name_error = None
        if not ind_name:
            name_error = "Name cannot be empty."
        elif chars := INVALID_FILENAME_CHARS.findall(ind_name):
            name_error = (
                f"The following characters are not allowed in indicator names: "
                f"**{' '.join(sorted(set(chars)))}** "
            )
        elif ind_name in stored_names:
            name_error = f"An indicator with name **{ind_name}** already exists."

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
            for k, v in params.items():
                schema = params_schema.get(k)
                if schema and isinstance(schema[1], int):
                    cast_params[k] = int(v)
                else:
                    cast_params[k] = v

            _save_indicator(
                SavedIndicator(
                    name=ind_name,
                    builtin=ind.acronym,
                    parameters=cast_params,
                )
            )
            st.session_state.pop("_add_indicator_mode", None)
            st.rerun()

        if col2.button("Cancel", icon=":material/close:", width="stretch"):
            st.session_state.pop("_add_indicator_mode", None)
            st.rerun()

elif mode == "custom":
    with st.container(border=True):
        ind_name = st.text_input(
            label="Name",
            key=(key := "new_ind_custom_name"),
            value=_default(key, ""),
            max_chars=40,
            placeholder="My indicator",
            on_change=lambda k=key: _persist(k),
            help="A unique name for this custom indicator.",
        )

        source = st.segmented_control(
            label="Source",
            key=(key := "new_custom_source"),
            required=True,
            options=_CODE_OPTIONS,
            default=_default(key, _CODE_OPTIONS[0]),
            label_visibility="collapsed",
            on_change=lambda k=key: _persist(k),
        )

        code: str | None = None
        if source == _CODE_OPTIONS[0]:
            resp = code_editor(
                key=(key := "new_ind_custom_code_editor"),
                code=_default(key, CODE_PLACEHOLDER),
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
            code = resp["text"]
        else:
            indicator_file = st.file_uploader(
                label="Indicator file",
                key="new_custom_file",
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
            if err := _check_indicator_code(code):
                st.error(err)

        name_error = None
        if not ind_name:
            name_error = "Name cannot be empty."
        elif INVALID_FILENAME_CHARS.findall(ind_name):
            chars = INVALID_FILENAME_CHARS.findall(ind_name)
            name_error = f"Invalid characters: {' '.join(repr(c) for c in sorted(set(chars)))}"
        elif ind_name in stored_names:
            name_error = f"An indicator with name **{ind_name}** already exists."

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
            _save_indicator(
                SavedIndicator(
                    name=ind_name,
                    code=code,
                )
            )
            st.session_state.pop("_add_indicator_mode", None)
            st.rerun()

        if col2.button("Cancel", icon=":material/close:", width="stretch"):
            st.session_state.pop("_add_indicator_mode", None)
            st.rerun()
