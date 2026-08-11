"""Backtide.

Author: Mavs
Description: Utilities for stored custom metrics.

"""

import ast
import inspect
import logging
import math
from pathlib import Path
from typing import Any

import cloudpickle
import pandas as pd

from backtide.config import Config
from backtide.metrics.base import BaseMetric

logger = logging.getLogger(__name__)


def _build_custom_metric(code: str) -> BaseMetric:
    """Execute code and return the final metric instance."""
    tree = ast.parse(code)
    if not tree.body or not isinstance(tree.body[-1], ast.Expr):
        raise ValueError("The last statement must be an instantiation of the metric.")
    namespace: dict[str, Any] = {}
    exec(compile(tree, "<metric>", "exec"), namespace)
    instance = eval(compile(ast.Expression(tree.body[-1].value), "<metric>", "eval"), namespace)
    if not isinstance(instance, BaseMetric):
        raise TypeError(f"Expected a subclass of BaseMetric, got {type(instance).__name__}.")
    instance._source_code = code
    return instance


def _check_metric_code(code: str) -> str | None:
    """Validate a custom metric signature and execute it on deterministic data."""
    try:
        instance = _build_custom_metric(code)
    except Exception as ex:  # noqa: BLE001
        return f"Failed to instantiate metric: {ex}"
    signature = inspect.signature(instance.compute)
    if list(signature.parameters) != ["equity_curve", "trades"]:
        return "Method `compute` must have signature: `compute(self, equity_curve, trades)`."
    if not isinstance(instance.percentage, bool):
        return "Metric `percentage` must be a bool."
    if not isinstance(instance.higher_is_better, bool):
        return "Metric `higher_is_better` must be a bool."
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
    values: dict[str, BaseMetric] = {}
    for file in sorted((Path(cfg.data.storage_path) / "metrics").glob("*.pkl")):
        try:
            with file.open("rb") as stream:
                values[file.stem] = cloudpickle.load(stream)
        except Exception as ex:  # noqa: BLE001
            logger.warning("Failed to load metric %s: %s", file.stem, ex)
    return values


def _save_metric(metric: BaseMetric, name: str, cfg: Config) -> None:
    """Persist a custom metric instance."""
    path = Path(cfg.data.storage_path) / "metrics"
    path.mkdir(parents=True, exist_ok=True)
    with (path / f"{name}.pkl").open("wb") as stream:
        cloudpickle.dump(metric, stream)
