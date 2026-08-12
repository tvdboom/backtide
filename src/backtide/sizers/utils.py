"""Backtide.

Author: Mavs
Description: Utilities for persisted built-in and custom position sizers.

"""

import ast
import inspect
import logging
import math
import os
from pathlib import Path
import tempfile
from typing import Any

import cloudpickle

from backtide.config import Config
from backtide.sizers import BaseSizer

logger = logging.getLogger(__name__)

_BUILTIN_RECORD_FORMAT = "backtide.builtin-sizer.v1"

BUILTIN_SIZER_DEFAULTS: dict[str, dict[str, int | float]] = {
    "EqualWeight": {"n_positions": 10},
    "FixedFractional": {"fraction": 0.1},
    "FixedNotional": {"amount": 1_000.0},
    "FixedQuantity": {"quantity": 1.0},
    "KellyCriterion": {
        "win_rate": 0.55,
        "avg_win": 1.0,
        "avg_loss": 1.0,
        "fraction": 0.25,
    },
    "RiskBased": {"risk_pct": 0.01},
    "VolatilityScaled": {"risk_pct": 0.01},
}


def _build_custom_sizer(code: str) -> BaseSizer:
    """Execute source and return the final sizer instance."""
    tree = ast.parse(code)
    if not tree.body or not isinstance(tree.body[-1], ast.Expr):
        raise ValueError("The last statement must instantiate the sizer.")
    namespace: dict[str, Any] = {}
    exec(compile(tree, "<sizer>", "exec"), namespace)
    instance = eval(compile(ast.Expression(tree.body[-1].value), "<sizer>", "eval"), namespace)
    if not isinstance(instance, BaseSizer):
        raise TypeError(f"Expected a BaseSizer subclass, got {type(instance).__name__}.")
    instance._source_code = code
    return instance


def _check_sizer_code(code: str) -> str | None:
    """Validate the custom sizer contract with deterministic account inputs."""
    try:
        instance = _build_custom_sizer(code)
    except Exception as exc:  # noqa: BLE001
        return f"Failed to instantiate sizer: {exc}"
    signature = inspect.signature(instance.calculate)
    expected = ["equity", "price", "stop_distance", "atr"]
    if list(signature.parameters) != expected:
        return f"Method `calculate` must have parameters: {', '.join(expected)}."
    try:
        result = float(instance.calculate(10_000.0, 100.0, 5.0, 0.2))
    except Exception as exc:  # noqa: BLE001
        return f"{exc.__class__.__name__}: {exc}"
    if not math.isfinite(result):
        return "Sizer `calculate` must return a finite number."
    return None


def _is_builtin_sizer(value: Any) -> bool:
    """Return whether `value` is implemented by the Rust extension."""
    return getattr(type(value), "__module__", "").startswith("backtide.")


def _load_stored_sizers(cfg: Config) -> dict[str, Any]:
    """Load valid saved sizers from the configured storage directory."""
    values = {}
    for file in sorted((Path(cfg.data.storage_path) / "sizers").glob("*.pkl")):
        try:
            if file.stat().st_size == 0:
                file.unlink()
                continue
            with file.open("rb") as stream:
                value = cloudpickle.load(stream)
            values[file.stem] = _restore_sizer(value)
        except Exception as exc:  # noqa: BLE001
            logger.warning("Failed to load sizer %s: %s", file.stem, exc)
    return values


def _save_sizer(value: Any, name: str, cfg: Config) -> None:
    """Persist one validated sizer with an atomic file replacement."""
    folder = Path(cfg.data.storage_path) / "sizers"
    folder.mkdir(parents=True, exist_ok=True)
    target = folder / f"{name}.pkl"
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb",
            dir=folder,
            prefix=".sizer-",
            suffix=".tmp",
            delete=False,
        ) as stream:
            temporary = Path(stream.name)
            cloudpickle.dump(_stored_sizer(value), stream)
            stream.flush()
            os.fsync(stream.fileno())
        temporary.replace(target)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def _stored_sizer(value: Any) -> Any:
    """Return a stable record for built-ins and the original custom object otherwise."""
    if not _is_builtin_sizer(value):
        return value
    parameters = {name: getattr(value, name) for name in inspect.signature(type(value)).parameters}
    return {
        "format": _BUILTIN_RECORD_FORMAT,
        "type": type(value).__name__,
        "parameters": parameters,
    }


def _restore_sizer(value: Any) -> Any:
    """Reconstruct a built-in sizer record while accepting legacy pickle files."""
    if not isinstance(value, dict) or value.get("format") != _BUILTIN_RECORD_FORMAT:
        return value
    from backtide.sizers import BUILTIN_SIZERS

    sizer_type = str(value.get("type") or "")
    cls = next((item for item in BUILTIN_SIZERS if item.__name__ == sizer_type), None)
    if cls is None:
        raise ValueError(f"Unknown built-in sizer type {sizer_type!r}.")
    parameters = value.get("parameters")
    if not isinstance(parameters, dict):
        raise ValueError("Built-in sizer parameters must be a mapping.")
    return cls(**parameters)
