"""Backtide.

Author: Mavs
Description: Utility functions to work with indicators.

"""

import inspect
import pickle
from pathlib import Path

import cloudpickle
import streamlit as st
from backtide.indicators import BaseIndicator


def _check_indicator_code(code: str) -> str | None:
    """Validate that `code` defines a class with `compute(self, data)`."""
    import ast

    try:
        tree = ast.parse(code)
        for node in ast.walk(tree):
            if isinstance(node, ast.ClassDef):
                for item in node.body:
                    if isinstance(item, ast.FunctionDef) and item.name == "compute":
                        args = [a.arg for a in item.args.args]
                        if args == ["self", "data"]:
                            return None
                        return (
                            f"Method `compute` in class `{node.name}` must have "
                            f"signature `compute(self, data)`."
                        )
        return "No class with a `compute(self, data)` method found in the code."
    except SyntaxError as ex:
        return f"Syntax error:\n\n{ex}"


def _save_indicator(
    indicator: BaseIndicator,
    name: str,
    storage_path: Path,
    *,
    code: str | None = None,
):
    """Pickle an indicator instance to disk under `name.pkl`."""
    storage_path.mkdir(parents=True, exist_ok=True)
    serializer = pickle if _is_builtin_indicator(indicator) else cloudpickle
    with (storage_path / f"{name}.pkl").open("wb") as f:
        serializer.dump(indicator, f)

    py_path = storage_path / f"{name}.py"
    if code is not None:
        py_path.write_text(code, encoding="utf-8")
    else:
        # Remove stale source file if saving a builtin
        py_path.unlink(missing_ok=True)


def _load_stored_indicators(cfg: Config) -> dict[str, Any]:
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


def _get_indicator_label(ind: BaseIndicator) -> str:
    """Build a UI label for an indicator."""
    cls = type(ind)
    if _is_builtin_indicator(ind):
        label = f":material/show_chart: **{cls.name}** · _{cls.acronym}_"

        # Show parameters for builtin indicators
        _, args = ind.__reduce__()
        sig = inspect.signature(cls)
        if params := {n: v for n, v in zip(sig.parameters, args, strict=True)}:
            label += f" · {', '.join(f'{k}={v}' for k, v in params.items())}"

        return label
    else:
        return f":material/code: **{cls.__name__}** · Custom"


def _is_builtin_indicator(ind: Any) -> bool:
    """Return True if the indicator is a built-in (Rust-defined) indicator."""
    return getattr(type(ind), "__module__", "").startswith("backtide.")


def _build_custom_indicator(code: str) -> BaseIndicator:
    """Execute code and instantiate the first BaseIndicator subclass found."""
    ns: dict = {}
    exec(code, ns)

    for obj in ns.values():
        if (
            isinstance(obj, type)
            and issubclass(obj, BaseIndicator)
            and obj is not BaseIndicator
        ):
            return obj()

    raise ValueError("The code must define a BaseIndicator subclass.")
