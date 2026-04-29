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
from backtide.indicators import BaseIndicator
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


def _load_stored_strategies(cfg: Config) -> dict[str, BaseStrategy]:
    """Load and return the strategy objects from storage."""
    path = Path(cfg.data.storage_path) / "strategies"

    strategies = {}
    for f in sorted(path.glob("*.pkl")):
        try:
            with f.open("rb") as fh:
                strategies[f.stem] = cloudpickle.load(fh)
        except Exception as ex:  # noqa: BLE001
            st.error(f"Failed to load strategy **{f.stem}**. Exception: {ex}")

    return strategies


def _resolve_auto_indicators(strats: list[BaseStrategy]) -> list[tuple[str, BaseIndicator, str]]:
    """Return indicators required by the given strategies."""
    out = []
    seen = set()
    for strat in strats:
        if get := getattr(strat, "required_indicators", None):
            if callable(get):
                cls = type(strat)
                source = getattr(cls, "name", cls.__name__)

                for ind in get():
                    # Create a deterministic name for the indicator
                    cls = type(ind)
                    acronym = getattr(cls, "acronym", cls.__name__)

                    try:
                        _, args = ind.__reduce__()
                    except Exception:  # noqa: BLE001
                        args = ()

                    arg_str = "_".join(str(a) for a in args) if args else "default"

                    # Sanitize for filesystems
                    arg_str = arg_str.replace(".", "p").replace("-", "n").replace(" ", "")
                    name = f"__auto_{acronym}_{arg_str}"

                    if name not in seen:
                        seen.add(name)
                        out.append((name, ind, source))

    return out


def _save_strategy(strat: BaseStrategy, name: str, cfg: Config):
    """Pickle a strategy instance to disk."""
    path = Path(cfg.data.storage_path) / "strategies"
    path.mkdir(parents=True, exist_ok=True)

    with (path / f"{name}.pkl").open("wb") as f:
        cloudpickle.dump(strat, f)
