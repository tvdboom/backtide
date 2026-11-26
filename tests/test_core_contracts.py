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
from backtide.strategies import BUILTIN_STRATEGIES


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
