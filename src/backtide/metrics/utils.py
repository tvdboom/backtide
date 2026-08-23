"""Backtide.

Author: Mavs
Description: Utilities for stored custom metrics.

"""

import inspect
import logging
import math
from pathlib import Path
from typing import Any

import pandas as pd

from backtide.config import Config
from backtide.metrics.base import BaseMetric
from backtide.utils.library import _build_custom_instance, _load_pickles, _save_pickle

logger = logging.getLogger(__name__)


def _metric_greater_is_better(metric: BaseMetric) -> bool:
    """Return the metric ranking direction, including the legacy attribute."""
    custom_classes: list[type[Any]] = []
    for cls in type(metric).__mro__:
        if cls is BaseMetric:
            break
        custom_classes.append(cls)
    for attribute in ("greater_is_better", "higher_is_better"):
        for cls in custom_classes:
            if attribute in cls.__dict__:
                value = cls.__dict__[attribute]
                if not isinstance(value, bool):
                    raise TypeError("Metric `greater_is_better` must be a bool.")
                return value
    return True


def _build_custom_metric(code: str) -> BaseMetric:
    """Execute code and return the final metric instance."""
    return _build_custom_instance(
        code,
        filename="<metric>",
        expected_type=BaseMetric,
        missing_expression="The last statement must be an instantiation of the metric.",
        type_error=lambda value: f"Expected a subclass of BaseMetric, got {type(value).__name__}.",
    )


def _check_metric_code(code: str) -> str | None:
    """Validate a custom metric signature and execute it on deterministic data."""
    try:
        instance = _build_custom_metric(code)
    except Exception as ex:  # noqa: BLE001
        return f"Failed to instantiate metric: {ex}"
    signature = inspect.signature(instance.compute)
    if list(signature.parameters) != ["equity_curve", "trades"]:
        return "Method `compute` must have signature: `compute(self, equity_curve, trades)`."
    description = type(instance).__doc__
    if not description or not inspect.cleandoc(description):
        return "Metric class must define a docstring used as its description."
    if not isinstance(instance.percentage, bool):
        return "Metric `percentage` must be a bool."
    try:
        _metric_greater_is_better(instance)
    except TypeError as ex:
        return str(ex)
    equity = pd.DataFrame(
        {"timestamp": [0, 86_400], "equity": [100.0, 101.0], "drawdown": [0.0, 0.0]}
    )
    trades = pd.DataFrame(
        {
            "symbol": ["TEST"],
            "quantity": [1.0],
            "entry_ts": [0],
            "exit_ts": [86_400],
            "entry_price": [100.0],
            "exit_price": [101.0],
            "pnl": [1.0],
        }
    )
    try:
        result = float(instance.compute(equity, trades))
    except Exception as ex:  # noqa: BLE001
        return f"{ex.__class__.__name__}: {ex}"
    if not math.isfinite(result):
        return "Metric `compute` must return a finite float."
    return None


def _load_stored_metrics(cfg: Config) -> dict[str, BaseMetric]:
    """Load custom metric objects from local storage."""
    return _load_pickles(
        Path(cfg.data.storage_path) / "metrics",
        logger=logger,
        item_name="metric",
    )


def _save_metric(metric: BaseMetric, name: str, cfg: Config) -> None:
    """Persist a custom metric instance."""
    _save_pickle(
        metric,
        Path(cfg.data.storage_path) / "metrics",
        name,
        temporary_prefix=".metric-",
    )
