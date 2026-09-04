"""Backtide.

Author: Mavs
Description: Unit tests for the backtest module.

"""

import pickle
import threading
from types import SimpleNamespace
from typing import Any, cast

import pandas as pd
import pytest

from backtide.backtest import (
    CommissionType,
    ConversionPeriod,
    CurrencyConversionMode,
    DataExpConfig,
    EmptyBarPolicy,
    EngineExpConfig,
    EquitySample,
    ExchangeExpConfig,
    Experiment,
    ExperimentConfig,
    ExperimentResult,
    ExperimentStatus,
    GeneralExpConfig,
    Order,
    OrderRecord,
    OrderStatus,
    OrderType,
    Portfolio,
    PortfolioExpConfig,
    RunResult,
    State,
    Trade,
)
import backtide.backtest.experiment as backtest_module
from backtide.indicators import SimpleMovingAverage
from backtide.strategies import BaseStrategy, BuyAndHold

# ─────────────────────────────────────────────────────────────────────────────
# Sub-configs
# ─────────────────────────────────────────────────────────────────────────────


class TestGeneralExpConfig:
    """Tests for the GeneralExpConfig model."""

    def test_defaults(self):
        """Test default values."""
        c = GeneralExpConfig()
        assert c.name == ""
        assert c.tags == []
        assert c.description == ""

    def test_custom(self):
        """Test custom values."""
        c = GeneralExpConfig(name="test", tags=["a"], description="desc")
        assert c.name == "test"

    def test_to_dict(self):
        """Test dict serialization."""
        d = GeneralExpConfig().to_dict()
        assert "name" in d
        assert "tags" in d

    def test_repr(self):
        """Test repr output."""
        assert "GeneralExpConfig" in repr(GeneralExpConfig())


class TestDataExpConfig:
    """Tests for the DataExpConfig model."""

    def test_defaults(self):
        """Test default values."""
        c = DataExpConfig()
        assert c.full_history is True
        assert c.symbols == []

    def test_custom(self):
        """Test custom values."""
        c = DataExpConfig(symbols=["AAPL"], full_history=False, start_date="2020-01-01")
        assert c.symbols == ["AAPL"]
        assert c.full_history is False
        assert c.start_date == "2020-01-01"

    def test_to_dict(self):
        """Test dict serialization."""
        d = DataExpConfig().to_dict()
        assert "instrument_type" in d
        assert "interval" in d


class TestPortfolioExpConfig:
    """Tests for the PortfolioExpConfig model."""

    def test_defaults(self):
        """Test default values."""
        c = PortfolioExpConfig()
        assert c.initial_cash == 10000

    def test_custom(self):
        """Test custom values."""
        c = PortfolioExpConfig(initial_cash=50000, base_currency="EUR")
        assert c.initial_cash == 50000


class TestExchangeExpConfig:
    """Tests for the ExchangeExpConfig model."""

    def test_defaults(self):
        """Test default values."""
        c = ExchangeExpConfig()
        assert isinstance(c.to_dict(), dict)

    def test_commission_type_accepts_enum_and_string_inputs(self):
        """Commission modes extract both enum instances and validated strings."""
        assert (
            ExchangeExpConfig(commission_type=CommissionType.Fixed).commission_type
            == CommissionType.Fixed
        )
        assert (
            ExchangeExpConfig(commission_type="percentage").commission_type
            == CommissionType.Percentage
        )
        with pytest.raises(ValueError, match="Unknown commission type"):
            ExchangeExpConfig(commission_type="invalid")

    def test_margin_defaults(self):
        """Margin / leverage / short-selling defaults are sensible."""
        c = ExchangeExpConfig()
        assert c.allow_margin is False
        assert c.max_leverage == 2.0
        assert c.initial_margin == 50.0
        assert c.maintenance_margin == 25.0
        assert c.margin_interest == 0.0
        assert c.allow_short_selling is False
        assert c.borrow_rate == 0.0
        assert c.raise_on_short_violation is False
        assert c.max_position_size == 100
        assert c.raise_on_margin_limit is False

    def test_raise_on_margin_limit_setter(self):
        """The new `raise_on_margin_limit` field is configurable."""
        c = ExchangeExpConfig(raise_on_margin_limit=True)
        assert c.raise_on_margin_limit is True
        c.raise_on_margin_limit = False
        assert c.raise_on_margin_limit is False

    def test_raise_on_short_violation_setter(self):
        """The `raise_on_short_violation` field is configurable."""
        c = ExchangeExpConfig(raise_on_short_violation=True)
        assert c.raise_on_short_violation is True
        c.raise_on_short_violation = False
        assert c.raise_on_short_violation is False

    def test_currency_conversion_settings(self):
        """Conversion mode / threshold / period / interval are configurable."""
        c = ExchangeExpConfig(
            conversion_mode=CurrencyConversionMode.HoldUntilThreshold,
            conversion_threshold=100.0,
            conversion_period=ConversionPeriod.Week,
            conversion_interval=5,
        )
        assert c.conversion_mode == CurrencyConversionMode.HoldUntilThreshold
        assert c.conversion_threshold == 100.0
        assert c.conversion_period == ConversionPeriod.Week
        assert c.conversion_interval == 5

    def test_roundtrip_includes_raise_on_margin_limit(self):
        """`raise_on_margin_limit` survives a TOML round-trip."""
        ec = ExperimentConfig(
            exchange=ExchangeExpConfig(raise_on_margin_limit=True, max_leverage=2.0),
        )
        ec2 = ExperimentConfig.from_toml(ec.to_toml())
        assert ec2.exchange.raise_on_margin_limit is True
        assert ec2.exchange.max_leverage == 2.0


class TestEngineExpConfig:
    """Tests for the EngineExpConfig model."""

    def test_defaults(self):
        """Test default values."""
        c = EngineExpConfig()
        assert c.warmup_period == 0
        assert c.trade_on_close is False

    def test_repr(self):
        """Test repr output."""
        assert "EngineExpConfig" in repr(EngineExpConfig())

    def test_random_seed_removed(self):
        """`random_seed` was removed from the engine config."""
        assert not hasattr(EngineExpConfig(), "random_seed")


# ─────────────────────────────────────────────────────────────────────────────
# ExperimentConfig
# ─────────────────────────────────────────────────────────────────────────────


class TestExperimentConfig:
    """Tests for the ExperimentConfig model."""

    def test_defaults(self):
        """Test default values."""
        ec = ExperimentConfig()
        assert ec.general.name == ""
        assert ec.data.symbols == []

    def test_to_dict(self):
        """Test dict serialization."""
        d = ExperimentConfig().to_dict()
        assert "general" in d
        assert "data" in d
        assert "portfolio" in d
        assert "engine" in d

    def test_to_toml_from_toml_roundtrip(self):
        """Test TOML round-trip serialization."""
        ec = ExperimentConfig(
            general=GeneralExpConfig(name="roundtrip"),
            data=DataExpConfig(symbols=["AAPL"]),
        )
        toml_str = ec.to_toml()
        ec2 = ExperimentConfig.from_toml(toml_str)
        assert toml_str.index("[indicators]") < toml_str.index("[metrics]")
        assert toml_str.index("[metrics]") < toml_str.index("[exchange]")
        assert "[metrics]\nselected = [" in toml_str
        assert ec2.general.name == "roundtrip"
        assert ec2.data.symbols == ["AAPL"]
        assert ec2.metrics == ec.metrics

    def test_from_toml_accepts_legacy_root_level_metrics(self):
        """Test compatibility with configurations saved before the metrics section."""
        config = ExperimentConfig.from_toml(
            'metrics = ["total_return"]\n\n[general]\nname = "legacy"\n'
        )

        assert config.general.name == "legacy"
        assert config.metrics == ["total_return"]

    def test_to_dict_from_dict_roundtrip(self):
        """Test dict round-trip serialization."""
        ec = ExperimentConfig(general=GeneralExpConfig(name="test"))
        d = ec.to_dict()
        ec2 = ExperimentConfig.from_dict(d)
        assert ec2.general.name == "test"

    def test_equality(self):
        """Test equality comparison."""
        assert ExperimentConfig() == ExperimentConfig()
        assert ExperimentConfig(general=GeneralExpConfig(name="a")) != ExperimentConfig()
        assert (ExperimentConfig() < ExperimentConfig()) is False

    def test_repr(self):
        """Test repr output."""
        assert "ExperimentConfig" in repr(ExperimentConfig())


# ─────────────────────────────────────────────────────────────────────────────
# Backtest enums
# ─────────────────────────────────────────────────────────────────────────────


@pytest.mark.parametrize(
    ("cls", "valid_str"),
    [
        (CommissionType, "Percentage"),
        (CommissionType, "Fixed"),
        (CommissionType, "PercentagePlusFixed"),
        (OrderType, "Market"),
    ],
)
def test_enum_from_string(cls, valid_str):
    """Backtest enums can be constructed from valid string."""
    obj = cls(valid_str)
    assert obj is not None


def test_enum_class_attrs():
    """Backtest enums that are Rust enums can be accessed via class attributes."""
    assert ConversionPeriod.Day is not None
    assert CurrencyConversionMode.Immediate is not None
    assert EmptyBarPolicy.Skip is not None


@pytest.mark.parametrize(
    "cls",
    [
        CommissionType,
        ConversionPeriod,
        CurrencyConversionMode,
        EmptyBarPolicy,
        OrderType,
    ],
)
def test_enum_variants(cls):
    """All backtest enums have non-empty variants()."""
    assert len(cls.variants()) > 0


@pytest.mark.parametrize(
    "cls",
    [CommissionType, EmptyBarPolicy, OrderType],
)
def test_enum_get_default(cls):
    """Backtest enums with get_default return a value."""
    assert cls.get_default() is not None


@pytest.mark.parametrize(
    "cls",
    [
        CommissionType,
        ConversionPeriod,
        CurrencyConversionMode,
        EmptyBarPolicy,
        ExperimentStatus,
        OrderStatus,
        OrderType,
    ],
)
def test_every_enum_variant_supports_its_public_contract(cls: Any) -> None:
    """Every variant can be inspected and round-tripped through pickle."""
    variants = cls.variants()

    for variant in variants:
        assert pickle.loads(pickle.dumps(variant)) == variant
        assert repr(variant)
        assert str(variant)
        assert hash(variant) == hash(variant)
        if hasattr(variant, "name"):
            assert variant.name
        if hasattr(variant, "description"):
            assert variant.description()


class TestCommissionType:
    """Tests for the CommissionType enum."""

    def test_str(self):
        """Test string representation."""
        assert "Percentage" in str(CommissionType("Percentage"))

    def test_invalid_raises(self):
        """Test invalid value raises ValueError."""
        with pytest.raises(ValueError, match="Unknown commission type"):
            CommissionType("invalid")


# ─────────────────────────────────────────────────────────────────────────────
# Order / OrderType
# ─────────────────────────────────────────────────────────────────────────────


class TestOrder:
    """Tests for the Order model."""

    def test_order_type_defaults_to_market(self):
        """Omitting order_type creates a market order."""
        order = Order(symbol="AAPL", quantity=10)
        assert order.order_type == OrderType.Market

    def test_default_id_is_generated(self):
        """A fresh Order receives an auto-generated id."""
        o1 = Order(symbol="AAPL", order_type="market", quantity=10)
        o2 = Order(symbol="AAPL", order_type="market", quantity=10)
        assert isinstance(o1.id, str)
        assert len(o1.id) > 0
        assert o1.id != o2.id  # uuid uniqueness

    def test_explicit_id_is_kept(self):
        """An explicit id is preserved (used by Cancel)."""
        uid = "a1b2c3d4e5f6a7b8a1b2c3d4e5f6a7b8"
        o = Order(symbol="AAPL", order_type="market", quantity=1, id=uid)
        assert o.id == uid

    def test_cancel_order_can_have_empty_symbol(self):
        """Cancel only needs the target id, not a symbol."""
        uid = "00112233445566778899aabbccddeeff"
        cancel = Order(
            symbol="",
            order_type="cancel",
            quantity=0,
            id=uid,
        )
        assert cancel.order_type == OrderType.Cancel
        assert cancel.id == uid

    def test_repr_contains_id(self):
        """Order repr always includes the id field."""
        uid = "aabbccdd11223344aabbccdd11223344"
        o = Order(symbol="AAPL", order_type="market", quantity=1, id=uid)
        assert uid in repr(o)

    @pytest.mark.parametrize(
        ("price", "limit_price", "expected"),
        [(None, None, "type=Market"), (100.0, None, "price=100"), (100.0, 99.0, "limit=99")],
    )
    def test_repr_and_pickle_cover_each_price_shape(
        self,
        price: float | None,
        limit_price: float | None,
        expected: str,
    ) -> None:
        """Order representations and pickle preserve every optional price shape."""
        order = Order("AAPL", 2.5, price=price, limit_price=limit_price)

        restored = pickle.loads(pickle.dumps(order))

        assert expected in repr(order)
        assert restored == order

    def test_invalid_quantity_and_identifier_are_rejected(self) -> None:
        """Order construction rejects unrelated quantities and malformed identifiers."""
        with pytest.raises(TypeError, match="quantity must be"):
            Order("AAPL", object())
        with pytest.raises(TypeError, match="invalid order id"):
            Order("AAPL", id="invalid")


class TestPortfolioAndState:
    """Tests for strategy-facing backtest snapshots."""

    def test_portfolio_and_state_expose_values_and_representations(self) -> None:
        """Snapshot constructors preserve values and provide useful representations."""
        order = Order("AAPL", 1.0)
        portfolio = Portfolio(cash={"USD": 100.0}, positions={"AAPL": 2.0}, orders=[order])
        state = State(timestamp=1_700_000_000, bar_index=2, total_bars=10, is_warmup=True)

        assert portfolio.cash
        assert portfolio.positions == {"AAPL": 2.0}
        assert portfolio.orders == [order]
        assert "AAPL" in repr(portfolio)
        assert state.datetime.tzinfo is not None
        assert state.bar_index == 2
        assert state.total_bars == 10
        assert state.is_warmup is True
        assert "bar_index=2" in repr(state)


class TestOrderType:
    """Tests for the OrderType enum."""

    def test_cancel_order_variant_exists(self):
        """The new Cancel variant is available."""
        assert OrderType.Cancel is not None
        assert "Cancel" in OrderType.Cancel.name

    def test_cancel_order_in_variants(self):
        """Cancel appears in the variants list."""
        assert any(v == OrderType.Cancel for v in OrderType.variants())

    def test_cancel_order_description(self):
        """Cancel has a non-empty description."""
        assert "cancel" in OrderType.Cancel.description().lower()


# ─────────────────────────────────────────────────────────────────────────────
# Result models
# ─────────────────────────────────────────────────────────────────────────────


class TestResultModels:
    """Tests for the experiment result pyclasses."""

    def test_classes_importable(self):
        """All result classes are importable from backtide.backtest."""
        assert EquitySample is not None
        assert Trade is not None
        assert OrderRecord is not None
        assert RunResult is not None
        assert ExperimentResult is not None

    def test_headline_types_are_available_from_all_public_paths(self):
        """Experiments and studies are importable from Backtide and its backtest package."""
        from backtide import Experiment as TopLevelExperiment
        from backtide import Study as TopLevelStudy
        from backtide.backtest.experiment import Experiment as ModuleExperiment
        from backtide.backtest.study import Study as ModuleStudy

        assert TopLevelExperiment is Experiment is ModuleExperiment
        assert TopLevelStudy is ModuleStudy

    def test_completed_result_models_expose_serializable_state(self) -> None:
        """A deterministic round trip exercises every persisted result model."""

        class RoundTripStrategy(BaseStrategy):
            """Open one share and close it on the following strategy tick."""

            def evaluate(self, data, portfolio, state, indicators):
                del data, state, indicators
                quantity = portfolio.positions.get("AAPL", 0.0)
                if quantity > 0.0:
                    return [Order("AAPL", -quantity)]
                if not portfolio.orders:
                    return [Order("AAPL", 1.0)]
                return []

        result = Experiment(
            _fixture_config(name="result-model-round-trip"),
            strategies=RoundTripStrategy(),
        ).run(verbose=False)
        run = result.strategies[0]

        values = [result, run, run.equity_curve[0], run.orders[0], run.trades[0]]
        for value in values:
            state = value.__getstate__()
            value.__setstate__(state)

            assert pickle.dumps(value)
            assert repr(value)


# ─────────────────────────────────────────────────────────────────────────────
# Experiment
# ─────────────────────────────────────────────────────────────────────────────


def _fixture_config(
    *,
    name: str = "",
    symbols: list[str] | None = None,
    metrics: list[Any] | None = None,
) -> ExperimentConfig:
    """Return an offline experiment configuration backed by the test fixture."""
    return ExperimentConfig(
        general=GeneralExpConfig(name=name),
        data=DataExpConfig(
            symbols=["AAPL"] if symbols is None else symbols,
            instrument_type="stocks",
            interval="1d",
            full_history=False,
            start_date="2024-01-01",
            end_date="2024-03-01",
        ),
        metrics=metrics
        or [
            "sharpe",
            "total_return",
            "pnl",
            "max_dd",
            "cagr",
            "win_rate",
            "profit_factor",
            "final_equity",
        ],
    )


class TestExperiment:
    """Tests for the class-based experiment interface."""

    def test_old_function_is_not_public(self):
        """The removed function API has no compatibility adapter."""
        assert not hasattr(backtest_module, "run_experiment")

    def test_no_symbols_raises(self):
        """An experiment with no symbols cannot run."""
        config = _fixture_config(symbols=[])
        with pytest.raises(ValueError, match="no symbols"):
            Experiment(config, strategies=[BuyAndHold()]).run(verbose=False)

    def test_no_strategies_raises(self):
        """An experiment requires at least one strategy."""
        with pytest.raises(ValueError, match="no strategies"):
            Experiment(_fixture_config(), strategies=[]).run(verbose=False)

    def test_runtime_dependencies_follow_config(self, monkeypatch):
        """Strategies and indicators are runtime dependencies while metrics live in config."""
        captured: dict[str, Any] = {}

        def fake_run(config, verbose, strategies, indicators):
            captured.update(
                config=config,
                verbose=verbose,
                strategies=strategies,
                indicators=indicators,
            )
            return object()

        monkeypatch.setattr(backtest_module, "_run_experiment", fake_run)
        strategy = BuyAndHold()
        indicator = SimpleMovingAverage(20)
        config = _fixture_config(metrics=["sharpe"])

        result = Experiment(
            config,
            strategies={"Runtime strategy": strategy},
            indicators={"Runtime indicator": indicator},
        ).run(verbose=False)

        assert result is not None
        assert captured["config"].strategy.strategies == ["Runtime strategy"]
        assert captured["config"].indicators.indicators == ["Runtime indicator"]
        assert captured["config"].metrics == ["sharpe"]
        assert captured["strategies"] == {"Runtime strategy": strategy}
        assert captured["indicators"] == {"Runtime indicator": indicator}
        assert captured["verbose"] is False

    def test_progress_callback_is_forwarded(self, monkeypatch):
        """A progress callback reaches the low-level simulation engine."""
        captured = None

        def fake_run(_config, _verbose, _strategies, _indicators, progress_callback):
            nonlocal captured
            captured = progress_callback
            progress_callback(4, 10)
            return object()

        updates = []
        monkeypatch.setattr(backtest_module, "_run_experiment", fake_run)

        Experiment(_fixture_config(), strategies=[BuyAndHold()]).run(
            verbose=False,
            progress_callback=lambda completed, total: updates.append((completed, total)),
        )

        assert captured is not None
        assert updates == [(4, 10)]

    def test_blank_name_is_generated(self, monkeypatch):
        """A blank name is replaced before dispatch."""

        class Capture(RuntimeError):
            """Capture the normalized configuration."""

            def __init__(self, config):
                super().__init__("captured")
                self.config = config

        class FakeUuid:
            """Return a deterministic UUID string."""

            def __str__(self) -> str:
                return "01234567-89ab-cdef-0123-456789abcdef"

        monkeypatch.setattr(backtest_module.uuid, "uuid4", FakeUuid)

        def fake_run(config, _verbose, _strategies, _indicators):
            raise Capture(config)

        monkeypatch.setattr(backtest_module, "_run_experiment", fake_run)
        with pytest.raises(Capture) as captured:
            Experiment(_fixture_config(), strategies=[BuyAndHold()]).run(verbose=False)

        assert captured.value.config.general.name == "01234567"

    def test_explicit_name_is_preserved(self, monkeypatch):
        """A non-empty experiment name is unchanged."""

        def fake_run(config, _verbose, _strategies, _indicators):
            assert config.general.name == "named"
            return object()

        monkeypatch.setattr(backtest_module, "_run_experiment", fake_run)
        Experiment(_fixture_config(name="named"), strategies=[BuyAndHold()]).run(verbose=False)

    def test_keyboard_interrupt_cleans_up(self, monkeypatch):
        """An interrupted experiment is translated and cleaned up."""
        cleaned: list[tuple[str | None, str]] = []

        def fake_run(*_args):
            raise KeyboardInterrupt

        monkeypatch.setattr(backtest_module, "_run_experiment", fake_run)
        monkeypatch.setattr(
            backtest_module,
            "_cleanup_experiment",
            lambda experiment_id, name: cleaned.append((experiment_id, name)),
        )

        with pytest.raises(backtest_module.ExperimentAborted):
            Experiment(_fixture_config(name="interrupt"), strategies=[BuyAndHold()]).run(
                verbose=False
            )

        assert cleaned == [(None, "interrupt")]

    def test_runtime_parameter_resolves_stored_names(self):
        """Stored runtime dependencies retain their configured names without overrides."""
        assert Experiment._resolve_runtime_param("Saved") == (["Saved"], {})

    def test_external_abort_cleans_up_completed_result(self, monkeypatch):
        """An abort requested during execution removes the newly persisted result."""
        cleaned = []
        abort_event = threading.Event()
        abort_event.set()
        monkeypatch.setattr(backtest_module, "_abort_event", abort_event)
        monkeypatch.setattr(
            backtest_module,
            "_run_experiment",
            lambda *_args: SimpleNamespace(experiment_id="exp-aborted"),
        )
        monkeypatch.setattr(
            backtest_module,
            "_cleanup_experiment",
            lambda experiment_id, name: cleaned.append((experiment_id, name)),
        )

        with pytest.raises(backtest_module.ExperimentAborted):
            Experiment(_fixture_config(name="abort"), strategies=[BuyAndHold()]).run(verbose=False)

        assert cleaned == [("exp-aborted", "abort")]


class TestExperimentPolymorphicForms:
    """Tests for runtime strategy and indicator forms."""

    @pytest.mark.parametrize(
        "strategies",
        [
            BuyAndHold(),
            [BuyAndHold()],
            {"Named": BuyAndHold()},
            [BuyAndHold(), {"Other": BuyAndHold(symbol="AAPL")}],
        ],
    )
    def test_strategy_forms_run(self, strategies):
        """A supported strategy form completes an experiment."""
        result = Experiment(_fixture_config(), strategies=strategies).run(verbose=False)
        assert result.status == ExperimentStatus.Success

    def test_indicator_instance_runs(self):
        """A custom indicator instance is computed."""
        result = Experiment(
            _fixture_config(),
            strategies=[BuyAndHold()],
            indicators=[SimpleMovingAverage(20)],
        ).run(verbose=False)
        assert result.status == ExperimentStatus.Success


class TestAutoIndicatorLookup:
    """Built-in strategies find indicators injected by the engine."""

    def test_multi_bollinger_rotation_produces_orders(self):
        """MultiBollingerRotation trades with its Bollinger Bands."""
        from backtide.strategies import MultiBollingerRotation

        result = Experiment(
            _fixture_config(),
            strategies=MultiBollingerRotation(period=5, rebalance_interval=1),
        ).run(verbose=False)
        assert result.status == ExperimentStatus.Success
        assert result.strategies[0].orders

    def test_triple_rsi_rotation_produces_orders(self):
        """TripleRsiRotation trades with its RSI indicators."""
        from backtide.strategies import TripleRsiRotation

        strategy = TripleRsiRotation(
            short_period=2,
            medium_period=3,
            long_period=5,
            rebalance_interval=1,
        )
        result = Experiment(_fixture_config(), strategies=strategy).run(verbose=False)
        assert result.status == ExperimentStatus.Success
        assert result.strategies[0].orders

    def test_bollinger_mean_reversion_produces_orders(self):
        """BollingerMeanReversion uses its injected indicator."""
        from backtide.strategies import BollingerMeanReversion

        result = Experiment(
            _fixture_config(),
            strategies=BollingerMeanReversion(period=3, std_dev=1.0),
        ).run(verbose=False)
        assert result.status == ExperimentStatus.Success
        assert result.strategies[0].orders


class TestBaseStrategyLog:
    """Tests for BaseStrategy.log()."""

    @pytest.mark.parametrize("level", ["error", "warn", "debug"])
    def test_custom_level(self, level):
        """A custom log level is forwarded."""
        from unittest.mock import patch

        with patch("backtide.backtest.experiment_log") as mock_log:
            BaseStrategy.log("message", level=level)
            mock_log.assert_called_once_with("message", level)

    def test_default_level(self):
        """The default log level is info."""
        from unittest.mock import patch

        with patch("backtide.backtest.experiment_log") as mock_log:
            BaseStrategy.log("message")
            mock_log.assert_called_once_with("message", "info")


class TestCleanupExperiment:
    """Tests for best-effort experiment cleanup."""

    def test_cleanup_with_experiment_id_calls_delete(self):
        """A known experiment id is deleted directly."""
        from unittest.mock import patch

        with patch("backtide.backtest.experiment._delete_experiment") as delete:
            backtest_module._cleanup_experiment("exp-123", "name")
            delete.assert_called_once_with("exp-123")

    def test_cleanup_without_id_uses_latest_name_match(self):
        """A partial experiment is found by name."""
        from unittest.mock import patch

        with (
            patch("backtide.backtest.experiment._query_experiments") as query,
            patch(
                "backtide.backtest.experiment._to_pandas",
                return_value=pd.DataFrame({"id": ["exp-456"]}),
            ),
            patch("backtide.backtest.experiment._delete_experiment") as delete,
        ):
            backtest_module._cleanup_experiment(None, "name")
            query.assert_called_once_with(search="name", limit=1)
            delete.assert_called_once_with("exp-456")

    def test_cleanup_failures_are_ignored(self):
        """Cleanup remains best effort."""
        from unittest.mock import patch

        with patch(
            "backtide.backtest.experiment._query_experiments",
            side_effect=RuntimeError("failed"),
        ):
            backtest_module._cleanup_experiment(None, "name")

    def test_known_experiment_delete_failures_are_ignored(self):
        """Best-effort cleanup suppresses deletion failures for a known identifier."""
        from unittest.mock import patch

        with patch(
            "backtide.backtest.experiment._delete_experiment",
            side_effect=RuntimeError("failed"),
        ):
            backtest_module._cleanup_experiment("exp-123", "name")


class TestStoredExperimentAccess:
    """Tests for persisted experiment access."""

    def test_query_experiment_after_run(self):
        """A completed experiment can be queried by name."""
        from backtide.storage import query_experiments, query_strategy_runs

        result = Experiment(
            _fixture_config(name="test-stored-exp"),
            strategies=[BuyAndHold()],
        ).run(verbose=False)
        experiments = cast(
            pd.DataFrame,
            query_experiments(search="test-stored-exp", limit=1),
        )

        assert result.experiment_id
        assert not experiments.empty
        assert query_strategy_runs(experiments.iloc[0]["id"]) is not None
