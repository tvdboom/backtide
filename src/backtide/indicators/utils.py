"""Backtide.

Author: Mavs
Description: Utility functions to work with indicators.

"""

import ast
import inspect
import logging
from pathlib import Path
from typing import Any

from backtide.config import Config
from backtide.indicators import BaseIndicator
from backtide.utils.library import _build_custom_instance, _load_pickles, _save_pickle
from backtide.utils.utils import _make_dummy_bars

logger = logging.getLogger(__name__)


def _build_custom_indicator(code: str) -> BaseIndicator:
    """Execute code and return the last expression."""
    return _build_custom_instance(
        code,
        filename="<indicator>",
        expected_type=BaseIndicator,
        missing_expression="The last statement must be an instantiation of the indicator.",
        type_error=lambda value: (
            f"Expected a subclass of BaseIndicator, got {type(value).__name__}."
        ),
    )


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


def _load_stored_indicators(cfg: Config) -> dict[str, Any]:
    """Load and return the indicator objects from storage."""
    return _load_pickles(
        Path(cfg.data.storage_path) / "indicators",
        logger=logger,
        item_name="indicator",
    )


def _save_indicator(ind: Any, name: str, cfg: Config) -> None:
    """Pickle an indicator instance to disk."""
    _save_pickle(
        ind,
        Path(cfg.data.storage_path) / "indicators",
        name,
        temporary_prefix=".indicator-",
    )
