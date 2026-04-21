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
    ExchangeExpConfig,
    ExperimentConfig,
    GeneralExpConfig,
    OrderType,
    PortfolioExpConfig,
    StrategyType,
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
        (StrategyType, "BuyAndHold"),
        (StrategyType, "SmaCrossover"),
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
        StrategyType,
    ],
)
def test_enum_variants(cls):
    """All backtest enums have non-empty variants()."""
    assert len(cls.variants()) > 0


@pytest.mark.parametrize(
    "cls",
    [CommissionType, EmptyBarPolicy, OrderType, StrategyType],
)
def test_enum_get_default(cls):
    """Backtest enums with get_default return a value."""
    assert cls.get_default() is not None


class TestStrategyType:
    """Tests for the StrategyType enum."""

    def test_name(self):
        """Test name property."""
        assert StrategyType("BuyAndHold").name == "Buy & Hold"

    def test_description(self):
        """Test description method."""
        assert len(StrategyType("BuyAndHold").description()) > 0

    def test_is_rotation(self):
        """Test is_rotation property."""
        assert StrategyType("RocRotation").is_rotation is True
        assert StrategyType("BuyAndHold").is_rotation is False

    def test_invalid_raises(self):
        """Test invalid value raises ValueError."""
        with pytest.raises(ValueError, match="Unknown strategy type"):
            StrategyType("invalid")


class TestCommissionType:
    """Tests for the CommissionType enum."""

    def test_str(self):
        """Test string representation."""
        assert "Percentage" in str(CommissionType("Percentage"))

    def test_invalid_raises(self):
        """Test invalid value raises ValueError."""
        with pytest.raises(ValueError, match="Unknown commission type"):
            CommissionType("invalid")
