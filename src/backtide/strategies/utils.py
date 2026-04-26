"""Backtide.

Author: Mavs
Description: Utility functions to work with strategies.

"""

import ast
import inspect
from pathlib import Path
from typing import Any

import cloudpickle
import streamlit as st

from backtide.config import Config
from backtide.strategies.base import BaseStrategy


def _build_custom_strategy(code: str) -> BaseStrategy:
    """Execute code and return the last expression."""
    tree = ast.parse(code)

    if not tree.body or not isinstance(tree.body[-1], ast.Expr):
        raise ValueError("The last statement must be an instantiation of the strategy.")

    # Exec everything except the last statement, eval the last
    ns = {}
    exec(compile(tree, "<strategy>", "exec"), ns)
    instance = eval(compile(ast.Expression(body=tree.body[-1].value), "<strategy>", "eval"), ns)

    if not isinstance(instance, BaseStrategy):
        raise TypeError(f"Expected a subclass of BaseStrategy, got {type(instance).__name__}.")

    # Can't reliably recover source code from an unpickled object
    # so we add the source code to the instance
    instance._source_code = code

    return instance


def _check_strategy_code(code: str) -> str | None:
    """Validate that `code` defines a class with `evaluate(self, data, state, indicators)`."""
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
    params = list(sig.parameters.keys())
    if params != ["data", "state", "indicators"]:
        return (
            "Method `evaluate` doesn't have "
            "signature: `evaluate(self, data, state, indicators)`."
        )

    return None


def _get_strategy_label(name: str, strat: Any) -> str:
    """Build a UI label for a strategy."""
    cls = type(strat)
    if _is_builtin_strategy(strat):
        category = "Multi-Asset" if cls.is_multi_asset else "Single Asset"
        label = f":material/psychology: **{name}** · _{cls.name}_ · {category}\n\n"

        # Show parameters for builtin strategies
        _, args = strat.__reduce__()
        sig = inspect.signature(cls)
        if params := dict(zip(sig.parameters, args, strict=True)):
            label += " · ".join(f"{k}={v}" for k, v in params.items())

        return label
    else:
        return f":material/psychology: **{name}** · _Custom_"


def _is_builtin_strategy(strat: Any) -> bool:
    """Return True if the strategy is a built-in (Rust-defined) strategy."""
    return getattr(type(strat), "__module__", "").startswith("backtide.")


def _load_stored_strategies(cfg: Config) -> dict[str, Any]:
    """Load and return the strategy objects from storage."""
    path = Path(cfg.data.storage_path) / "strategies"

    strategies: dict[str, Any] = {}
    for f in sorted(path.glob("*.pkl")):
        try:
            with f.open("rb") as fh:
                strategies[f.stem] = cloudpickle.load(fh)
        except Exception as ex:  # noqa: BLE001
            st.error(f"Failed to load strategy **{f.stem}**. Exception: {ex}")

    return strategies


def _save_strategy(strat: Any, name: str, cfg: Config):
    """Pickle a strategy instance to disk."""
    path = Path(cfg.data.storage_path) / "strategies"
    path.mkdir(parents=True, exist_ok=True)

    with (path / f"{name}.pkl").open("wb") as f:
        cloudpickle.dump(strat, f)
