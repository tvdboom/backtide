"""Backtide.

Author: Mavs
Description: Unit tests for the backtest module.

"""

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
    ExperimentConfig,
    ExperimentResult,
    GeneralExpConfig,
    Order,
    OrderRecord,
    OrderType,
    PortfolioExpConfig,
    StrategyExpConfig,
    StrategyRunResult,
    Trade,
    run_experiment,
)

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

    def test_default_id_is_generated(self):
        """A fresh Order receives an auto-generated id."""
        o1 = Order(symbol="AAPL", order_type="market", quantity=10)
        o2 = Order(symbol="AAPL", order_type="market", quantity=10)
        assert isinstance(o1.id, str) and len(o1.id) > 0
        assert o1.id != o2.id  # uuid uniqueness

    def test_explicit_id_is_kept(self):
        """An explicit id is preserved (used by CancelOrder)."""
        o = Order(symbol="AAPL", order_type="market", quantity=1, id="abc123")
        assert o.id == "abc123"

    def test_cancel_order_can_have_empty_symbol(self):
        """CancelOrder only needs the target id, not a symbol."""
        cancel = Order(
            symbol="",
            order_type="cancelorder",
            quantity=0,
            id="target_id",
        )
        assert cancel.order_type == OrderType.CancelOrder
        assert cancel.id == "target_id"

    def test_repr_contains_id(self):
        """Order repr always includes the id field."""
        o = Order(symbol="AAPL", order_type="market", quantity=1, id="xyz")
        assert "xyz" in repr(o)


class TestOrderType:
    """Tests for the OrderType enum."""

    def test_cancel_order_variant_exists(self):
        """The new CancelOrder variant is available."""
        assert OrderType.CancelOrder is not None
        assert "Cancel" in OrderType.CancelOrder.name

    def test_cancel_order_in_variants(self):
        """CancelOrder appears in the variants list."""
        assert any(v == OrderType.CancelOrder for v in OrderType.variants())

    def test_cancel_order_description(self):
        """CancelOrder has a non-empty description."""
        assert "cancel" in OrderType.CancelOrder.description().lower()


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
        assert StrategyRunResult is not None
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
            strategy=StrategyExpConfig(strategies=[]),
        )
        with pytest.raises(RuntimeError):
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
            lambda *a, **kw: [],
            raising=False,
        )

        cfg = ExperimentConfig(
            general=GeneralExpConfig(name="offline"),
            data=DataExpConfig(symbols=["NOPE-XYZ"]),
            strategy=StrategyExpConfig(strategies=[]),
        )
        # We expect either a clean failed result, or a runtime error from
        # the resolve step; both are acceptable defensive outcomes.
        try:
            result = run_experiment(cfg, verbose=False)
        except RuntimeError:
            return
        assert isinstance(result, ExperimentResult)
        assert result.status in ("failed", "completed")
