"""Backtide.

Author: Mavs
Description: Utility functions to work with strategies.

"""

import ast
from collections.abc import Sequence
import inspect
import logging
from pathlib import Path
from typing import Any

from backtide.config import Config
from backtide.indicators import BaseIndicator, _indicator_deterministic_name
from backtide.strategies.base import BaseStrategy
from backtide.utils.library import _build_custom_instance, _load_pickles, _save_pickle

logger = logging.getLogger(__name__)


def _build_custom_strategy(code: str) -> BaseStrategy:
    """Execute code and return the last expression."""
    return _build_custom_instance(
        code,
        filename="<strategy>",
        expected_type=BaseStrategy,
        missing_expression="The last statement must be an instantiation of the strategy.",
        type_error=lambda value: (
            f"Expected a subclass of BaseStrategy, got {type(value).__name__}."
        ),
    )


def _check_strategy_code(code: str) -> str | None:
    """Validate that `code` defines a method with the expected signature."""
    try:
        ast.parse(code)
    except SyntaxError as ex:
        return f"Syntax error:\n\n{ex}"

    try:
        instance = _build_custom_strategy(code)
    except Exception as ex:  # noqa: BLE001
        return f"Failed to instantiate strategy: {ex}"

    # Verify the evaluate method exists with the correct signature
    sig = inspect.signature(instance.evaluate)
    if list(sig.parameters.keys()) != ["data", "portfolio", "state", "indicators"]:
        return (
            "Method `evaluate` doesn't have signature: "
            "`evaluate(self, data, portfolio, state, indicators)`."
        )

    # Check that every return statement in `evaluate` yields a list expression.
    tree = ast.parse(code)
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        if node.name != "evaluate":
            continue

        returns = [n for n in ast.walk(node) if isinstance(n, ast.Return)]
        if not returns:
            return "Method `evaluate` must return a list of Orders."

        for ret in returns:
            if ret.value is None:
                return "Method `evaluate` must return a list of Orders, not None."
            if isinstance(ret.value, ast.Constant):
                return (
                    "Method `evaluate` must return a list of Orders, "
                    f"not a constant ({ret.value.value!r})."
                )

    return None


def _get_strategy_label(name: str, strat: Any) -> str:
    """Build a UI label for a strategy."""
    cls = type(strat)
    if _is_builtin_strategy(strat):
        category = "Multi-Asset" if cls.is_multi_asset else "Single Asset"
        label = f":material/psychology: **{name}** · _{cls.name}_ · {category}"

        # Show parameters for builtin strategies
        _, args = strat.__reduce__()
        sig = inspect.signature(cls)
        if params := dict(zip(sig.parameters, args, strict=True)):
            label += " · " + ", ".join(f"{k}={v}" for k, v in params.items())

        return label
    else:
        return f":material/psychology: **{name}** · _Custom_"


def _is_builtin_strategy(strat: Any) -> bool:
    """Return True if the strategy is a built-in (Rust-defined) strategy."""
    return getattr(type(strat), "__module__", "").startswith("backtide.")


def _load_stored_strategies(cfg: Config) -> dict[str, Any]:
    """Load and return the strategy objects from storage."""
    return _load_pickles(
        Path(cfg.data.storage_path) / "strategies",
        logger=logger,
        item_name="strategy",
    )


def _resolve_auto_indicators(strats: Sequence[Any]) -> list[tuple[str, BaseIndicator, str]]:
    """Return indicators required by the given strategies.

    Accepts any iterable of objects (built-in Rust strategies don't inherit
    from `BaseStrategy` in the Python class hierarchy, so a strict `BaseStrategy`
    bound would needlessly exclude them at type-check time). The function itself
    duck-types on `required_indicators` and silently skips objects that lack it.

    """
    out = []
    seen = set()
    for strat in strats:
        if get := getattr(strat, "required_indicators", None):
            if callable(get):
                cls = type(strat)
                source = getattr(cls, "name", cls.__name__)

                for ind in get():
                    name = _indicator_deterministic_name(ind)
                    if name not in seen:
                        seen.add(name)
                        out.append((name, ind, source))

    return out


def _save_strategy(strat: Any, name: str, cfg: Config) -> None:
    """Pickle a strategy instance to disk."""
    _save_pickle(
        strat,
        Path(cfg.data.storage_path) / "strategies",
        name,
        temporary_prefix=".strategy-",
    )
