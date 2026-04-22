"""Backtide.

Author: Mavs
Description: Indicator management page.

"""

from pathlib import Path

from code_editor import code_editor
import streamlit as st

from backtide.config import get_config
from backtide.indicators import BUILTIN_INDICATORS
from backtide.indicators.utils import (
    _build_custom_indicator,
    _check_indicator_code,
    _get_indicator_label,
    _is_builtin_indicator,
    _load_stored_indicators,
    _save_indicator,
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

# Parameter schemas for built-in indicators.
INDICATOR_PARAMS: dict[str, dict[str, tuple]] = {
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

code_placeholder = lambda t: (
    f'''\
from backtide.indicators import BaseIndicator


class MyIndicator(BaseIndicator):
    def compute(self, data):
        """Compute the indicator values.

        Parameters
        ----------
        data : {t}
            Historical OHLCV data.

        Returns
        -------
        {t}
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
        # ── Write your logic here ────────────────────────



        # ───────────────────────────────────────────────────

        return result


MyIndicator()'''
)


# ─────────────────────────────────────────────────────────────────────────────
# Page interface
# ─────────────────────────────────────────────────────────────────────────────

st.set_page_config(page_title="Backtide - Indicators")

cfg = get_config()

storage_path = Path(cfg.data.storage_path) / "indicators"
storage_path.mkdir(parents=True, exist_ok=True)

st.subheader("Indicators", text_alignment="center")
st.write("")

if stored_ind := _load_stored_indicators(cfg):
    for name, ind in stored_ind.items():
        with st.container(border=True):
            col1, col2 = st.columns([5, 1], vertical_alignment="center")
            col1.markdown(_get_indicator_label(name, ind))

            if col2.button(
                label="Delete",
                key=f"delete_ind_{name}",
                icon=":material/delete:",
                type="tertiary",
            ):
                Path(storage_path, f"{name}.pkl").unlink(missing_ok=True)
                st.rerun()

            # Show source code expander for custom indicators
            if not _is_builtin_indicator(ind):
                with st.expander(
                    label="Source code",
                    key=(exp_key := f"source_code_{name}"),
                    icon=":material/code:",
                    expanded=bool(_default(exp_key)),
                    on_change=lambda k=exp_key: _persist(k),
                ):
                    resp = code_editor(
                        code=(source := getattr(ind, "_source_code", "")),
                        key=f"edit_ind_code_{name}",
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
                        if err := _check_indicator_code(edited_code, cfg):
                            st.error(err)
                        else:
                            if st.button(
                                "Save changes",
                                key=f"save_edit_{name}",
                                icon=":material/save:",
                                type="primary",
                            ):
                                try:
                                    new_instance = _build_custom_indicator(edited_code)
                                    _save_indicator(new_instance, name, cfg)
                                    st.session_state[f"_{exp_key}"] = False
                                    st.rerun()
                                except Exception as ex:  # noqa: BLE001
                                    st.error(f"Failed to rebuild indicator. {ex}")
else:
    st.info("There are no saved indicators.", icon=":material/info:")


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
            options=BUILTIN_INDICATORS,
            index=BUILTIN_INDICATORS.index(_default(key, BUILTIN_INDICATORS[0])),
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
        elif ind_name in stored_ind.keys():
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
            if params_schema:
                for k, v in params.items():
                    schema = params_schema.get(k)
                    if schema and isinstance(schema[1], int):
                        cast_params[k] = int(v)
                    else:
                        cast_params[k] = v

            _save_indicator(ind(**cast_params), ind_name, cfg)
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
            value=_default(key, "MyIndicator"),
            max_chars=40,
            placeholder="Add a name...",
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
                key=(key := "ind_custom_code_editor"),
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
                "The uploaded file must contain a class that inherits from `BaseIndicator` "
                f"with a `compute(self, data: {cfg.data.dataframe_library.class_name})` method."
            )

            indicator_file = st.file_uploader(
                label="Indicator file",
                key="new_custom_file",
                type="py",
                accept_multiple_files=False,
                label_visibility="collapsed",
            )

            if indicator_file is not None:
                code = indicator_file.read().decode("utf-8")
                with st.expander("Preview uploaded file"):
                    st.code(code, language="python", line_numbers=True)
            else:
                st.info("No file uploaded yet.", icon=":material/upload_file:")

        if code:
            if err := _check_indicator_code(code, cfg):
                st.error(err)

        name_error = None
        if not ind_name:
            name_error = "Name cannot be empty."
        elif INVALID_FILENAME_CHARS.findall(ind_name):
            chars = INVALID_FILENAME_CHARS.findall(ind_name)
            name_error = f"Invalid characters: {' '.join(repr(c) for c in sorted(set(chars)))}"
        elif ind_name in stored_ind.keys():
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
            try:
                instance = _build_custom_indicator(code)  # ty: ignore[invalid-argument-type]
                _save_indicator(instance, ind_name, cfg)
                st.session_state.pop("_add_indicator_mode", None)
                st.rerun()
            except Exception as ex:  # noqa: BLE001
                st.error(f"Failed to build indicator. {ex}")

        if col2.button("Cancel", icon=":material/close:", width="stretch"):
            st.session_state.pop("_add_indicator_mode", None)
            st.rerun()
