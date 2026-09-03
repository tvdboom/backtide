"""Backtide.

Author: Mavs
Description: Tests for backtest parameter-sweep and walk-forward studies.

"""

from __future__ import annotations

from types import SimpleNamespace
from typing import Any, cast

import pytest

from backtide.backtest import ExperimentConfig
import backtide.backtest.study as study_module
from backtide.backtest.study import (
    Study,
    StudyResult,
    WalkForwardConfig,
)
from backtide.metrics import BaseMetric
from backtide.storage import query_study


class TunableStrategy:
    """Small constructor-driven strategy used by orchestration tests."""

    def __init__(self, lookback: int = 10, threshold: float = 0.5):
        self.lookback = lookback
        self.threshold = threshold


def _config(*, walk_forward: bool = False) -> ExperimentConfig:
    """Return a deterministic experiment configuration."""
    data = {"symbols": ["SPY"]}
    if walk_forward:
        data.update(
            {
                "full_history": False,
                "start_date": "2020-01-01",
                "end_date": "2020-01-10",
            }
        )
    return ExperimentConfig.from_dict(
        {
            "general": {"name": "Study test"},
            "data": data,
            "strategy": {"strategies": ["Saved strategy"]},
            "metrics": ["sharpe", "max_dd", "n_trades"],
        }
    )


def _fake_run_factory(calls):
    """Return an Experiment.run replacement that scores constructor values."""

    def run(experiment, *, verbose=True, progress_callback=None):
        del verbose
        calls.append(experiment)
        if progress_callback is not None:
            progress_callback(0, len(experiment.strategies))
        runs = []
        for index, (name, strategy) in enumerate(experiment.strategies.items(), start=1):
            score = float(strategy.lookback) + float(strategy.threshold)
            runs.append(
                SimpleNamespace(
                    strategy_id=f"run-{len(calls)}-{index}",
                    strategy_name=name,
                    metrics={
                        "sharpe": score,
                        "risk_score": score,
                        "max_dd": -0.1,
                        "n_trades": 3.0,
                    },
                    trades=[object(), object(), object()],
                    equity_curve=[
                        SimpleNamespace(timestamp=1_577_836_800, drawdown=0.0),
                        SimpleNamespace(timestamp=1_578_614_400, drawdown=-0.1),
                    ],
                    error=None,
                    is_benchmark=False,
                )
            )
        result = SimpleNamespace(
            experiment_id=f"experiment-{len(calls)}",
            name=experiment.config.general.name,
            warnings=[],
            strategies=runs,
        )
        if progress_callback is not None:
            progress_callback(len(experiment.strategies), len(experiment.strategies))
        return result

    return run


class TestStudy:
    """Tests for the public study orchestration."""

    def test_sweep_persists_one_parent_with_ranked_candidates(
        self,
        monkeypatch,
        tmp_path,
    ):
        """A Cartesian sweep remains one user-visible parent experiment."""
        calls = []
        result_path = tmp_path / "experiment-1" / "study.json"
        monkeypatch.setattr(
            "backtide.backtest.study.Experiment.run",
            _fake_run_factory(calls),
        )
        monkeypatch.setattr(
            "backtide.backtest.study._result_path",
            lambda _experiment_id: result_path,
        )

        updates = []
        result = Study(
            _config(),
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10, 20], "threshold": [0.5, 1.0]},
            min_trades=2,
        ).run(verbose=False, progress_callback=lambda done, total: updates.append((done, total)))

        assert len(calls) == 1
        assert len(calls[0].strategies) == 4
        assert len({id(strategy) for strategy in calls[0].strategies.values()}) == 4
        assert len(result.candidates) == 4
        assert list(calls[0].strategies) == ["C1", "C2", "C3", "C4"]
        assert [candidate.candidate_id for candidate in result.candidates] == [
            "candidate-1",
            "candidate-2",
            "candidate-3",
            "candidate-4",
        ]
        assert result.best_candidate is not None
        assert result.best_candidate.parameters == {"lookback": 20, "threshold": 1.0}
        assert result.best_candidate.rank == 1
        assert result_path.is_file()
        assert updates[-1] == (4.0, 4)

    def test_custom_main_metric_declares_smaller_values_as_better(
        self,
        monkeypatch,
        tmp_path,
    ):
        """The first custom metric controls both the objective and ranking direction."""

        class RiskScore(BaseMetric):
            """Return a risk score where smaller values are preferred."""

            greater_is_better = False

            def compute(self, equity_curve, trades) -> float:
                """Return a fixed score for this orchestration test."""
                del equity_curve, trades
                return 1.0

        calls = []
        config = _config()
        config.metrics = [{"risk_score": RiskScore()}]
        monkeypatch.setattr(
            "backtide.backtest.study.Experiment.run",
            _fake_run_factory(calls),
        )
        monkeypatch.setattr(
            "backtide.backtest.study._result_path",
            lambda experiment_id: tmp_path / experiment_id / "study.json",
        )

        result = Study(
            config,
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10, 20]},
        ).run(verbose=False)

        assert result.objective == "risk_score"
        assert result.maximize is False
        assert result.best_candidate is not None
        assert result.best_candidate.parameters["lookback"] == 10

    def test_walk_forward_summarizes_and_deletes_temporary_experiments(
        self,
        monkeypatch,
        tmp_path,
    ):
        """Walk-forward folds retain summaries while removing child experiments."""
        calls = []
        deleted = []
        monkeypatch.setattr(
            "backtide.backtest.study.Experiment.run",
            _fake_run_factory(calls),
        )
        monkeypatch.setattr(
            "backtide.backtest.study._result_path",
            lambda experiment_id: tmp_path / experiment_id / "study.json",
        )
        monkeypatch.setattr("backtide.storage.delete_experiment", deleted.append)

        result = Study(
            _config(walk_forward=True),
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10, 20]},
            walk_forward=WalkForwardConfig(training_days=4, test_days=2, step_days=2),
        ).run(verbose=False)

        assert len(result.folds) == 3
        assert [fold.test_start for fold in result.folds] == [
            "2020-01-05",
            "2020-01-07",
            "2020-01-09",
        ]
        assert all(fold.candidate_id == "candidate-2" for fold in result.folds)
        assert result.walk_forward == WalkForwardConfig(
            training_days=4,
            test_days=2,
            step_days=2,
        )
        assert len(calls) == 7
        assert deleted == [f"experiment-{index}" for index in range(2, 8)]

    def test_walk_forward_uses_the_available_full_history(self, monkeypatch, tmp_path):
        """Full-history studies derive fold boundaries from the parent equity samples."""
        calls = []
        monkeypatch.setattr(
            "backtide.backtest.study.Experiment.run",
            _fake_run_factory(calls),
        )
        monkeypatch.setattr(
            "backtide.backtest.study._result_path",
            lambda experiment_id: tmp_path / experiment_id / "study.json",
        )
        monkeypatch.setattr("backtide.storage.delete_experiment", lambda _experiment_id: None)

        result = Study(
            _config(),
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10, 20]},
            walk_forward=WalkForwardConfig(training_days=4, test_days=2, step_days=2),
        ).run(verbose=False)

        assert [fold.test_start for fold in result.folds] == [
            "2020-01-05",
            "2020-01-07",
            "2020-01-09",
        ]

    def test_rejects_constructor_values_that_cannot_be_recovered(self):
        """Required fixed values must be stored on the saved strategy instance."""

        class MissingValue:
            def __init__(self, required):
                pass

        study = Study(
            _config(),
            strategy=MissingValue(5),
            parameter_space={"other": [1]},
        )

        with pytest.raises(ValueError, match="Unknown strategy parameter"):
            study.run(verbose=False)

    @pytest.mark.parametrize(
        "values",
        [
            {"training_days": 0},
            {"test_days": 0},
            {"step_days": 0},
        ],
    )
    def test_walk_forward_rejects_non_positive_windows(self, values):
        """Walk-forward window sizes must be positive."""
        with pytest.raises(ValueError, match="must be at least one"):
            WalkForwardConfig(**values)

    @pytest.mark.parametrize(
        ("changes", "message"),
        [
            ({"min_trades": -1}, "min_trades"),
            ({"max_drawdown": 1.1}, "max_drawdown"),
        ],
    )
    def test_study_constraints_are_validated(self, changes, message):
        """Study trade and drawdown constraints reject out-of-range values."""
        study = Study(
            _config(),
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10]},
            **changes,
        )

        with pytest.raises(ValueError, match=message):
            study._validate()

    @pytest.mark.parametrize(
        ("parameter_space", "message"),
        [
            ({"": [1]}, "non-empty strings"),
            ({"lookback": "bad"}, "must be a sequence"),
            ({"lookback": []}, "at least one value"),
            ({}, "at least one parameter"),
            ({"lookback": list(range(101)), "threshold": list(range(101))}, "at most 10,000"),
        ],
    )
    def test_parameter_space_validation(self, parameter_space, message):
        """Parameter spaces reject malformed names, values, and excessive products."""
        study = Study(_config(), strategy=TunableStrategy(), parameter_space=parameter_space)

        with pytest.raises(ValueError, match=message):
            study._normalized_parameter_space()

    def test_candidate_specs_reject_unreconstructable_parameters(self):
        """Candidate reconstruction rejects variadic, positional-only, and missing values."""

        class Variadic:
            def __init__(self, *values):
                del values

        class PositionalOnly:
            def __init__(self, value, /):
                self.value = value

        class Required:
            def __init__(self, value, other=0):
                del value, other

        with pytest.raises(ValueError, match="Variadic parameter"):
            Study._candidate_specs(Variadic(), {"values": [1]})
        with pytest.raises(ValueError, match="Positional-only parameter"):
            Study._candidate_specs(PositionalOnly(1), {})
        with pytest.raises(ValueError, match="must be swept or stored"):
            Study._candidate_specs(Required(1), {"other": [1]})

    def test_missing_engine_run_is_an_ineligible_candidate(self):
        """A candidate omitted by the engine receives an explicit error summary."""
        study = Study(
            _config(),
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10]},
        )
        specs = study._candidate_specs(TunableStrategy(), {"lookback": [10]})
        result = SimpleNamespace(strategies=[])

        candidates = study._summarize(cast(Any, result), specs)

        assert candidates[0].eligible is False
        assert candidates[0].error == "The experiment did not return this candidate."

    def test_drawdown_falls_back_to_equity_samples(self):
        """Drawdown summaries use equity samples when no drawdown metric exists."""
        run = SimpleNamespace(
            equity_curve=[SimpleNamespace(drawdown=-0.2), SimpleNamespace(drawdown=-0.1)]
        )

        assert Study._drawdown(cast(Any, run), {}) == -0.2
        assert Study._drawdown(cast(Any, SimpleNamespace(equity_curve=[])), {}) is None

    def test_date_folds_report_unavailable_and_short_history(self):
        """Walk-forward validation explains missing or insufficient date ranges."""
        study = Study(
            _config(),
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10]},
            walk_forward=WalkForwardConfig(training_days=4, test_days=2),
        )

        with pytest.raises(ValueError, match="could not be determined"):
            study._date_folds(cast(Any, SimpleNamespace(strategies=[])))

        short = SimpleNamespace(
            strategies=[SimpleNamespace(equity_curve=[SimpleNamespace(timestamp=1_577_836_800)])]
        )
        with pytest.raises(ValueError, match="does not contain one complete fold"):
            study._date_folds(cast(Any, short))

    def test_failed_walk_forward_fold_is_summarized(self, monkeypatch, tmp_path):
        """A failed fold remains visible and does not discard the parent study."""
        calls = []
        monkeypatch.setattr(
            "backtide.backtest.study.Experiment.run",
            _fake_run_factory(calls),
        )
        monkeypatch.setattr(
            "backtide.backtest.study._result_path",
            lambda experiment_id: tmp_path / experiment_id / "study.json",
        )
        monkeypatch.setattr(
            Study,
            "_date_folds",
            lambda _self, _result: [
                (
                    1,
                    study_module.date(2020, 1, 1),
                    study_module.date(2020, 1, 4),
                    study_module.date(2020, 1, 5),
                    study_module.date(2020, 1, 6),
                )
            ],
        )
        monkeypatch.setattr(Study, "_run_fold", lambda *_args, **_kwargs: 1 / 0)

        result = Study(
            _config(),
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10]},
            walk_forward=WalkForwardConfig(training_days=4, test_days=2),
        ).run(verbose=False)

        assert result.folds[0].error == "division by zero"
        assert result.warnings[-1] == "Walk-forward fold 1 failed: division by zero"


class TestStudyResolution:
    """Tests for resolving study metrics and saved strategies."""

    class ScoreMetric(BaseMetric):
        """Return a fixed score for resolution tests."""

        greater_is_better = False

        def compute(self, equity_curve, trades):
            """Return one deterministic score."""
            del equity_curve, trades
            return 1.0

    def test_study_requires_a_metric(self):
        """An empty metric configuration cannot define a study objective."""
        config = _config()
        config.metrics = []

        with pytest.raises(ValueError, match="at least one experiment metric"):
            Study(config, strategy=TunableStrategy(), parameter_space={"lookback": [10]})

    def test_saved_custom_metric_controls_direction(self, monkeypatch):
        """A named saved metric supplies its ranking direction."""
        config = _config()
        config.metrics = ["saved_score"]
        monkeypatch.setattr(
            "backtide.metrics.utils._load_stored_metrics",
            lambda _config: {"saved_score": self.ScoreMetric()},
        )

        study = Study(config, strategy=TunableStrategy(), parameter_space={"lookback": [10]})

        assert (study.objective, study.maximize) == ("saved_score", False)

    def test_unknown_saved_metric_is_rejected(self, monkeypatch):
        """A missing named custom metric produces a specific validation error."""
        config = _config()
        config.metrics = ["missing"]
        monkeypatch.setattr("backtide.metrics.utils._load_stored_metrics", lambda _config: {})

        with pytest.raises(ValueError, match="Main metric 'missing' was not found"):
            Study(config, strategy=TunableStrategy(), parameter_space={"lookback": [10]})

    def test_custom_metric_mapping_requires_one_entry(self):
        """Inline custom metric mappings contain exactly one named instance."""
        study = object.__new__(Study)
        cast(Any, study).config = SimpleNamespace(
            metrics=[{"one": self.ScoreMetric(), "two": self.ScoreMetric()}]
        )

        with pytest.raises(ValueError, match="exactly one metric"):
            study._objective_settings()

    def test_bare_custom_metric_uses_its_class_name(self):
        """A bare custom metric derives the study objective from its class name."""
        config = _config()
        config.metrics = [self.ScoreMetric()]

        study = Study(config, strategy=TunableStrategy(), parameter_space={"lookback": [10]})

        assert (study.objective, study.maximize) == ("ScoreMetric", False)

    def test_saved_strategy_is_loaded_by_name(self, monkeypatch):
        """A configured saved strategy name resolves to its persisted instance."""
        template = TunableStrategy()
        monkeypatch.setattr(
            "backtide.strategies.utils._load_stored_strategies",
            lambda _config: {"Saved strategy": template},
        )
        study = Study(_config(), parameter_space={"lookback": [10]})

        assert study._resolve_strategy() == ("Saved strategy", template)

    def test_strategy_selection_requires_exactly_one_saved_name(self):
        """Implicit strategy resolution requires exactly one configured name."""
        study = object.__new__(Study)
        study.strategy = None
        cast(Any, study).config = SimpleNamespace(strategy=SimpleNamespace(strategies=[]))

        with pytest.raises(ValueError, match="exactly one strategy"):
            study._resolve_strategy()

    def test_missing_saved_strategy_is_rejected(self, monkeypatch):
        """An unavailable saved strategy name produces a specific error."""
        monkeypatch.setattr("backtide.strategies.utils._load_stored_strategies", lambda _cfg: {})
        study = Study(_config(), parameter_space={"lookback": [10]})

        with pytest.raises(ValueError, match="Saved strategy 'Saved strategy' was not found"):
            study._resolve_strategy()


class TestStudyPersistence:
    """Tests for study sidecar loading."""

    def test_query_round_trips_a_persisted_result(self, monkeypatch, tmp_path):
        """The public query reconstructs nested candidate and fold models."""
        calls = []
        path = tmp_path / "study.json"
        monkeypatch.setattr(
            "backtide.backtest.study.Experiment.run",
            _fake_run_factory(calls),
        )
        monkeypatch.setattr("backtide.backtest.study._result_path", lambda _experiment_id: path)
        expected = Study(
            _config(),
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10, 20]},
        ).run(verbose=False)

        actual = query_study(expected.study_id)

        assert isinstance(actual, StudyResult)
        assert actual == expected

    def test_query_returns_none_for_a_regular_experiment(self, monkeypatch, tmp_path):
        """Regular experiments have no study sidecar."""
        monkeypatch.setattr(
            "backtide.backtest.study._result_path",
            lambda _experiment_id: tmp_path / "missing.json",
        )

        assert query_study("regular") is None

    def test_non_finite_metrics_do_not_break_json_persistence(self, monkeypatch, tmp_path):
        """Undefined engine metrics are omitted from the persisted candidate summary."""
        calls = []
        fake_run = _fake_run_factory(calls)

        def run_with_undefined_metric(
            experiment,
            *,
            verbose=True,
            progress_callback=None,
        ):
            result = fake_run(
                experiment,
                verbose=verbose,
                progress_callback=progress_callback,
            )
            result.strategies[0].metrics["undefined"] = float("nan")
            return result

        path = tmp_path / "study.json"
        monkeypatch.setattr("backtide.backtest.study.Experiment.run", run_with_undefined_metric)
        monkeypatch.setattr("backtide.backtest.study._result_path", lambda _experiment_id: path)

        result = Study(
            _config(),
            strategy=TunableStrategy(),
            parameter_space={"lookback": [10]},
        ).run(verbose=False)

        assert "undefined" not in result.candidates[0].metrics
        assert query_study(result.study_id) == result

    def test_unknown_schema_version_is_rejected(self):
        """Persisted study results reject unsupported schema versions."""
        with pytest.raises(ValueError, match="Unsupported study result schema version"):
            StudyResult.from_dict({"schema_version": 99})

    def test_non_object_sidecar_is_rejected(self, monkeypatch, tmp_path):
        """The public query requires a JSON object in a study sidecar."""
        path = tmp_path / "study.json"
        path.write_text("[]", encoding="utf-8")
        monkeypatch.setattr("backtide.backtest.study._result_path", lambda _study_id: path)

        with pytest.raises(ValueError, match="must contain a JSON object"):
            query_study("study")

    def test_result_path_rejects_traversal(self, monkeypatch, tmp_path):
        """Study sidecar paths cannot escape the experiment storage directory."""
        config = SimpleNamespace(data=SimpleNamespace(storage_path=tmp_path))
        monkeypatch.setattr("backtide.config.get_config", lambda: config)

        with pytest.raises(ValueError, match="Invalid experiment id"):
            study_module._result_path("../escape")

        assert (
            study_module._result_path("valid")
            == (tmp_path / "experiments" / "valid" / "study.json").resolve()
        )
