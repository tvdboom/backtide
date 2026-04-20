"""Backtide.

Author: Mavs
Description: Indicator management page.

"""

import ast

from code_editor import code_editor
import streamlit as st
import cloudpickle as pickle

from backtide.backtest import list_indicators
from backtide.ui.utils import (
    SavedIndicator,
    _INDICATORS_DIR,
    _load_saved_indicators,
)
from backtide.utils.constants import INVALID_FILENAME_CHARS


# ─────────────────────────────────────────────────────────────────────────────
# Helper functionalities
# ─────────────────────────────────────────────────────────────────────────────

# Load all predefined indicator classes from Rust (with default params).
# Used to populate the selectbox and get param schemas.
_PREDEFINED = list_indicators()
_PREDEFINED_MAP = {ind.acronym: ind for ind in _PREDEFINED}

# Parameter schemas for built-in indicators.
# {param_name: (label, default, min, max, step, help_text)}
INDICATOR_PARAMS: dict[str, dict] = {
    "SMA": {"period": ("Period", 14, 2, 500, 1, "Number of bars for the moving average window.")},
    "EMA": {"period": ("Period", 14, 2, 500, 1, "Number of bars for the exponential moving average window.")},
    "WMA": {"period": ("Period", 14, 2, 500, 1, "Number of bars for the weighted moving average window.")},
    "RSI": {"period": ("Period", 14, 2, 500, 1, "Lookback period for the RSI calculation.")},
    "MACD": {
        "fast_period": ("Fast period", 12, 2, 500, 1, "Number of bars for the fast EMA."),
        "slow_period": ("Slow period", 26, 2, 500, 1, "Number of bars for the slow EMA."),
        "signal_period": ("Signal period", 9, 2, 500, 1, "Number of bars for the signal line EMA."),
    },
    "BB": {
        "period": ("Period", 20, 2, 500, 1, "Number of bars for the moving average."),
        "std_dev": ("Std. deviations", 2.0, 0.1, 10.0, 0.1, "Number of standard deviations for the bands."),
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


def _save_indicator(ind: SavedIndicator) -> None:
    """Persist an indicator definition to disk as pickle."""
    _INDICATORS_DIR.mkdir(parents=True, exist_ok=True)
    path = _INDICATORS_DIR / f"{ind.name}.pkl"
    path.write_bytes(pickle.dumps(ind))


def _delete_indicator(name: str) -> None:
    """Remove a saved indicator definition from disk."""
    path = _INDICATORS_DIR / f"{name}.pkl"
    path.unlink(missing_ok=True)


def _check_indicator_code(code: str) -> str | None:
    """Validate that *code* defines `indicator(data)`."""
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
    exec(code, ns)  # noqa: S102
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

USER_CODE_OPTIONS = [":material/code: Code editor", ":material/upload_file: Upload file"]

saved = _load_saved_indicators()

if saved:
    for ind in saved:
        with st.container(border=True):
            col1, col2 = st.columns([5, 1], vertical_alignment="center")

            if ind.kind == "builtin":
                label = f":material/show_chart: **{ind.name}** · {ind.builtin_type}"
                if ind.parameters:
                    params_str = ", ".join(f"{k}={v}" for k, v in ind.parameters.items())
                    label += f" ({params_str})"
            else:
                label = f":material/code: **{ind.name}** · Custom"

            col1.markdown(label)

            if col2.button(
                label="Delete",
                key=f"delete_ind_{ind.name}",
                icon=":material/delete:",
                type="tertiary",
            ):
                _delete_indicator(ind.name)
                st.rerun()
else:
    st.info("No saved indicators yet.", icon=":material/info:")

# ─────────────────────────────────────────────────────────────────────────────
# Add new indicator
# ─────────────────────────────────────────────────────────────────────────────

st.divider()

col1, col2 = st.columns(2)

if col1.button(
    "Add built-in",
    icon=":material/show_chart:",
    type="secondary",
    use_container_width=True,
):
    st.session_state["_add_indicator_mode"] = "builtin"

if col2.button(
    "Add custom",
    icon=":material/code:",
    type="secondary",
    use_container_width=True,
):
    st.session_state["_add_indicator_mode"] = "custom"

_mode = st.session_state.get("_add_indicator_mode")

if _mode == "builtin":
    with st.container(border=True):
        col1, col2 = st.columns([3, 2])

        ind_type_str = col1.selectbox(
            label="Indicator",
            key="new_builtin_type",
            options=list(_PREDEFINED_MAP),
            format_func=lambda s: f"{s} - {_PREDEFINED_MAP[s].name}",
        )

        default_name = str(ind_type_str)
        ind_name = col2.text_input(
            label="Name",
            key="new_builtin_name",
            value=default_name,
            max_chars=40,
            help="A unique name for this indicator configuration.",
        )

        # Parameter inputs
        params_schema = INDICATOR_PARAMS.get(ind_type_str, {})
        params = {}
        if params_schema:
            cols = st.columns(len(params_schema))
            for col, (param_key, (label, default, min_v, max_v, step, help_text)) in zip(
                cols, params_schema.items()
            ):
                params[param_key] = col.number_input(
                    label=label,
                    key=f"new_builtin_{param_key}",
                    value=default,
                    min_value=min_v,
                    max_value=max_v,
                    step=step,
                    format="%.1f" if isinstance(default, float) else None,
                    help=help_text,
                )
        else:
            st.caption(f"{_PREDEFINED_MAP[ind_type_str].name} has no configurable parameters.")

        st.caption(_PREDEFINED_MAP[ind_type_str].description)

        # Validate name
        name_error = None
        if not ind_name:
            name_error = "Name is required."
        elif INVALID_FILENAME_CHARS.findall(ind_name):
            chars = INVALID_FILENAME_CHARS.findall(ind_name)
            name_error = f"Invalid characters: {' '.join(repr(c) for c in sorted(set(chars)))}"

        if name_error:
            st.error(name_error)

        col1, col2 = st.columns(2)

        if col1.button(
            "Save",
            icon=":material/save:",
            type="primary",
            disabled=bool(name_error),
            use_container_width=True,
        ):
            cast_params = {}
            for k, v in params.items():
                schema = params_schema.get(k)
                if schema and isinstance(schema[1], int):
                    cast_params[k] = int(v)
                else:
                    cast_params[k] = v

            # Construct the indicator with user-chosen params
            ind_cls = type(_PREDEFINED_MAP[ind_type_str])
            compute_fn = ind_cls(**cast_params)
            _save_indicator(SavedIndicator(
                name=ind_name,
                kind="builtin",
                compute_fn=compute_fn,
                builtin_type=ind_type_str,
                parameters=cast_params,
            ))
            st.session_state.pop("_add_indicator_mode", None)
            st.rerun()

        if col2.button("Cancel", icon=":material/close:", use_container_width=True):
            st.session_state.pop("_add_indicator_mode", None)
            st.rerun()

elif _mode == "custom":
    with st.container(border=True):
        ind_name = st.text_input(
            label="Name",
            key="new_custom_name",
            max_chars=40,
            placeholder="My indicator",
            help="A unique name for this custom indicator.",
        )

        source = st.segmented_control(
            label="Source",
            key="new_custom_source",
            required=True,
            options=USER_CODE_OPTIONS,
            default=USER_CODE_OPTIONS[0],
            label_visibility="collapsed",
        )

        code: str | None = None
        if source == USER_CODE_OPTIONS[0]:
            resp = code_editor(
                code=CODE_PLACEHOLDER,
                key="new_custom_code_editor",
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
            name_error = "Name is required."
        elif INVALID_FILENAME_CHARS.findall(ind_name):
            chars = INVALID_FILENAME_CHARS.findall(ind_name)
            name_error = f"Invalid characters: {' '.join(repr(c) for c in sorted(set(chars)))}"

        if name_error:
            st.error(name_error)

        col1, col2 = st.columns(2)

        if col1.button(
            "Save",
            icon=":material/save:",
            type="primary",
            disabled=bool(name_error) or not code,
            use_container_width=True,
        ):
            compute_fn = _make_custom_compute_fn(code)
            _save_indicator(SavedIndicator(
                name=ind_name,
                kind="custom",
                compute_fn=compute_fn,
                code=code,
            ))
            st.session_state.pop("_add_indicator_mode", None)
            st.rerun()

        if col2.button("Cancel", icon=":material/close:", use_container_width=True):
            st.session_state.pop("_add_indicator_mode", None)
            st.rerun()
