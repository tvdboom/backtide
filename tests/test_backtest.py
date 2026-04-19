"""Backtide.

Author: Mavs
Description: Unit tests for the backtest module.

"""

import pytest

from backtide.backtest import (
    CodeSnippet,
    CommissionType,
    ConversionPeriod,
    CurrencyConversionMode,
    DataExpConfig,
    EmptyBarPolicy,
    EngineExpConfig,
    ExchangeExpConfig,
    ExperimentConfig,
    GeneralExpConfig,
    IndicatorExpConfig,
    IndicatorType,
    OrderType,
    PortfolioExpConfig,
    StrategyExpConfig,
    StrategyType,
)


# ─────────────────────────────────────────────────────────────────────────────
# CodeSnippet
# ─────────────────────────────────────────────────────────────────────────────


class TestCodeSnippet:
    """Tests for the CodeSnippet model."""

    def test_defaults(self):
        cs = CodeSnippet()
        assert cs.name == ""
        assert cs.code == ""

    def test_custom(self):
        cs = CodeSnippet(name="my_strategy", code="return True")
        assert cs.name == "my_strategy"
        assert cs.code == "return True"

    def test_repr(self):
        assert "CodeSnippet" in repr(CodeSnippet())

    def test_equality(self):
        assert CodeSnippet() == CodeSnippet()
        assert CodeSnippet(name="a") != CodeSnippet(name="b")


# ─────────────────────────────────────────────────────────────────────────────
# Sub-configs
# ─────────────────────────────────────────────────────────────────────────────


class TestGeneralExpConfig:
    def test_defaults(self):
        c = GeneralExpConfig()
        assert c.name == ""
        assert c.tags == []
        assert c.description == ""

    def test_custom(self):
        c = GeneralExpConfig(name="test", tags=["a"], description="desc")
        assert c.name == "test"

    def test_to_dict(self):
        d = GeneralExpConfig().to_dict()
        assert "name" in d
        assert "tags" in d

    def test_repr(self):
        assert "GeneralExpConfig" in repr(GeneralExpConfig())


class TestDataExpConfig:
    def test_defaults(self):
        c = DataExpConfig()
        assert c.full_history is True
        assert c.symbols == []

    def test_custom(self):
        c = DataExpConfig(symbols=["AAPL"], full_history=False, start_date="2020-01-01")
        assert c.symbols == ["AAPL"]
        assert c.full_history is False
        assert c.start_date == "2020-01-01"

    def test_to_dict(self):
        d = DataExpConfig().to_dict()
        assert "instrument_type" in d
        assert "interval" in d


class TestPortfolioExpConfig:
    def test_defaults(self):
        c = PortfolioExpConfig()
        assert c.initial_cash == 10000

    def test_custom(self):
        c = PortfolioExpConfig(initial_cash=50000, base_currency="EUR")
        assert c.initial_cash == 50000


class TestExchangeExpConfig:
    def test_defaults(self):
        c = ExchangeExpConfig()
        assert isinstance(c.to_dict(), dict)


class TestEngineExpConfig:
    def test_defaults(self):
        c = EngineExpConfig()
        assert c.warmup_period == 0
        assert c.trade_on_close is False

    def test_repr(self):
        assert "EngineExpConfig" in repr(EngineExpConfig())


# ─────────────────────────────────────────────────────────────────────────────
# ExperimentConfig
# ─────────────────────────────────────────────────────────────────────────────


class TestExperimentConfig:
    def test_defaults(self):
        ec = ExperimentConfig()
        assert ec.general.name == ""
        assert ec.data.symbols == []

    def test_to_dict(self):
        d = ExperimentConfig().to_dict()
        assert "general" in d
        assert "data" in d
        assert "portfolio" in d
        assert "engine" in d

    def test_to_toml_from_toml_roundtrip(self):
        ec = ExperimentConfig(
            general=GeneralExpConfig(name="roundtrip"),
            data=DataExpConfig(symbols=["AAPL"]),
        )
        toml_str = ec.to_toml()
        ec2 = ExperimentConfig.from_toml(toml_str)
        assert ec2.general.name == "roundtrip"
        assert ec2.data.symbols == ["AAPL"]

    def test_to_dict_from_dict_roundtrip(self):
        ec = ExperimentConfig(general=GeneralExpConfig(name="test"))
        d = ec.to_dict()
        ec2 = ExperimentConfig.from_dict(d)
        assert ec2.general.name == "test"

    def test_equality(self):
        assert ExperimentConfig() == ExperimentConfig()
        assert ExperimentConfig(general=GeneralExpConfig(name="a")) != ExperimentConfig()

    def test_repr(self):
        assert "ExperimentConfig" in repr(ExperimentConfig())


# ─────────────────────────────────────────────────────────────────────────────
# Backtest enums
# ─────────────────────────────────────────────────────────────────────────────


@pytest.mark.parametrize(
    "cls,valid_str",
    [
        (CommissionType, "Percentage"),
        (CommissionType, "Fixed"),
        (CommissionType, "PercentagePlusFixed"),
        (IndicatorType, "Sma"),
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
    [CommissionType, ConversionPeriod, CurrencyConversionMode, EmptyBarPolicy,
     IndicatorType, OrderType, StrategyType],
)
def test_enum_variants(cls):
    """All backtest enums have non-empty variants()."""
    assert len(cls.variants()) > 0


@pytest.mark.parametrize(
    "cls",
    [CommissionType, EmptyBarPolicy, IndicatorType, OrderType, StrategyType],
)
def test_enum_get_default(cls):
    """Backtest enums with get_default return a value."""
    assert cls.get_default() is not None


class TestStrategyType:
    def test_name(self):
        assert StrategyType("BuyAndHold").name == "Buy & Hold"

    def test_description(self):
        assert len(StrategyType("BuyAndHold").description()) > 0

    def test_is_rotation(self):
        assert StrategyType("RocRotation").is_rotation is True
        assert StrategyType("BuyAndHold").is_rotation is False

    def test_invalid_raises(self):
        with pytest.raises(ValueError, match="Unknown strategy type"):
            StrategyType("invalid")


class TestCommissionType:
    def test_str(self):
        assert "Percentage" in str(CommissionType("Percentage"))

    def test_invalid_raises(self):
        with pytest.raises(ValueError, match="Unknown commission type"):
            CommissionType("invalid")


