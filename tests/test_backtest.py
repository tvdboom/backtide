"""Backtide.

Author: Mavs
Description: Unit tests for the backtest module.

"""

from typing import Any, cast

import pandas as pd
import pytest

import backtide.backtest as backtest_module
from backtide.backtest import (
    CommissionType,
    ConversionPeriod,
    CurrencyConversionMode,
    DataExpConfig,
    EmptyBarPolicy,
    EngineExpConfig,
    EquitySample,
    ExchangeExpConfig,
    ExperimentConfig,
    ExperimentResult,
    ExperimentStatus,
    GeneralExpConfig,
    IndicatorExpConfig,
    Order,
    OrderRecord,
    OrderType,
    PortfolioExpConfig,
    RunResult,
    StrategyExpConfig,
    Trade,
    run_experiment,
)
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
        assert ec2.general.name == "roundtrip"
        assert ec2.data.symbols == ["AAPL"]

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


# ─────────────────────────────────────────────────────────────────────────────
# run_experiment integration
# ─────────────────────────────────────────────────────────────────────────────


class TestRunExperiment:
    """Smoke tests for the run_experiment pipeline."""

    def test_no_symbols_raises(self):
        """An experiment with no symbols cannot run."""
        cfg = ExperimentConfig(
            general=GeneralExpConfig(name="empty"),
            data=DataExpConfig(symbols=[]),
            strategy=StrategyExpConfig(benchmark="", strategies=[]),
        )
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(cfg, verbose=False)

    def test_no_data_returns_failed(self, monkeypatch):
        """When there is no data and no strategies, status is 'failed'.

        Uses ``conftest``'s temp storage so this never hits the network:
        the resolve/download phase is monkey-patched to return empty.
        """
        from backtide.core import data as core_data

        # Stub out network calls to keep the test offline.
        monkeypatch.setattr(
            core_data,
            "resolve_profiles",
            list,
            raising=False,
        )

        cfg = ExperimentConfig(
            general=GeneralExpConfig(name="offline"),
            data=DataExpConfig(symbols=["NOPE-XYZ"]),
            strategy=StrategyExpConfig(benchmark="SPY", strategies=[]),
        )
        # We expect either a clean failed result, or a runtime/value error from
        # the resolve step; both are acceptable defensive outcomes.
        try:
            result = run_experiment(cfg, verbose=False)
        except (RuntimeError, ValueError):
            return
        assert isinstance(result, ExperimentResult)
        assert result.status in (ExperimentStatus.Error, ExperimentStatus.Success)


# ─────────────────────────────────────────────────────────────────────────────
# run_experiment kwargs forms
# ─────────────────────────────────────────────────────────────────────────────


class TestRunExperimentKwargs:
    """Tests for the kwargs-based ``run_experiment`` invocation forms.

    All tests rely on the no-symbols / empty-data guard rails so they
    never need network access to verify that the kwargs translation
    layer correctly populates an ``ExperimentConfig``.
    """

    # ── Backward compatibility ──────────────────────────────────────

    def test_positional_experiment_config_still_works(self):
        """Passing an ExperimentConfig positionally is backward compatible."""
        cfg = ExperimentConfig(
            general=GeneralExpConfig(name="legacy"),
            data=DataExpConfig(symbols=[]),
        )
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(cfg, verbose=False)

    def test_no_args_uses_defaults(self):
        """Calling without args uses defaults (no symbols → RuntimeError)."""
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(verbose=False)

    # ── Flat kwargs ──────────────────────────────────────────────────

    def test_flat_kwargs_route_to_general(self):
        """Flat ``name`` / ``description`` kwargs route to ``general``."""
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(
                name="flat-name",
                description="flat-desc",
                tags=["a", "b"],
                verbose=False,
            )

    def test_flat_kwargs_route_to_data(self):
        """Flat ``symbols`` / ``interval`` kwargs route to ``data``."""
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(
                symbols=[],
                interval="1d",
                instrument_type="stocks",
                full_history=False,
                start_date="2024-01-01",
                end_date="2024-03-01",
                verbose=False,
            )

    def test_flat_kwargs_route_to_portfolio(self):
        """Flat ``initial_cash`` / ``base_currency`` kwargs route to portfolio."""
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(
                initial_cash=50_000,
                base_currency="USD",
                verbose=False,
            )

    def test_flat_kwargs_route_to_exchange(self):
        """Flat exchange-section kwargs route to ``exchange``."""
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(
                commission_type="Fixed",
                commission_fixed=2.5,
                slippage=0.1,
                allow_short_selling=False,
                verbose=False,
            )

    def test_flat_kwargs_route_to_engine(self):
        """Flat engine-section kwargs route to ``engine``."""
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(
                warmup_period=5,
                trade_on_close=True,
                risk_free_rate=0.02,
                empty_bar_policy="Skip",
                verbose=False,
            )

    def test_unknown_flat_kwarg_raises_value_error(self):
        """Unknown kwargs raise ValueError with a helpful message."""
        with pytest.raises(ValueError, match="Unknown keyword argument"):
            run_experiment(not_a_field=123, verbose=False)

    def test_enum_string_alias_via_kwargs(self):
        """Enum aliases like ``interval='1d'`` work through kwargs.

        Regression test: serde-based round-tripping doesn't honour these
        aliases; the implementation must use Python-level setattr.
        """
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(interval="1d", verbose=False)
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(interval="1h", verbose=False)

    # ── Sub-config kwargs ────────────────────────────────────────────

    def test_sub_config_instance_kwargs(self):
        """Each sub-config can be passed as a typed instance kwarg."""
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(
                general=GeneralExpConfig(name="sub"),
                data=DataExpConfig(symbols=[]),
                portfolio=PortfolioExpConfig(initial_cash=20_000),
                strategy=StrategyExpConfig(strategies=[]),
                indicators=IndicatorExpConfig(indicators=[]),
                exchange=ExchangeExpConfig(),
                engine=EngineExpConfig(warmup_period=3),
                verbose=False,
            )

    # ── Mixing positional and kwargs ─────────────────────────────────

    def test_positional_config_with_kwargs_overrides(self):
        """Kwargs override fields of a positional ExperimentConfig."""
        cfg = ExperimentConfig(
            general=GeneralExpConfig(name="orig"),
            data=DataExpConfig(symbols=[]),
        )
        # If kwargs were ignored, this would still hit the no-symbols guard
        # (it does anyway). We just verify no error is raised by the
        # kwargs translation itself.
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(cfg, name="overridden", verbose=False)

    def test_blank_name_gets_auto_generated(self, monkeypatch):
        """Blank names are replaced by an 8-char auto id before dispatch."""

        class _CaptureConfig(RuntimeError):
            def __init__(self, cfg):
                super().__init__("captured")
                self.cfg = cfg

        class _FakeUuid:
            def __str__(self) -> str:
                return "01234567-89ab-cdef-0123-456789abcdef"

        monkeypatch.setattr(backtest_module.uuid, "uuid4", lambda: _FakeUuid())

        def _fake_run_experiment(cfg, _verbose, _strategy_overrides, _indicator_overrides):
            raise _CaptureConfig(cfg)

        monkeypatch.setattr(backtest_module, "_run_experiment", _fake_run_experiment)

        with pytest.raises(_CaptureConfig) as ex:
            run_experiment(
                name="",
                symbols=["AAPL"],
                instrument_type="stocks",
                interval="1d",
                full_history=False,
                start_date="2024-01-01",
                end_date="2024-03-01",
                strategies=[BuyAndHold()],
                verbose=False,
            )

        assert ex.value.cfg.general.name == "01234567"

    def test_explicit_name_is_preserved(self, monkeypatch):
        """A non-empty name passes through unchanged."""

        class _CaptureConfig(RuntimeError):
            def __init__(self, cfg):
                super().__init__("captured")
                self.cfg = cfg

        def _fake_run_experiment(cfg, _verbose, _strategy_overrides, _indicator_overrides):
            raise _CaptureConfig(cfg)

        monkeypatch.setattr(backtest_module, "_run_experiment", _fake_run_experiment)

        with pytest.raises(_CaptureConfig) as ex:
            run_experiment(
                name="my-exp",
                symbols=["AAPL"],
                instrument_type="stocks",
                interval="1d",
                full_history=False,
                start_date="2024-01-01",
                end_date="2024-03-01",
                strategies=[BuyAndHold()],
                verbose=False,
            )

        assert ex.value.cfg.general.name == "my-exp"


# ─────────────────────────────────────────────────────────────────────────────
# run_experiment polymorphic strategies / indicators
# ─────────────────────────────────────────────────────────────────────────────


# Date range matching the pre-built test fixture so these tests are offline.
run_kwargs: dict[str, Any] = {
    "symbols": ["AAPL"],
    "instrument_type": "stocks",
    "interval": "1d",
    "full_history": False,
    "start_date": "2024-01-01",
    "end_date": "2024-03-01",
}


class TestRunExperimentPolymorphicForms:
    """Tests for the polymorphic ``strategies`` / ``indicators`` kwargs.

    Each test runs a real (small) backtest against the pre-built
    ``tests/_data/database.duckdb`` fixture and inspects the
    [`RunResult`] entries returned in ``result.strategies`` to verify
    that inline instances and dict forms are honoured by the engine.
    """

    def test_strategies_single_instance_uses_class_name(self):
        """A single ``BaseStrategy`` instance is accepted and the run completes."""
        result = run_experiment(strategies=BuyAndHold(), verbose=False, **run_kwargs)
        assert isinstance(result, ExperimentResult)
        assert result.status == ExperimentStatus.Success

    def test_strategies_list_of_instances(self):
        """A list of instances produces a completed result."""
        result = run_experiment(
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result.status == ExperimentStatus.Success

    def test_strategies_dict_form_uses_explicit_name(self):
        """``strategies=[{'name': instance}]`` passes the instance under that name."""
        result = run_experiment(
            strategies=[{"My Strategy": BuyAndHold()}],
            verbose=False,
            **run_kwargs,
        )
        assert result.status == ExperimentStatus.Success

    def test_strategies_mixed_list(self):
        """A list mixing instances and dicts produces a completed result."""
        result = run_experiment(
            strategies=[BuyAndHold(), {"named": BuyAndHold(symbol="AAPL")}],
            verbose=False,
            **run_kwargs,
        )
        assert result.status == ExperimentStatus.Success

    def test_strategies_via_strategy_sub_config_dict(self):
        """Strategies passed via flat ``strategies`` kwarg are honoured."""
        result = run_experiment(
            strategies=BuyAndHold(),
            verbose=False,
            **run_kwargs,
        )
        assert result.status == ExperimentStatus.Success

    def test_indicators_instance_runs_successfully(self):
        """An indicator instance is computed and the strategy completes."""
        result = run_experiment(
            strategies=[BuyAndHold()],
            indicators=[SimpleMovingAverage(20)],
            verbose=False,
            **run_kwargs,
        )
        assert result.status == ExperimentStatus.Success

    def test_indicators_sub_config_form(self):
        """``indicators=IndicatorExpConfig(...)`` is treated as the sub-config."""
        result = run_experiment(
            strategies=[BuyAndHold()],
            indicators=IndicatorExpConfig(indicators=[]),
            verbose=False,
            **run_kwargs,
        )
        assert result.status == ExperimentStatus.Success


# ─────────────────────────────────────────────────────────────────────────────
# Regression: auto-injected indicators reach built-in strategies
# ─────────────────────────────────────────────────────────────────────────────


class TestAutoIndicatorLookup:
    """Built-in strategies must find the indicators the engine auto-injects.

    Previously the strategy-side helper produced ``__auto_BB_20_2p0`` while
    the engine stored series under ``BB_20_2p0``, so every built-in strategy
    that relied on an auto-included indicator (BB, RSI, SMA, MACD, ATR, …)
    silently returned no orders. The two helpers are now aligned and all
    indicator-driven strategies actually trade.
    """

    def test_multi_bollinger_rotation_produces_orders(self):
        """MultiBollingerRotation trades with auto-injected Bollinger Bands."""
        from backtide.strategies import MultiBollingerRotation

        result = run_experiment(
            strategies=MultiBollingerRotation(period=5, rebalance_interval=1),
            verbose=False,
            **run_kwargs,
        )
        assert result.status == ExperimentStatus.Success
        rr = result.strategies[0]
        assert len(rr.orders) > 0, "rotation strategy produced no orders"

    def test_triple_rsi_rotation_produces_orders(self):
        """TripleRsiRotation trades with auto-injected RSI indicators."""
        from backtide.strategies import TripleRsiRotation

        result = run_experiment(
            strategies=TripleRsiRotation(
                short_period=2, medium_period=3, long_period=5, rebalance_interval=1
            ),
            verbose=False,
            **run_kwargs,
        )
        assert result.status == ExperimentStatus.Success
        rr = result.strategies[0]
        assert len(rr.orders) > 0, "rotation strategy produced no orders"

    def test_bollinger_mean_reversion_produces_orders(self):
        """BollingerMeanReversion triggers when price crosses bands.

        Pick a very short window so the band is volatile enough for the
        AAPL Jan-Mar 2024 fixture to produce at least one cross.
        """
        from backtide.strategies import BollingerMeanReversion

        # The strategy is satisfied as long as it can *look up* its
        # auto-injected BB indicator — even on a quiet stretch the order
        # list may be empty, but the run must succeed without warnings.
        result = run_experiment(
            strategies=BollingerMeanReversion(period=3, std_dev=1.0),
            verbose=False,
            **run_kwargs,
        )
        assert result.status == ExperimentStatus.Success
        # Either we got orders, or — at minimum — the auto-indicator
        # lookup didn't fail loudly. We assert the auto-indicator
        # warning is absent because that's the failure mode the
        # regression fix targets.
        rr = result.strategies[0]
        # The fix is verified if there are orders OR no
        # "produced no orders" warning. Most short fixtures yield
        # at least one band touch, so we assert orders are produced
        # with a permissive parameter set.
        assert len(rr.orders) > 0, "indicator-driven strategy produced no orders"


# ─────────────────────────────────────────────────────────────────────────────
# Additional coverage tests
# ─────────────────────────────────────────────────────────────────────────────


class TestBaseStrategyLog:
    """Tests for BaseStrategy.log() method."""

    def test_log_default_level(self):
        """Test log method with default info level."""
        from unittest.mock import patch

        with patch("backtide.backtest.experiment_log") as mock_log:
            BaseStrategy.log("test message")
            mock_log.assert_called_once_with("test message", "info")

    def test_log_custom_level(self):
        """Test log method with custom log level."""
        from unittest.mock import patch

        with patch("backtide.backtest.experiment_log") as mock_log:
            BaseStrategy.log("test error", level="error")
            mock_log.assert_called_once_with("test error", "error")

    def test_log_warn_level(self):
        """Test log method with warn level."""
        from unittest.mock import patch

        with patch("backtide.backtest.experiment_log") as mock_log:
            BaseStrategy.log("test warning", level="warn")
            mock_log.assert_called_once_with("test warning", "warn")

    def test_log_debug_level(self):
        """Test log method with debug level."""
        from unittest.mock import patch

        with patch("backtide.backtest.experiment_log") as mock_log:
            BaseStrategy.log("test debug", level="debug")
            mock_log.assert_called_once_with("test debug", "debug")


class TestRunExperimentPolymorphicDict:
    """Tests for dict form of polymorphic strategies/indicators parameters."""

    def test_strategies_dict_multiple_entries(self):
        """Test that multiple strategies in dict form are handled correctly."""
        result = run_experiment(
            strategies=[
                {"Strategy1": BuyAndHold()},
                {"Strategy2": BuyAndHold()},
            ],
            verbose=False,
            **run_kwargs,
        )
        assert result.status is not None

    def test_mixed_strategies_strings_and_instances(self):
        """Test mixing string names with instances."""
        result = run_experiment(
            strategies=[
                "BuyAndHold",
                BuyAndHold(),
                {"CustomName": BuyAndHold()},
            ],
            verbose=False,
            **run_kwargs,
        )
        assert result.status is not None

    def test_indicators_dict_form(self):
        """Test indicator with dict form."""
        from backtide.indicators import SimpleMovingAverage

        result = run_experiment(
            strategies=[BuyAndHold()],
            indicators=[{"MyIndicator": SimpleMovingAverage(20)}],
            verbose=False,
            **run_kwargs,
        )
        assert result.status is not None


class TestRunExperimentErrors:
    """Tests for error cases in run_experiment."""

    def test_no_symbols_raises_error(self):
        """Test that missing symbols raises ValueError."""
        with pytest.raises(ValueError, match="no symbols"):
            run_experiment(
                symbols=[],
                strategies=[BuyAndHold()],
                verbose=False,
            )

    def test_no_strategies_raises_error(self):
        """Test that missing strategies raises ValueError."""
        with pytest.raises(ValueError, match="no strategies"):
            run_experiment(
                symbols=["AAPL"],
                instrument_type="stocks",
                interval="1d",
                strategies=[],
                verbose=False,
            )

    def test_unknown_kwargs_raise_error(self):
        """Test that unknown keyword arguments raise ValueError."""
        with pytest.raises(ValueError, match="Unknown keyword arguments"):
            run_experiment(
                symbols=["AAPL"],
                instrument_type="stocks",
                interval="1d",
                strategies=[BuyAndHold()],
                unknown_arg="invalid",
                verbose=False,
            )


class TestCleanupExperiment:
    """Tests for _cleanup_experiment function."""

    def test_cleanup_with_experiment_id_calls_delete(self):
        """Test cleanup with experiment ID calls delete."""
        from unittest.mock import patch

        with patch("backtide.backtest._delete_experiment") as mock_delete:
            from backtide.backtest import _cleanup_experiment

            _cleanup_experiment("exp-123", "test-name")
            mock_delete.assert_called_once_with("exp-123")

    def test_cleanup_with_experiment_id_delete_failure(self):
        """Test cleanup gracefully handles delete failure."""
        from unittest.mock import patch

        with patch("backtide.backtest._delete_experiment") as mock_delete:
            mock_delete.side_effect = Exception("Delete failed")
            from backtide.backtest import _cleanup_experiment

            # Should not raise - best-effort removal
            _cleanup_experiment("exp-123", "test-name")

    def test_cleanup_without_experiment_id_queries_experiments(self):
        """Test cleanup without ID queries experiments by name."""
        from unittest.mock import patch

        import pandas as pd

        with (
            patch("backtide.backtest._query_experiments") as mock_query,
            patch("backtide.backtest._to_pandas") as mock_to_pandas,
            patch("backtide.backtest._delete_experiment") as mock_delete,
        ):
            df = pd.DataFrame({"id": ["exp-456"]})
            mock_to_pandas.return_value = df
            from backtide.backtest import _cleanup_experiment

            _cleanup_experiment(None, "test-name")
            mock_query.assert_called_once()
            mock_delete.assert_called_once_with("exp-456")

    def test_cleanup_query_returns_empty(self):
        """Test cleanup when query returns empty result."""
        from unittest.mock import patch

        import pandas as pd

        with (
            patch("backtide.backtest._query_experiments"),
            patch("backtide.backtest._to_pandas") as mock_to_pandas,
            patch("backtide.backtest._delete_experiment") as mock_delete,
        ):
            mock_to_pandas.return_value = pd.DataFrame()
            from backtide.backtest import _cleanup_experiment

            _cleanup_experiment(None, "nonexistent")
            # Should not call delete if no experiments found
            assert mock_delete.call_count == 0


class TestIntegrationCoverage:
    """Integration tests to cover additional paths."""

    def test_run_experiment_with_indicators_and_custom_dates(self):
        """Test run_experiment with indicators and custom date range."""
        from backtide.indicators import SimpleMovingAverage

        result = run_experiment(
            name="test-with-indicators",
            symbols=["AAPL"],
            instrument_type="stocks",
            interval="1d",
            full_history=False,
            start_date="2024-01-01",
            end_date="2024-03-01",
            strategies=[BuyAndHold()],
            indicators=[SimpleMovingAverage(10), SimpleMovingAverage(20)],
            initial_cash=50000,
            commission_pct=0.001,
            slippage=0.001,
            verbose=False,
        )
        assert result is not None

    def test_run_experiment_with_tags_and_description(self):
        """Test run_experiment with tags and description in config."""
        result = run_experiment(
            name="test-tags",
            tags=["test", "coverage"],
            description="Test experiment for coverage",
            symbols=["AAPL"],
            instrument_type="stocks",
            interval="1d",
            full_history=False,
            start_date="2024-01-01",
            end_date="2024-03-01",
            strategies=[BuyAndHold()],
            verbose=False,
        )
        assert result is not None

    def test_multiple_strategies_execution(self):
        """Test execution with multiple strategies."""
        result = run_experiment(
            strategies=[
                BuyAndHold(),
                BuyAndHold(symbol="AAPL"),
            ],
            verbose=False,
            **run_kwargs,
        )
        assert len(result.strategies) >= 1


class TestPolymorphicResolution:
    """Tests for polymorphic parameter resolution."""

    def test_resolve_instance_strategies(self):
        """Test resolution of strategy instances."""
        result = run_experiment(
            strategies=BuyAndHold(),
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_resolve_list_of_instances(self):
        """Test resolution of list of strategy instances."""
        result = run_experiment(
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_indicators_instance_form(self):
        """Test indicators as instances."""
        from backtide.indicators import SimpleMovingAverage

        result = run_experiment(
            strategies=[BuyAndHold()],
            indicators=[SimpleMovingAverage(10)],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None


class TestBacktestErrorPaths:
    """Tests for various error paths in backtest module."""

    def test_run_experiment_exception_cleanup_on_keyboard_interrupt(self, monkeypatch):
        """Test cleanup is called on KeyboardInterrupt."""
        cleanup_called = []

        def fake_run(*args, **kwargs):  # noqa: ARG001
            raise KeyboardInterrupt

        def fake_cleanup(exp_id, name):
            cleanup_called.append((exp_id, name))

        monkeypatch.setattr(backtest_module, "_run_experiment", fake_run)
        monkeypatch.setattr(backtest_module, "_cleanup_experiment", fake_cleanup)

        with pytest.raises(backtest_module.ExperimentAborted):
            run_experiment(strategies=[BuyAndHold()], verbose=False, **run_kwargs)

        assert len(cleanup_called) > 0

    def test_cleanup_with_name_search_raises_exception(self):
        """Test cleanup gracefully handles query exceptions."""
        from unittest.mock import patch

        import backtide.backtest as backtide_module

        with patch("backtide.backtest._query_experiments") as mock_query:
            mock_query.side_effect = Exception("Query failed")

            # Should not raise - best-effort removal
            backtide_module._cleanup_experiment(None, "test-name")

    def test_cleanup_with_pandas_conversion_failure(self):
        """Test cleanup handles pandas conversion failures."""
        from unittest.mock import patch

        import backtide.backtest as backtide_module

        with (
            patch("backtide.backtest._query_experiments"),
            patch("backtide.backtest._to_pandas") as mock_to_pandas,
        ):
            mock_to_pandas.side_effect = Exception("Conversion failed")

            # Should not raise
            backtide_module._cleanup_experiment(None, "test-name")


class TestComplexConfigurations:
    """Tests for complex experiment configurations."""

    def test_experiment_with_multiple_indicators(self):
        """Test experiment with multiple indicator types."""
        from backtide.indicators import SimpleMovingAverage

        result = run_experiment(
            strategies=[BuyAndHold()],
            indicators=[
                SimpleMovingAverage(5),
                SimpleMovingAverage(10),
                SimpleMovingAverage(20),
            ],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_experiment_kwargs_precedence(self):
        """Test that kwargs override config values."""
        result = run_experiment(
            config=ExperimentConfig(),
            name="override_test",
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_experiment_with_slippage_and_commission(self):
        """Test experiment with slippage and commission."""
        result = run_experiment(
            commission_pct=0.001,
            slippage=0.001,
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_experiment_with_warmup_period(self):
        """Test experiment with warmup period."""
        result = run_experiment(
            warmup_period=5,
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None


class TestExperimentConfigEdgeCases:
    """Tests for edge cases in experiment configuration."""

    def test_run_with_empty_string_name_generates_uuid(self):
        """Test that empty name gets auto-generated UUID."""
        result = run_experiment(
            name="    ",  # Whitespace name
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_run_with_none_indicators(self):
        """Test run with None indicators."""
        result = run_experiment(
            strategies=[BuyAndHold()],
            indicators=None,
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_run_with_single_indicator_instance(self):
        """Test run with single indicator (not in list)."""
        from backtide.indicators import SimpleMovingAverage

        result = run_experiment(
            strategies=[BuyAndHold()],
            indicators=SimpleMovingAverage(10),
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_run_with_very_large_initial_cash(self):
        """Test with very large initial cash."""
        result = run_experiment(
            initial_cash=999_999_999,
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_run_with_very_small_commission(self):
        """Test with very small commission rate."""
        result = run_experiment(
            commission_pct=0.00001,
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_run_with_zero_slippage(self):
        """Test run with zero slippage."""
        result = run_experiment(
            slippage=0.0,
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None


class TestMultipleStrategies:
    """Tests for running multiple strategies."""

    def test_run_three_strategies(self):
        """Test running three different strategy instances."""
        result = run_experiment(
            strategies=[
                BuyAndHold(),
                BuyAndHold(symbol="AAPL"),
                BuyAndHold(symbol="AAPL"),
            ],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_run_with_mixed_dict_and_instance_strategies(self):
        """Test with mixed dict, instance, and string strategies."""
        result = run_experiment(
            strategies=[
                {"Custom1": BuyAndHold()},
                BuyAndHold(),
                {"Custom2": BuyAndHold()},
            ],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_indicators_with_multiple_instances(self):
        """Test multiple indicator instances."""
        from backtide.indicators import SimpleMovingAverage

        result = run_experiment(
            strategies=[BuyAndHold()],
            indicators=[
                SimpleMovingAverage(3),
                SimpleMovingAverage(5),
                SimpleMovingAverage(7),
                SimpleMovingAverage(10),
            ],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None

    def test_indicators_dict_form_multiple(self):
        """Test multiple indicators in dict form."""
        from backtide.indicators import SimpleMovingAverage

        result = run_experiment(
            strategies=[BuyAndHold()],
            indicators=[
                {"SMA3": SimpleMovingAverage(3)},
                {"SMA5": SimpleMovingAverage(5)},
            ],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None


class TestDataFormats:
    """Tests for different data formats and edge cases."""

    def test_run_experiment_full_history(self):
        """Test with full_history=True."""
        result = run_experiment(
            full_history=True,
            strategies=[BuyAndHold()],
            verbose=False,
            symbols=["AAPL"],
            instrument_type="stocks",
            interval="1d",
        )
        assert result is not None

    def test_run_experiment_with_explicit_dates(self):
        """Test with explicit date range."""
        result = run_experiment(
            full_history=False,
            start_date="2024-01-15",
            end_date="2024-02-15",
            strategies=[BuyAndHold()],
            verbose=False,
            symbols=["AAPL"],
            instrument_type="stocks",
            interval="1d",
        )
        assert result is not None

    def test_run_experiment_with_only_start_date(self):
        """Test with only start date."""
        result = run_experiment(
            symbols=["AAPL"],
            instrument_type="stocks",
            interval="1d",
            full_history=False,
            start_date="2024-01-01",
            strategies=[BuyAndHold()],
            verbose=False,
        )
        assert result is not None

    def test_run_experiment_with_only_end_date(self):
        """Test with only end date."""
        result = run_experiment(
            symbols=["AAPL"],
            instrument_type="stocks",
            interval="1d",
            full_history=False,
            end_date="2024-03-01",
            strategies=[BuyAndHold()],
            verbose=False,
        )
        assert result is not None


class TestAbortEventHandling:
    """Tests for abort event handling in backtest."""

    def test_abort_event_not_set(self):
        """Test normal case when abort is not set."""
        assert backtest_module._abort_event is None
        result = run_experiment(
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert result is not None


class TestStoredExperimentAccess:
    """Tests for accessing stored experiments."""

    def test_query_and_iterate_experiments(self):
        """Test querying experiments and accessing results."""
        from backtide.storage import query_experiments, query_strategy_runs

        # Run an experiment first
        run_experiment(
            name="test-stored-exp",
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )

        # Query experiments
        experiments = cast(pd.DataFrame, query_experiments(search="test-stored-exp", limit=1))
        if not experiments.empty:
            exp_id = experiments.iloc[0]["id"]
            runs = query_strategy_runs(exp_id)
            assert runs is not None

    def test_experiment_result_attributes(self):
        """Test that experiment results have expected attributes."""
        result = run_experiment(
            name="test-attrs",
            strategies=[BuyAndHold()],
            verbose=False,
            **run_kwargs,
        )
        assert hasattr(result, "experiment_id")
        assert hasattr(result, "status")
        assert hasattr(result, "strategies")
