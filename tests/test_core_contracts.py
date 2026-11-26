"""Backtide.

Author: Mavs
Description: Validate shared Python contracts of Rust-backed public value types.

"""

from __future__ import annotations

from typing import Any

import pytest

from backtide.backtest import (
    CommissionType,
    ConversionPeriod,
    CurrencyConversionMode,
    DataExpConfig,
    EmptyBarPolicy,
    EngineExpConfig,
    ExchangeExpConfig,
    ExperimentStatus,
    GeneralExpConfig,
    OrderStatus,
    OrderType,
    Portfolio,
    PortfolioExpConfig,
    State,
)
from backtide.config import (
    DataConfig,
    DisplayConfig,
    GeneralConfig,
)
from backtide.data import Country, Currency, Exchange, InstrumentType, Interval, Provider
from backtide.indicators import BUILTIN_INDICATORS
from backtide.live import SessionConfig
from backtide.strategies import BUILTIN_STRATEGIES


class TestConfigurationInputs:
    """Test enum conversion when public configuration models receive Python inputs."""

    @pytest.mark.parametrize("period", ConversionPeriod.variants())
    def test_conversion_period_accepts_serialized_names(self, period: ConversionPeriod) -> None:
        """Preserve a currency conversion schedule supplied by a configuration form."""
        config = ExchangeExpConfig(conversion_period=str(period))

        assert config.conversion_period == period

    @pytest.mark.parametrize(
        ("model_type", "field", "value"),
        [
            (DataConfig, "providers", {"stocks": "unknown"}),
            (DataExpConfig, "instrument_type", "unknown"),
            (DataExpConfig, "interval", "unknown"),
            (ExchangeExpConfig, "allowed_order_types", ["unknown"]),
            (ExchangeExpConfig, "conversion_period", "unknown"),
            (GeneralConfig, "base_currency", "unknown"),
            (SessionConfig, "allowed_order_types", ["unknown"]),
        ],
    )
    def test_invalid_enum_fields_are_rejected(
        self, model_type: type[Any], field: str, value: Any
    ) -> None:
        """Reject unsupported configuration choices at the Python extension boundary."""
        with pytest.raises(ValueError, match="Unknown"):
            model_type(**{field: value})


class TestEnumInputs:
    """Test enum validation at the Python extension boundary."""

    @pytest.mark.parametrize(
        "enum_type",
        [
            CommissionType,
            ConversionPeriod,
            Country,
            Currency,
            CurrencyConversionMode,
            EmptyBarPolicy,
            Exchange,
            ExperimentStatus,
            InstrumentType,
            Interval,
            OrderStatus,
            OrderType,
            Provider,
        ],
    )
    def test_unknown_names_are_rejected(self, enum_type: type[Any]) -> None:
        """Reject invalid names instead of returning an unrelated enum default."""
        with pytest.raises(ValueError, match="Unknown"):
            enum_type("not_a_variant")

        assert enum_type.__RUST_ENUM__ is True
        for variant in enum_type.variants():
            assert variant.__repr__() == repr(variant)

    @pytest.mark.parametrize("enum_type", [Currency, Interval, OrderType])
    @pytest.mark.parametrize("value", [None, 123, []])
    def test_wrong_argument_types_are_rejected(self, enum_type: type[Any], value: Any) -> None:
        """Report a type error when a string enum is constructed from non-text input."""
        with pytest.raises(TypeError, match=r"str|String"):
            enum_type(value)


class TestConstructorKeywords:
    """Test typo detection in public extension constructors."""

    @pytest.mark.parametrize(
        "model_type",
        [
            DataConfig,
            DataExpConfig,
            DisplayConfig,
            EngineExpConfig,
            ExchangeExpConfig,
            GeneralConfig,
            GeneralExpConfig,
            Portfolio,
            PortfolioExpConfig,
            State,
            *BUILTIN_INDICATORS,
            *BUILTIN_STRATEGIES,
        ],
    )
    def test_unknown_keywords_are_rejected(self, model_type: type[Any]) -> None:
        """Reject misspelled settings rather than silently changing simulation behavior."""
        with pytest.raises(TypeError, match="unexpected keyword"):
            model_type(misspelled_setting=1)

        instance = model_type()
        assert instance.__repr__() == repr(instance)
