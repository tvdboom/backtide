"""Backtide.

Author: Mavs
Description: Tests for built-in and custom experiment metrics.

"""

from pathlib import Path
from typing import Any

import pytest

from backtide.backtest import DataExpConfig, Experiment, ExperimentConfig
from backtide.live import MarketUpdate, Session, SessionConfig
from backtide.metrics import BaseMetric, list_builtin_metrics
from backtide.metrics.utils import (
    _build_custom_metric,
    _check_metric_code,
    _load_stored_metrics,
    _metric_greater_is_better,
    _save_metric,
)
from backtide.strategies import BuyAndHold
from backtide.ui.services import APIError, BacktideServices


class TestMetricCatalog:
    """Tests for built-in metric metadata and experiment defaults."""

    def test_default_metrics_begin_with_sharpe(self):
        """Default metric configuration places Sharpe first."""
        config = ExperimentConfig()

        assert config.metrics == [
            "sharpe",
            "total_return",
            "pnl",
            "max_dd",
            "cagr",
            "n_trades",
            "win_rate",
            "sortino",
            "ann_volatility",
            "final_equity",
            "excess_return",
            "alpha",
        ]

    def test_catalog_exposes_extended_rust_metrics(self):
        """The Rust catalog includes the complete performance metric set."""
        keys = {metric.key for metric in list_builtin_metrics()}

        assert {"sharpe", "sortino", "cagr", "max_dd", "profit_factor", "calmar"} <= keys

    def test_documentation_lists_every_builtin_metric_key(self):
        """The metrics guide exposes every exact built-in string key."""
        documentation = (
            Path(__file__).resolve().parents[1]
            / "docs_sources"
            / "user_guide"
            / "library"
            / "metrics.md"
        ).read_text(
            encoding="utf-8",
        )

        for metric in list_builtin_metrics():
            assert f"| `{metric.key}` | {metric.name} |" in documentation

    def test_primary_metric_summary_respects_lower_is_better(self, monkeypatch):
        """Experiment summaries rank the configured metric in its declared direction."""
        services = BacktideServices()
        monkeypatch.setattr(
            services,
            "metric_catalog",
            lambda: {
                "builtin": [],
                "saved": [
                    {
                        "key": "risk_score",
                        "name": "Risk score",
                        "percentage": False,
                        "greater_is_better": False,
                    }
                ],
            },
        )

        summary = services._primary_metric_summary(
            'metrics = ["risk_score"]',
            [
                {"metrics": {"risk_score": 4.0}, "is_benchmark": False},
                {"metrics": {"risk_score": 2.0}, "is_benchmark": False},
            ],
        )

        assert summary["primary_metric_name"] == "Risk score"
        assert summary["primary_metric_value"] == 2.0

    def test_builtin_keys_are_reserved_for_rust_metrics(self):
        """A custom metric cannot shadow a built-in result key."""
        with pytest.raises(APIError, match="reserved"):
            BacktideServices().save_metric({"name": "sharpe", "code": ""})


class TestCustomMetric:
    """Tests for custom metric validation and persistence."""

    code = """from backtide.metrics import BaseMetric

class AveragePnl(BaseMetric):
    '''Return the average realized trade PnL.'''

    percentage = False
    greater_is_better = True

    def compute(self, equity_curve, trades):
        return float(trades["pnl"].mean()) if len(trades) else 0.0

AveragePnl()
"""

    class ConstantMetric(BaseMetric):
        """Return a deterministic value for interface tests."""

        def compute(self, equity_curve, trades) -> float:
            """Return a fixed scalar."""
            del equity_curve, trades
            return 7.0

    def test_greater_is_better_defaults_to_true(self):
        """Custom metrics rank larger values first when no direction is declared."""
        assert _metric_greater_is_better(self.ConstantMetric()) is True

    def test_greater_is_better_can_rank_smaller_values_first(self):
        """A custom metric can explicitly rank smaller values first."""

        class RiskMetric(self.ConstantMetric):
            greater_is_better = False

        assert _metric_greater_is_better(RiskMetric()) is False

    def test_one_experiment_config_list_holds_builtin_and_custom_metrics(self):
        """Experiment metrics are specified only on ExperimentConfig."""
        config = ExperimentConfig(
            data=DataExpConfig(
                symbols=["AAPL"],
                instrument_type="stocks",
                interval="1d",
                full_history=False,
                start_date="2024-01-01",
                end_date="2024-03-01",
            ),
            metrics=["sharpe", {"constant_score": self.ConstantMetric()}],
        )

        result = Experiment(config, strategies=[BuyAndHold()]).run(verbose=False)

        assert config.to_dict()["metrics"] == ["sharpe", "constant_score"]
        assert result.strategies[0].metrics["constant_score"] == 7.0

    def test_one_session_config_list_holds_builtin_and_custom_metrics(self):
        """Session metrics are specified only on SessionConfig."""
        config = SessionConfig(metrics=["pnl", {"constant_score": self.ConstantMetric()}])
        session = Session(config)
        market = MarketUpdate(
            "BTC-USD",
            "1m",
            1_700_000_000,
            1_700_000_060,
            100.0,
            102.0,
            99.0,
            101.0,
        )

        snapshot = session.on_bar(market).snapshot

        assert config.metrics[0] == "pnl"
        custom_metric = config.metrics[1]
        assert isinstance(custom_metric, dict)
        assert list(custom_metric) == ["constant_score"]
        assert snapshot.metrics["constant_score"] == 7.0

    def test_build_and_validate_metric(self):
        """A valid final instance and compute signature are accepted."""
        metric = _build_custom_metric(self.code)

        assert isinstance(metric, BaseMetric)
        assert _check_metric_code(self.code) is None
        assert BacktideServices._custom_metric_description(metric) == (
            "Return the average realized trade PnL."
        )

    def test_requires_a_class_docstring(self):
        """Validation requires the description to live in the class docstring."""
        code = self.code.replace("    '''Return the average realized trade PnL.'''\n\n", "")

        assert _check_metric_code(code) == (
            "Metric class must define a docstring used as its description."
        )

    def test_rejects_non_finite_metric(self):
        """Validation rejects custom metrics that return a non-finite scalar."""
        code = self.code.replace(
            'return float(trades["pnl"].mean()) if len(trades) else 0.0',
            'return float("nan")',
        )

        assert _check_metric_code(code) == "Metric `compute` must return a finite float."

    @pytest.mark.parametrize(
        ("replacement", "message"),
        [
            (
                "def compute(self, equity_curve):\n        return 1.0",
                "must have signature",
            ),
            (
                (
                    "percentage = 'yes'\n    greater_is_better = True\n\n    "
                    "def compute(self, equity_curve, trades):\n        return 1.0"
                ),
                "percentage` must be a bool",
            ),
            (
                (
                    "percentage = False\n    greater_is_better = 'yes'\n\n    "
                    "def compute(self, equity_curve, trades):\n        return 1.0"
                ),
                "greater_is_better` must be a bool",
            ),
            (
                "def compute(self, equity_curve, trades):\n        raise LookupError('missing')",
                "LookupError: missing",
            ),
        ],
    )
    def test_validation_reports_metric_contract_errors(
        self,
        replacement: str,
        message: str,
    ) -> None:
        """Metric validation reports signature, metadata, and execution failures."""
        start = self.code.index("    percentage = False")
        end = self.code.index("\n\nAveragePnl()")
        code = f"{self.code[:start]}    {replacement}{self.code[end:]}"

        assert message in str(_check_metric_code(code))

    def test_validation_reports_instantiation_errors(self) -> None:
        """Metric validation translates a final-expression type mismatch."""
        assert "Failed to instantiate metric" in str(_check_metric_code("object()"))

    def test_legacy_metric_direction_is_supported(self) -> None:
        """The legacy ranking attribute remains supported on custom subclasses."""

        class LegacyMetric(self.ConstantMetric):
            higher_is_better = False

        assert _metric_greater_is_better(LegacyMetric()) is False

    def test_invalid_metric_direction_raises(self) -> None:
        """A non-boolean ranking direction raises a specific type error."""

        class InvalidMetric(self.ConstantMetric):
            greater_is_better = "yes"

        with pytest.raises(TypeError, match="must be a bool"):
            _metric_greater_is_better(InvalidMetric())

    def test_save_and_load_metric(self, tmp_path: Path):
        """Custom metric source and behavior survive local persistence."""
        config: Any = type(
            "Config", (), {"data": type("Data", (), {"storage_path": tmp_path})()}
        )()
        metric = _build_custom_metric(self.code)

        _save_metric(metric, "Average PnL", config)
        loaded = _load_stored_metrics(config)

        assert vars(loaded["Average PnL"])["_source_code"] == self.code
