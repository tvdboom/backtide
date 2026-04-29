"""Backtide.

Author: Mavs
Description: Helpers backing :func:`backtide.backtest.run_experiment`.

These utilities glue the Rust engine to the rest of the Python codebase:

* Auto-injecting the indicators required by the selected strategies.
* Persisting and loading the optional buy-and-hold benchmark side-car next to
  every experiment.
* Wrapping the core ``run_experiment`` to do both of the above transparently
  for any caller (UI, CLI, Python scripts).
"""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import Any

from backtide.config import Config, get_config
from backtide.core.backtest import ExperimentConfig, ExperimentResult
from backtide.core.backtest import run_experiment as _run_experiment_core

__all__ = [
    "_benchmark_sidecar_path",
    "_inject_auto_indicators",
    "_load_benchmark_sidecar",
    "_run_benchmark_sidecar",
    "_save_benchmark_sidecar",
    "run_experiment",
]

_log = logging.getLogger(__name__)


# ─────────────────────────────────────────────────────────────────────────────
# Auto-indicator injection
# ─────────────────────────────────────────────────────────────────────────────


def _inject_auto_indicators(
    config: ExperimentConfig, app_cfg: Config
) -> ExperimentConfig:
    """Resolve the indicators required by *config*'s strategies and append
    their on-disk names to ``config.indicators.indicators``.

    The strategies referenced in ``config.strategy.strategies`` are loaded
    from the local storage directory, looked up against
    ``backtide.strategies.STRATEGY_INDICATORS``, and the resulting indicators
    are pickled under deterministic ``__auto_*`` names so the engine can
    resolve them by name. Already-present names are not duplicated.
    """
    # Local imports keep the public ``backtide.backtest`` module light and
    # break a potential import cycle with ``backtide.strategies``.
    from backtide.strategies.utils import (
        _ensure_auto_indicators_saved,
        _load_strategies_by_name,
        _resolve_strategy_indicators,
    )

    names = list(config.strategy.strategies or [])
    if not names:
        return config

    strategies = _load_strategies_by_name(app_cfg, names)
    if not strategies:
        return config

    required = _resolve_strategy_indicators(strategies)
    if not required:
        return config

    auto_names = _ensure_auto_indicators_saved(app_cfg, required)
    if not auto_names:
        return config

    existing = list(config.indicators.indicators)
    merged = existing + [n for n in auto_names if n not in existing]
    if merged == existing:
        return config

    return ExperimentConfig.from_dict(
        {**config.to_dict(), "indicators": {"indicators": merged}}
    )


# ─────────────────────────────────────────────────────────────────────────────
# Benchmark side-car
# ─────────────────────────────────────────────────────────────────────────────


def _benchmark_sidecar_path(app_cfg: Config, experiment_id: str) -> Path:
    """Return the on-disk path of an experiment's benchmark sidecar JSON."""
    return Path(str(app_cfg.data.storage_path)) / "benchmarks" / f"{experiment_id}.json"


def _save_benchmark_sidecar(
    app_cfg: Config, experiment_id: str, payload: dict[str, Any]
) -> None:
    """Persist a benchmark sidecar payload as JSON next to the experiment."""
    path = _benchmark_sidecar_path(app_cfg, experiment_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload), encoding="utf-8")


def _load_benchmark_sidecar(
    app_cfg: Config, experiment_id: str
) -> dict[str, Any] | None:
    """Load the benchmark sidecar JSON for *experiment_id*, if any."""
    path = _benchmark_sidecar_path(app_cfg, experiment_id)
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None


def _run_benchmark_sidecar(
    config: ExperimentConfig,
    result: ExperimentResult,
    app_cfg: Config,
    *,
    verbose: bool,
) -> None:
    """Execute a passive Buy & Hold experiment on the configured benchmark
    symbol and persist a sidecar JSON with its equity curve and the alpha
    each strategy generated relative to it."""
    # Local import to avoid a circular dependency at module import time.
    from backtide.strategies import BuyAndHold

    bench_symbol = (config.strategy.benchmark or "").strip()
    if not bench_symbol or result.status != "completed":
        return

    bench_dict = config.to_dict()
    bench_dict["general"]["name"] = f"{config.general.name}__benchmark"
    bench_dict["general"]["tags"] = list(config.general.tags) + [
        f"benchmark_of:{result.experiment_id}"
    ]
    bench_dict["data"]["symbols"] = [bench_symbol]
    bench_dict["portfolio"]["starting_positions"] = []
    bench_dict["indicators"]["indicators"] = []
    bench_dict["strategy"]["strategies"] = [BuyAndHold()]
    bench_dict["strategy"]["benchmark"] = ""

    try:
        bench_cfg = ExperimentConfig.from_dict(bench_dict)
        bench_result = _run_experiment_core(bench_cfg, verbose=verbose)
    except Exception as ex:  # noqa: BLE001
        _log.warning("Benchmark run failed: %s", ex)
        return

    if not bench_result.strategies:
        return

    bench_run = bench_result.strategies[0]
    bench_total_return = float(bench_run.metrics.get("total_return", 0.0) or 0.0)
    bench_cagr = float(bench_run.metrics.get("cagr", 0.0) or 0.0)
    alpha_per_strategy = {
        run.strategy_name: float(run.metrics.get("total_return", 0.0) or 0.0)
        - bench_total_return
        for run in result.strategies
    }
    payload = {
        "symbol": bench_symbol,
        "total_return": bench_total_return,
        "cagr": bench_cagr,
        "equity_curve": [
            {"timestamp": int(s.timestamp), "equity": float(s.equity)}
            for s in bench_run.equity_curve
        ],
        "alpha_per_strategy": alpha_per_strategy,
    }
    try:
        _save_benchmark_sidecar(app_cfg, result.experiment_id, payload)
    except OSError as ex:
        _log.warning("Could not save benchmark sidecar: %s", ex)


# ─────────────────────────────────────────────────────────────────────────────
# Public entry point
# ─────────────────────────────────────────────────────────────────────────────


def run_experiment(
    config: ExperimentConfig,
    *,
    verbose: bool = True,
) -> ExperimentResult:
    """Run a backtest experiment.

    Performs the same end-to-end pipeline as the Rust core, but additionally:

    * **Auto-injects** every indicator required by the experiment's strategies
      (resolved via ``backtide.strategies.STRATEGY_INDICATORS`` and persisted
      under deterministic ``__auto_*`` names) so the engine can compute them
      once up-front alongside any user-selected indicators.
    * **Runs a benchmark side-car** when ``config.strategy.benchmark`` is a
      non-empty symbol: a passive Buy & Hold experiment on that symbol is
      executed and a JSON sidecar (equity curve + per-strategy alpha) is
      written to ``<storage_path>/benchmarks/<experiment_id>.json``.

    Parameters
    ----------
    config : [ExperimentConfig]
        The complete experiment configuration.

    verbose : bool, default=True
        Whether to display a progress bar while running.

    Returns
    -------
    [ExperimentResult]
        The aggregated result of the run.

    See Also
    --------
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:ExperimentResult
    - backtide.storage:query_experiments
    """
    app_cfg = get_config()
    config = _inject_auto_indicators(config, app_cfg)
    result = _run_experiment_core(config, verbose=verbose)
    _run_benchmark_sidecar(config, result, app_cfg, verbose=verbose)
    return result

