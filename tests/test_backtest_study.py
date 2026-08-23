"""Backtide.

Author: Mavs
Description: Tests for backtest parameter-sweep and walk-forward studies.

"""

from __future__ import annotations

from types import SimpleNamespace

import pytest

from backtide.backtest import ExperimentConfig
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
