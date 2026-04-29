"""Backtide.

Author: Mavs
Description: Utility functions to work with strategies.

"""

import ast
import inspect
import logging
from pathlib import Path
from typing import Any

import cloudpickle
import streamlit as st

from backtide.config import Config
from backtide.indicators.utils import _is_builtin_indicator, _save_indicator
from backtide.strategies.base import BaseStrategy

_log = logging.getLogger(__name__)


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


def _load_strategies_by_name(cfg: Config, names: list[str]) -> list[Any]:
    """Load specific strategies by name from storage. Missing or unreadable
    entries are skipped (with a warning) so a stray entry does not abort the
    whole experiment."""
    path = Path(cfg.data.storage_path) / "strategies"
    out: list[Any] = []
    for name in names:
        target = path / f"{name}.pkl"
        if not target.exists():
            _log.warning("Strategy %s not found in storage; skipping.", name)
            continue
        try:
            with target.open("rb") as fh:
                out.append(cloudpickle.load(fh))
        except Exception as ex:  # noqa: BLE001
            _log.warning("Failed to load strategy %s: %s", name, ex)
    return out


def _save_strategy(strat: Any, name: str, cfg: Config):
    """Pickle a strategy instance to disk."""
    path = Path(cfg.data.storage_path) / "strategies"
    path.mkdir(parents=True, exist_ok=True)

    with (path / f"{name}.pkl").open("wb") as f:
        cloudpickle.dump(strat, f)


# ─────────────────────────────────────────────────────────────────────────────
# Strategy → indicator auto-inclusion
# ─────────────────────────────────────────────────────────────────────────────


def _auto_indicator_name(ind: Any) -> str:
    """Return a deterministic on-disk name for an auto-included indicator.

    The name is built from the indicator's class acronym (or class name) and
    its constructor arguments, prefixed with ``__auto_`` so it doesn't clash
    with user-saved indicators. Two indicators with the same class + params
    yield the same name and therefore the same pickle file.
    """
    cls = type(ind)
    acronym = getattr(cls, "acronym", cls.__name__)
    try:
        _, args = ind.__reduce__()
    except Exception:  # noqa: BLE001
        args = ()
    arg_str = "_".join(str(a) for a in args) if args else "default"
    # Sanitize for filesystems
    arg_str = arg_str.replace(".", "p").replace("-", "n").replace(" ", "")
    return f"__auto_{acronym}_{arg_str}"


def _resolve_strategy_indicators(strategies: list[Any]) -> list[tuple[str, BaseStrategy, str]]:
    """Return indicators required by the given strategies.

    Looks each strategy up in the ``STRATEGY_INDICATORS`` registry and builds
    the corresponding indicator instances using the strategy's current
    parameter values. Duplicate (class, params) pairs are de-duplicated; the
    first strategy that requested each indicator is recorded as its source.

    """
    # Local import to avoid a circular import at module load time.
    from backtide.strategies import STRATEGY_INDICATORS

    seen: set[str] = set()
    out: list[tuple[str, Any, str]] = []
    for strat in strategies:
        builder = STRATEGY_INDICATORS.get(type(strat))
        if builder is None:
            continue
        try:
            indicators = builder(strat)
        except Exception:  # noqa: BLE001
            continue
        source = getattr(type(strat), "name", type(strat).__name__)
        for ind in indicators:
            name = _auto_indicator_name(ind)
            if name in seen:
                continue
            seen.add(name)
            out.append((name, ind, source))
    return out


def _ensure_auto_indicators_saved(
    cfg: Config, required: list[tuple[str, Any, str]]
) -> list[str]:
    """Persist any missing auto-indicator pickles and return their names.

    Indicators that are already on disk under the given ``__auto_*`` name
    are not re-pickled. Returns the list of names in input order.
    """
    path = Path(cfg.data.storage_path) / "indicators"
    path.mkdir(parents=True, exist_ok=True)
    names: list[str] = []
    for name, ind, _source in required:
        if not _is_builtin_indicator(ind):
            # Only built-in indicators can be auto-resolved deterministically.
            continue
        target = path / f"{name}.pkl"
        if not target.exists():
            try:
                _save_indicator(ind, name, cfg)
            except Exception as ex:  # noqa: BLE001
                _log.warning("Could not auto-save indicator %s: %s", name, ex)
                continue
        names.append(name)
    return names

