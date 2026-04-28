"""Backtide.

Author: Mavs
Description: Utility functions to work with indicators.

"""

import ast
import inspect
from pathlib import Path
from typing import Any

import cloudpickle
import streamlit as st

from backtide.config import Config
from backtide.indicators import BaseIndicator
from backtide.utils.utils import _make_dummy_bars


def _build_custom_indicator(code: str) -> BaseIndicator:
    """Execute code and return the last expression."""
    tree = ast.parse(code)

    if not tree.body or not isinstance(tree.body[-1], ast.Expr):
        raise ValueError("The last statement must be an instantiation of the indicator.")

    # Exec everything except the last statement, eval the last
    ns = {}
    exec(compile(tree, "<indicator>", "exec"), ns)
    instance = eval(compile(ast.Expression(body=tree.body[-1].value), "<indicator>", "eval"), ns)

    if not isinstance(instance, BaseIndicator):
        raise TypeError(f"Expected a subclass of BaseIndicator, got {type(instance).__name__}.")

    # Can't reliably recover source code from an unpickled object
    # so we add the source code to the instance
    instance._source_code = code

    return instance


def _check_indicator_code(code: str, cfg: Config) -> str | None:
    """Validate that `code` defines a class with `compute(self, data)` and test it."""
    try:
        ast.parse(code)
    except SyntaxError as ex:
        return f"Syntax error:\n\n{ex}"

    try:
        instance = _build_custom_indicator(code)
    except Exception as ex:  # noqa: BLE001
        return f"Failed to instantiate indicator: {ex}"

    # Verify the compute method exists with the correct signature
    sig = inspect.signature(instance.compute)
    if list(sig.parameters.keys()) != ["data"]:
        return "Method `compute` doesn't have signature: `compute(self, data)`."

    dummy = _make_dummy_bars(cfg.data.dataframe_library)
    try:
        result = instance.compute(dummy)
    except Exception as ex:  # noqa: BLE001
        return f"{ex.__class__.__name__}: {ex}"

    if result is None:
        return "Indicator `compute` returned `None`. It must return a result."

    return None


def _get_indicator_label(name: str, ind: BaseIndicator) -> str:
    """Build a UI label for an indicator."""
    cls = type(ind)
    if _is_builtin_indicator(ind):
        label = f":material/show_chart: **{name}** · _{cls.acronym}_"

        # Show parameters for builtin indicators
        _, args = ind.__reduce__()
        sig = inspect.signature(cls)
        if params := dict(zip(sig.parameters, args, strict=True)):
            label += " · " + ", ".join(f"{k}={v}" for k, v in params.items())

        return label
    else:
        return f":material/show_chart: **{name}** · _Custom_"


def _is_builtin_indicator(ind: Any) -> bool:
    """Return True if the indicator is a built-in (Rust-defined) indicator."""
    return getattr(type(ind), "__module__", "").startswith("backtide.")


def _load_stored_indicators(cfg: Config) -> dict[str, BaseIndicator]:
    """Load and return the indicator objects from storage."""
    path = Path(cfg.data.storage_path) / "indicators"

    indicators = {}
    for f in sorted(path.glob("*.pkl")):
        try:
            with f.open("rb") as fh:
                indicators[f.stem] = cloudpickle.load(fh)
        except Exception as ex:  # noqa: BLE001
            st.error(f"Failed to load indicator **{f.stem}**. Exception: {ex}")

    return indicators


def _save_indicator(ind: BaseIndicator, name: str, cfg: Config):
    """Pickle an indicator instance to disk."""
    path = Path(cfg.data.storage_path) / "indicators"

    with (path / f"{name}.pkl").open("wb") as f:
        cloudpickle.dump(ind, f)
