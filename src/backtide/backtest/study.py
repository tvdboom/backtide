"""Backtide.

Author: Mavs
Description: Backtest parameter-sweep and walk-forward studies.

"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from contextlib import contextmanager
from dataclasses import asdict, dataclass, replace
from datetime import UTC, date, datetime, timedelta
import inspect
from itertools import product
import json
import math
from pathlib import Path
from typing import TYPE_CHECKING, Any

from backtide.backtest.experiment import Experiment, ExperimentConfig

if TYPE_CHECKING:
    from collections.abc import Iterator

    from backtide.backtest.experiment import ExperimentResult, RunResult

_STUDY_FILENAME = "study.json"
_STUDY_SCHEMA_VERSION = 1
_MAX_CANDIDATES = 10_000


class _StudyProgress:
    """Map nested experiment progress onto candidate-run work units."""

    def __init__(
        self,
        callback: Callable[[float, int], None] | None,
        total: int,
    ) -> None:
        self.callback = callback
        self.total = total
        self.emit(0.0)

    def set_total(self, total: int, completed: float) -> None:
        """Publish a refined total after walk-forward folds are known."""
        self.total = total
        self.emit(completed)

    def stage(self, offset: float, weight: int) -> Callable[[int, int], None] | None:
        """Return a callback that maps one experiment onto part of the study."""
        if self.callback is None:
            return None

        def update(completed: int, total: int) -> None:
            fraction = min(max(completed / total, 0.0), 1.0) if total > 0 else 0.0
            self.emit(offset + fraction * weight)

        return update

    def emit(self, completed: float) -> None:
        """Send one bounded, monotonic-compatible study update."""
        if self.callback is not None:
            self.callback(min(max(completed, 0.0), float(self.total)), self.total)


@dataclass(frozen=True)
class WalkForwardConfig:
    """Configure rolling or anchored walk-forward validation.

    Parameters
    ----------
    training_days : int, default=1095
        Number of calendar days in each parameter-selection window.

    test_days : int, default=365
        Number of untouched calendar days immediately after each training window.

    step_days : int | None, default=None
        Days between consecutive folds. `None` advances by `test_days`.

    anchored : bool, default=False
        Keep the first training date fixed while expanding the training window.

    See Also
    --------
    - backtide.backtest:Study
    - backtide.backtest:StudyResult
    - backtide.backtest:WalkForwardFoldResult

    """

    training_days: int = 1095
    test_days: int = 365
    step_days: int | None = None
    anchored: bool = False

    def __post_init__(self) -> None:
        """Validate positive window sizes."""
        if self.training_days < 1:
            raise ValueError("training_days must be at least one.")
        if self.test_days < 1:
            raise ValueError("test_days must be at least one.")
        if self.step_days is not None and self.step_days < 1:
            raise ValueError("step_days must be at least one when provided.")


@dataclass(frozen=True)
class CandidateResult:
    """Summarize one parameter combination in a study.

    Attributes
    ----------
    candidate_id : str
        Stable identifier within the study.

    strategy_name : str
        Compact strategy-run name such as `C1` stored on the parent experiment.

    strategy_id : str
        Persisted strategy-run identifier.

    parameters : dict[str, object]
        Constructor values used to create the strategy instance.

    metrics : dict[str, float]
        Metrics produced by the experiment engine.

    trade_count : int
        Number of completed round-trip trades.

    eligible : bool
        Whether the candidate satisfied the configured constraints.

    rank : int | None
        One-based objective rank among eligible candidates.

    error : str | None
        Strategy or validation error, when present.

    See Also
    --------
    - backtide.backtest:RunResult
    - backtide.backtest:Study
    - backtide.backtest:StudyResult

    """

    candidate_id: str
    strategy_name: str
    strategy_id: str
    parameters: dict[str, Any]
    metrics: dict[str, float]
    trade_count: int
    eligible: bool
    rank: int | None
    error: str | None


@dataclass(frozen=True)
class WalkForwardFoldResult:
    """Summarize one walk-forward training and test fold.

    Attributes
    ----------
    fold : int
        One-based fold number.

    training_start : str
        Inclusive training-period start date.

    training_end : str
        Inclusive training-period end date.

    test_start : str
        Inclusive untouched test-period start date.

    test_end : str
        Inclusive untouched test-period end date.

    candidate_id : str | None
        Candidate selected using only the training period.

    parameters : dict[str, object]
        Constructor parameters selected for the test period.

    training_objective : float | None
        Selected candidate's objective on the training period.

    test_objective : float | None
        Selected candidate's objective on the test period.

    test_metrics : dict[str, float]
        Complete test-period metric mapping.

    trade_count : int
        Number of completed test-period trades.

    error : str | None
        Fold-level error, when present.

    See Also
    --------
    - backtide.backtest:CandidateResult
    - backtide.backtest:StudyResult
    - backtide.backtest:WalkForwardConfig

    """

    fold: int
    training_start: str
    training_end: str
    test_start: str
    test_end: str
    candidate_id: str | None
    parameters: dict[str, Any]
    training_objective: float | None
    test_objective: float | None
    test_metrics: dict[str, float]
    trade_count: int
    error: str | None


@dataclass(frozen=True)
class StudyResult:
    """Return the persisted result of a study.

    Attributes
    ----------
    study_id : str
        Identifier of the persisted study that owns its candidate experiments.

    name : str
        Study name inherited from the experiment configuration.

    strategy_name : str
        Saved or runtime strategy used to generate candidates.

    objective : str
        Main experiment metric used to rank eligible candidates.

    maximize : bool
        Whether larger objective values rank first.

    parameter_space : dict[str, list[object]]
        Ordered values evaluated for every swept constructor parameter.

    candidates : list[[CandidateResult]]
        Full-sample candidate summaries.

    folds : list[[WalkForwardFoldResult]]
        Walk-forward summaries. Empty when validation was not requested.

    best_candidate_id : str | None
        Highest-ranked eligible full-sample candidate.

    min_trades : int
        Minimum completed trades required for eligibility.

    max_drawdown : float | None
        Maximum permitted drawdown magnitude as a positive fraction.

    warnings : list[str]
        Non-fatal study and engine warnings.

    walk_forward : [WalkForwardConfig] | None
        Validation settings used by the study, when enabled.

    See Also
    --------
    - backtide.backtest:CandidateResult
    - backtide.storage:query_study
    - backtide.backtest:Study

    """

    study_id: str
    name: str
    strategy_name: str
    objective: str
    maximize: bool
    parameter_space: dict[str, list[Any]]
    candidates: list[CandidateResult]
    folds: list[WalkForwardFoldResult]
    best_candidate_id: str | None
    min_trades: int
    max_drawdown: float | None
    warnings: list[str]
    walk_forward: WalkForwardConfig | None = None

    @property
    def best_candidate(self) -> CandidateResult | None:
        """Return the highest-ranked eligible candidate."""
        return next(
            (
                candidate
                for candidate in self.candidates
                if candidate.candidate_id == self.best_candidate_id
            ),
            None,
        )

    def to_dict(self) -> dict[str, Any]:
        """Convert the study result to a JSON-compatible dictionary."""
        return {
            "schema_version": _STUDY_SCHEMA_VERSION,
            **asdict(self),
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> StudyResult:
        """Build a result from its persisted dictionary representation."""
        schema_version = int(value.get("schema_version", 0))
        if schema_version != _STUDY_SCHEMA_VERSION:
            raise ValueError(f"Unsupported study result schema version: {schema_version}.")
        return cls(
            study_id=str(value["study_id"]),
            name=str(value["name"]),
            strategy_name=str(value["strategy_name"]),
            objective=str(value["objective"]),
            maximize=bool(value["maximize"]),
            parameter_space={
                str(name): list(values) for name, values in dict(value["parameter_space"]).items()
            },
            candidates=[CandidateResult(**candidate) for candidate in value["candidates"]],
            folds=[WalkForwardFoldResult(**fold) for fold in value["folds"]],
            best_candidate_id=value.get("best_candidate_id"),
            min_trades=int(value["min_trades"]),
            max_drawdown=value.get("max_drawdown"),
            warnings=[str(warning) for warning in value.get("warnings", [])],
            walk_forward=(
                WalkForwardConfig(**dict(value["walk_forward"]))
                if value.get("walk_forward") is not None
                else None
            ),
        )


@dataclass(frozen=True)
class _CandidateSpec:
    candidate_id: str
    strategy_name: str
    parameters: dict[str, Any]


class Study:
    """Run a study with a parameter sweep and optional walk-forward validation.

    A study helps assess a strategy's robustness by comparing multiple
    experiments across a parameter neighborhood. Full-sample candidate
    experiments are persisted under one study record. Walk-forward training and
    test experiments are temporary: Backtide summarizes each fold into the study
    and removes the temporary experiments immediately.

    Parameters
    ----------
    config : [ExperimentConfig] | None, default=None
        Complete experiment settings shared by every candidate.

    strategy : str | [BaseStrategy] | None, default=None
        Saved strategy name or runtime strategy instance. When omitted, exactly
        one saved strategy must be selected in `config`.

    parameter_space : dict[str, sequence[object]] | None, default=None
        Constructor parameter values whose Cartesian product forms the sweep.

    min_trades : int, default=0
        Exclude candidates with fewer completed trades.

    max_drawdown : float | None, default=None
        Exclude candidates whose drawdown magnitude exceeds this positive
        fraction. For example, `0.25` permits at most a 25% drawdown.

    walk_forward : [WalkForwardConfig] | None, default=None
        Optional rolling or anchored out-of-sample validation.

    See Also
    --------
    - backtide.backtest:Experiment
    - backtide.storage:query_study
    - backtide.backtest:StudyResult

    Examples
    --------
    ```pycon
    from math import prod

    from backtide import ExperimentConfig, Study, WalkForwardConfig

    config = ExperimentConfig.from_dict(
        {
            "general": {"name": "SMA parameter study"},
            "data": {
                "symbols": ["SPY"],
                "full_history": False,
                "start_date": "2012-01-01",
                "end_date": "2025-12-31",
            },
            "strategy": {"strategies": ["My SMA strategy"]},
        }
    )
    study = Study(
        config,
        parameter_space={"fast": [10, 20, 30], "slow": [100, 150, 200]},
        min_trades=30,
        walk_forward=WalkForwardConfig(training_days=1095, test_days=365),
    )
    print(f"{prod(len(values) for values in study.parameter_space.values())} candidates")
    result = study.run(verbose=False)  # norun
    ```

    """

    def __init__(
        self,
        config: ExperimentConfig | None = None,
        strategy: Any = None,
        parameter_space: Mapping[str, Sequence[Any]] | None = None,
        *,
        min_trades: int = 0,
        max_drawdown: float | None = None,
        walk_forward: WalkForwardConfig | None = None,
    ) -> None:
        self.config = config or ExperimentConfig()
        self.strategy = strategy
        self.parameter_space = dict(parameter_space or {})
        self.objective, self.maximize = self._objective_settings()
        self.min_trades = min_trades
        self.max_drawdown = max_drawdown
        self.walk_forward = walk_forward

    def run(
        self,
        *,
        verbose: bool = True,
        progress_callback: Callable[[float, int], None] | None = None,
    ) -> StudyResult:
        """Run and persist the study.

        Parameters
        ----------
        verbose : bool, default=True
            Show experiment progress output.

        progress_callback : Callable[[float, int], None] | None, default=None
            Receive `(completed, total)` candidate-run progress across the
            full-sample sweep and every walk-forward experiment.

        Returns
        -------
        [StudyResult]
            Candidate rankings, walk-forward folds, and study id.

        """
        self._validate()
        source_name, template = self._resolve_strategy()
        parameter_space = self._normalized_parameter_space()
        specs = self._candidate_specs(template, parameter_space)
        progress = _StudyProgress(progress_callback, len(specs))
        study_config = self._config_for_run(self.config, self.config.general.name)
        full_sample = Experiment(
            study_config,
            strategies=self._candidate_instances(template, specs),
        ).run(
            verbose=verbose,
            progress_callback=progress.stage(0, len(specs)),
        )
        progress.emit(float(len(specs)))
        candidates = self._summarize(full_sample, specs)
        best_candidate_id = next(
            (candidate.candidate_id for candidate in candidates if candidate.rank == 1),
            None,
        )
        warnings = [str(warning) for warning in full_sample.warnings]
        folds: list[WalkForwardFoldResult] = []
        if self.walk_forward is not None:
            date_folds = self._date_folds(full_sample)
            fold_weight = len(specs) + 1
            progress.set_total(len(specs) + len(date_folds) * fold_weight, len(specs))
            for fold_index, fold in enumerate(date_folds):
                fold_offset = len(specs) + fold_index * fold_weight
                try:
                    folds.append(
                        self._run_fold(
                            fold,
                            template,
                            specs,
                            verbose=verbose,
                            training_progress_callback=progress.stage(
                                fold_offset,
                                len(specs),
                            ),
                            test_progress_callback=progress.stage(
                                fold_offset + len(specs),
                                1,
                            ),
                        )
                    )
                except Exception as exc:  # noqa: BLE001
                    folds.append(
                        WalkForwardFoldResult(
                            fold=fold[0],
                            training_start=fold[1].isoformat(),
                            training_end=fold[2].isoformat(),
                            test_start=fold[3].isoformat(),
                            test_end=fold[4].isoformat(),
                            candidate_id=None,
                            parameters={},
                            training_objective=None,
                            test_objective=None,
                            test_metrics={},
                            trade_count=0,
                            error=str(exc),
                        )
                    )
                    warnings.append(f"Walk-forward fold {fold[0]} failed: {exc}")
                finally:
                    progress.emit(float(fold_offset + fold_weight))

        result = StudyResult(
            study_id=full_sample.experiment_id,
            name=full_sample.name,
            strategy_name=source_name,
            objective=self.objective,
            maximize=self.maximize,
            parameter_space=parameter_space,
            candidates=candidates,
            folds=folds,
            best_candidate_id=best_candidate_id,
            min_trades=self.min_trades,
            max_drawdown=self.max_drawdown,
            warnings=warnings,
            walk_forward=self.walk_forward,
        )
        _write_result(result)
        return result

    def _validate(self) -> None:
        """Validate study-level values before running any experiment."""
        if self.min_trades < 0:
            raise ValueError("min_trades must be zero or greater.")
        if self.max_drawdown is not None and not 0 <= self.max_drawdown <= 1:
            raise ValueError("max_drawdown must be between zero and one.")

    def _objective_settings(self) -> tuple[str, bool]:
        """Return the main metric name and its declared ranking direction."""
        if not self.config.metrics:
            raise ValueError("A study requires at least one experiment metric.")
        configured = self.config.metrics[0]
        if isinstance(configured, str):
            from backtide.config import get_config
            from backtide.metrics import BUILTIN_METRICS
            from backtide.metrics.utils import (
                _load_stored_metrics,
                _metric_greater_is_better,
            )

            definition = next(
                (metric for metric in BUILTIN_METRICS if metric.key == configured),
                None,
            )
            if definition is not None:
                return configured, bool(definition.higher_is_better)
            custom = _load_stored_metrics(get_config()).get(configured)
            if custom is None:
                raise ValueError(f"Main metric {configured!r} was not found.")
            return configured, _metric_greater_is_better(custom)
        if isinstance(configured, Mapping):
            if len(configured) != 1:
                raise ValueError("A custom metric entry must contain exactly one metric.")
            name, metric = next(iter(configured.items()))
        else:
            name, metric = type(configured).__name__, configured
        from backtide.metrics.utils import _metric_greater_is_better

        return str(name), _metric_greater_is_better(metric)

    def _resolve_strategy(self) -> tuple[str, Any]:
        """Resolve the configured strategy name or runtime instance."""
        if self.strategy is not None and not isinstance(self.strategy, str):
            return type(self.strategy).__name__, self.strategy
        name = self.strategy
        if name is None:
            names = list(self.config.strategy.strategies)
            if len(names) != 1:
                raise ValueError("A study requires exactly one strategy.")
            name = names[0]
        from backtide.config import get_config
        from backtide.strategies.utils import _load_stored_strategies

        stored = _load_stored_strategies(get_config())
        if name not in stored:
            raise ValueError(f"Saved strategy {name!r} was not found.")
        return str(name), stored[name]

    def _normalized_parameter_space(self) -> dict[str, list[Any]]:
        """Return validated ordered parameter values."""
        normalized: dict[str, list[Any]] = {}
        combinations = 1
        for name, values in self.parameter_space.items():
            if not isinstance(name, str) or not name.strip():
                raise ValueError("Parameter names must be non-empty strings.")
            if isinstance(values, (str, bytes)) or not isinstance(values, Sequence):
                raise ValueError(f"Parameter {name!r} values must be a sequence.")
            items = list(values)
            if not items:
                raise ValueError(f"Parameter {name!r} must contain at least one value.")
            normalized[name] = items
            combinations *= len(items)
            if combinations > _MAX_CANDIDATES:
                raise ValueError(f"A study supports at most {_MAX_CANDIDATES:,} candidates.")
        if not normalized:
            raise ValueError("parameter_space must contain at least one parameter.")
        return normalized

    @staticmethod
    def _candidate_specs(
        template: Any,
        parameter_space: Mapping[str, list[Any]],
    ) -> list[_CandidateSpec]:
        """Build deterministic candidate specifications from a constructor."""
        signature = inspect.signature(type(template))
        parameters = signature.parameters
        unknown = sorted(set(parameter_space) - set(parameters))
        if unknown:
            raise ValueError(f"Unknown strategy parameter(s): {', '.join(unknown)}.")
        base: dict[str, Any] = {}
        for name, parameter in parameters.items():
            if parameter.kind in {
                inspect.Parameter.VAR_POSITIONAL,
                inspect.Parameter.VAR_KEYWORD,
            }:
                if name in parameter_space:
                    raise ValueError(f"Variadic parameter {name!r} cannot be swept.")
                continue
            if parameter.kind is inspect.Parameter.POSITIONAL_ONLY:
                raise ValueError(f"Positional-only parameter {name!r} cannot be reconstructed.")
            if name in parameter_space:
                continue
            if hasattr(template, name):
                base[name] = getattr(template, name)
            elif parameter.default is not inspect.Parameter.empty:
                base[name] = parameter.default
            else:
                raise ValueError(
                    f"Strategy parameter {name!r} must be swept or stored on the instance."
                )

        names = list(parameter_space)
        specs = []
        combinations = product(*(parameter_space[name] for name in names))
        for index, values in enumerate(combinations, start=1):
            swept = dict(zip(names, values, strict=True))
            candidate_id = f"candidate-{index}"
            label = f"C{index}"
            specs.append(
                _CandidateSpec(
                    candidate_id=candidate_id,
                    strategy_name=label,
                    parameters={**base, **swept},
                )
            )
        return specs

    @staticmethod
    def _candidate_instances(
        template: Any,
        specs: Sequence[_CandidateSpec],
    ) -> dict[str, Any]:
        """Create one isolated strategy instance per candidate."""
        return {spec.strategy_name: type(template)(**spec.parameters) for spec in specs}

    def _summarize(
        self,
        result: ExperimentResult,
        specs: Sequence[_CandidateSpec],
    ) -> list[CandidateResult]:
        """Convert engine runs to ranked candidate summaries."""
        runs = {run.strategy_name: run for run in result.strategies if not run.is_benchmark}
        candidates: list[CandidateResult] = []
        for spec in specs:
            run = runs.get(spec.strategy_name)
            if run is None:
                candidates.append(
                    CandidateResult(
                        candidate_id=spec.candidate_id,
                        strategy_name=spec.strategy_name,
                        strategy_id="",
                        parameters=dict(spec.parameters),
                        metrics={},
                        trade_count=0,
                        eligible=False,
                        rank=None,
                        error="The experiment did not return this candidate.",
                    )
                )
                continue
            metrics = {
                str(key): parsed
                for key, value in run.metrics.items()
                if math.isfinite(parsed := float(value))
            }
            trade_count = len(run.trades)
            drawdown = self._drawdown(run, metrics)
            objective = metrics.get(self.objective)
            eligible = (
                run.error is None
                and objective is not None
                and math.isfinite(objective)
                and trade_count >= self.min_trades
                and (
                    self.max_drawdown is None
                    or drawdown is None
                    or abs(min(drawdown, 0.0)) <= self.max_drawdown
                )
            )
            candidates.append(
                CandidateResult(
                    candidate_id=spec.candidate_id,
                    strategy_name=spec.strategy_name,
                    strategy_id=run.strategy_id,
                    parameters=dict(spec.parameters),
                    metrics=metrics,
                    trade_count=trade_count,
                    eligible=eligible,
                    rank=None,
                    error=run.error,
                )
            )

        ranked = sorted(
            (candidate for candidate in candidates if candidate.eligible),
            key=lambda candidate: candidate.metrics[self.objective],
            reverse=self.maximize,
        )
        ranks = {candidate.candidate_id: index for index, candidate in enumerate(ranked, start=1)}
        return [
            replace(candidate, rank=ranks.get(candidate.candidate_id)) for candidate in candidates
        ]

    @staticmethod
    def _drawdown(run: RunResult, metrics: Mapping[str, float]) -> float | None:
        """Return maximum drawdown as a signed fraction."""
        for key in ("max_dd", "max_drawdown"):
            if key in metrics:
                return metrics[key]
        values = [float(sample.drawdown) for sample in run.equity_curve]
        return min(values) if values else None

    def _config_for_run(
        self,
        source: ExperimentConfig,
        name: str,
        *,
        start_date: date | None = None,
        end_date: date | None = None,
    ) -> ExperimentConfig:
        """Clone the shared experiment config for one study execution."""
        payload = source.to_dict()
        payload["general"]["name"] = name
        payload["strategy"]["strategies"] = []
        if start_date is not None and end_date is not None:
            payload["data"].update(
                {
                    "full_history": False,
                    "start_date": start_date.isoformat(),
                    "end_date": end_date.isoformat(),
                }
            )
        config = ExperimentConfig.from_dict(payload)
        config.metrics = source.metrics
        return config

    def _date_folds(
        self,
        result: ExperimentResult,
    ) -> list[tuple[int, date, date, date, date]]:
        """Build complete folds from the history returned by the parent run."""
        if self.walk_forward is None:
            return []
        timestamps = (
            int(sample.timestamp) for run in result.strategies for sample in run.equity_curve
        )
        first_timestamp = next(timestamps, None)
        if first_timestamp is not None:
            earliest = latest = first_timestamp
            for timestamp in timestamps:
                earliest = min(earliest, timestamp)
                latest = max(latest, timestamp)
            start = datetime.fromtimestamp(earliest, tz=UTC).date()
            end = datetime.fromtimestamp(latest, tz=UTC).date()
        elif self.config.data.start_date and self.config.data.end_date:
            start = date.fromisoformat(str(self.config.data.start_date))
            end = date.fromisoformat(str(self.config.data.end_date))
        else:
            raise ValueError(
                "Available history could not be determined from the parent experiment."
            )
        step_days = self.walk_forward.step_days or self.walk_forward.test_days
        folds = []
        offset = 0
        while True:
            training_start = (
                start if self.walk_forward.anchored else start + timedelta(days=offset)
            )
            training_end = start + timedelta(days=offset + self.walk_forward.training_days - 1)
            test_start = training_end + timedelta(days=1)
            test_end = test_start + timedelta(days=self.walk_forward.test_days - 1)
            if test_end > end:
                break
            folds.append((len(folds) + 1, training_start, training_end, test_start, test_end))
            offset += step_days
        if not folds:
            raise ValueError("The configured date range does not contain one complete fold.")
        return folds

    def _run_fold(
        self,
        fold: tuple[int, date, date, date, date],
        template: Any,
        specs: Sequence[_CandidateSpec],
        *,
        verbose: bool,
        training_progress_callback: Callable[[int, int], None] | None = None,
        test_progress_callback: Callable[[int, int], None] | None = None,
    ) -> WalkForwardFoldResult:
        """Select on the training window and evaluate once on the test window."""
        number, training_start, training_end, test_start, test_end = fold
        training_config = self._config_for_run(
            self.config,
            f"{self.config.general.name} · fold {number} training",
            start_date=training_start,
            end_date=training_end,
        )
        with _temporary_experiment(
            Experiment(
                training_config,
                strategies=self._candidate_instances(template, specs),
            ),
            verbose=verbose,
            progress_callback=training_progress_callback,
        ) as training:
            training_candidates = self._summarize(training, specs)
        selected = next(
            (candidate for candidate in training_candidates if candidate.rank == 1),
            None,
        )
        if selected is None:
            raise ValueError("No candidate satisfied the training constraints.")
        selected_spec = next(spec for spec in specs if spec.candidate_id == selected.candidate_id)
        test_config = self._config_for_run(
            self.config,
            f"{self.config.general.name} · fold {number} test",
            start_date=test_start,
            end_date=test_end,
        )
        with _temporary_experiment(
            Experiment(
                test_config,
                strategies=self._candidate_instances(
                    template,
                    [selected_spec],
                ),
            ),
            verbose=verbose,
            progress_callback=test_progress_callback,
        ) as test:
            test_candidate = self._summarize(test, [selected_spec])[0]
        return WalkForwardFoldResult(
            fold=number,
            training_start=training_start.isoformat(),
            training_end=training_end.isoformat(),
            test_start=test_start.isoformat(),
            test_end=test_end.isoformat(),
            candidate_id=selected.candidate_id,
            parameters=dict(selected.parameters),
            training_objective=selected.metrics.get(self.objective),
            test_objective=test_candidate.metrics.get(self.objective),
            test_metrics=dict(test_candidate.metrics),
            trade_count=test_candidate.trade_count,
            error=test_candidate.error,
        )


@contextmanager
def _temporary_experiment(
    experiment: Experiment,
    *,
    verbose: bool,
    progress_callback: Callable[[int, int], None] | None = None,
) -> Iterator[ExperimentResult]:
    """Run an experiment and remove its persisted rows and artifacts afterward."""
    result = experiment.run(verbose=verbose, progress_callback=progress_callback)
    try:
        yield result
    finally:
        from backtide.storage import delete_experiment

        delete_experiment(result.experiment_id)


def _result_path(study_id: str) -> Path:
    """Return the validated sidecar path for one study."""
    from backtide.config import get_config

    root = (Path(get_config().data.storage_path) / "experiments").resolve()
    path = (root / study_id / _STUDY_FILENAME).resolve()
    if root not in path.parents:
        raise ValueError("Invalid experiment id.")
    return path


def _write_result(result: StudyResult) -> None:
    """Atomically persist one study result beside its parent experiment."""
    path = _result_path(result.study_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    temporary.write_text(
        json.dumps(result.to_dict(), indent=2, sort_keys=True, allow_nan=False),
        encoding="utf-8",
    )
    temporary.replace(path)


__all__ = [
    "CandidateResult",
    "Study",
    "StudyResult",
    "WalkForwardConfig",
    "WalkForwardFoldResult",
]
