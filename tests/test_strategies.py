"""Backtide.

Author: Mavs
Description: Unit tests for built-in strategies.

"""

import pickle

import pandas as pd
import pytest

from backtide.backtest import Portfolio, State
from backtide.strategies import BUILTIN_STRATEGIES


class TestBuiltinStrategies:
    """Tests for the shared built-in strategy contract."""

    @pytest.mark.parametrize("strategy_type", BUILTIN_STRATEGIES)
    def test_finite_history_without_cash_never_creates_a_buy(self, strategy_type: type) -> None:
        """Accept real OHLCV history without creating unfunded entry orders."""
        strategy = strategy_type()
        data = {
            "AAPL": pd.DataFrame({key: [100.0] * 64 for key in ("open", "high", "low", "close")})
        }
        portfolio = Portfolio(cash={"USD": 0.0})

        orders = strategy.evaluate(data, portfolio, State(bar_index=63, total_bars=64))

        assert orders == []
        assert portfolio.positions == {}
        assert portfolio.orders == []

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
