"""Backtide.

Author: Mavs
Description: Unit tests for built-in strategies.

"""

import pickle

import pytest

from backtide.backtest import Portfolio, State
from backtide.strategies import BUILTIN_STRATEGIES


class TestBuiltinStrategies:
    """Tests for the shared built-in strategy contract."""

    @pytest.mark.parametrize("strategy_type", BUILTIN_STRATEGIES)
    def test_default_strategy_exposes_the_public_contract(self, strategy_type: type) -> None:
        """Every built-in exposes metadata, indicators, evaluation, and pickling."""
        strategy = strategy_type()

        orders = strategy.evaluate({}, Portfolio(), State())
        restored = pickle.loads(pickle.dumps(strategy))

        assert strategy.name
        assert strategy.description()
        assert isinstance(strategy.is_multi_asset, bool)
        assert strategy.name in repr(strategy)
        assert isinstance(strategy.required_indicators(), list)
        assert orders == []
        assert type(restored) is strategy_type
