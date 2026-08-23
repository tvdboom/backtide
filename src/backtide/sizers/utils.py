"""Backtide.

Author: Mavs
Description: Utilities for persisted built-in and custom position sizers.

"""

import inspect
import logging
import math
from pathlib import Path
from typing import Any

from backtide.config import Config
from backtide.sizers import BaseSizer
from backtide.utils.library import _build_custom_instance, _load_pickles, _save_pickle

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
    return _build_custom_instance(
        code,
        filename="<sizer>",
        expected_type=BaseSizer,
        missing_expression="The last statement must instantiate the sizer.",
        type_error=lambda value: f"Expected a BaseSizer subclass, got {type(value).__name__}.",
    )


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
    return _load_pickles(
        Path(cfg.data.storage_path) / "sizers",
        logger=logger,
        item_name="sizer",
        restore=_restore_sizer,
        remove_empty=True,
    )


def _save_sizer(value: Any, name: str, cfg: Config) -> None:
    """Persist one validated sizer with an atomic file replacement."""
    _save_pickle(
        _stored_sizer(value),
        Path(cfg.data.storage_path) / "sizers",
        name,
        temporary_prefix=".sizer-",
    )


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
