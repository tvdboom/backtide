"""Backtide.

Author: Mavs
Description: Unit tests for built-in strategies.

"""

import pickle
from typing import Any

import numpy as np
import pandas as pd
import polars as pl
import pytest

from backtide.backtest import OrderType, Portfolio, State
from backtide.data import Currency
from backtide.indicators import SimpleMovingAverage
from backtide.strategies import (
    BUILTIN_STRATEGIES,
    AdaptiveRsi,
    AlphaRsiPro,
    BollingerMeanReversion,
    DoubleTop,
    HybridAlphaRsi,
    Macd,
    Momentum,
    RiskAverse,
    Roc,
    Rsi,
    Rsrs,
    SmaCrossover,
    SmaNaive,
    TurtleTrading,
    Vcp,
)


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

    def test_indicator_result_can_be_used_directly_as_strategy_history(self) -> None:
        """Accept the single-series output returned by the public indicator API."""
        data = pd.DataFrame({key: [1.0, 2.0, 3.0] for key in ("open", "high", "low", "close")})
        indicator = SimpleMovingAverage(2)
        history = indicator.compute(data)

        orders = SmaNaive(2).evaluate(
            {"AAPL": data},
            Portfolio(cash={"USD": 300.0}),
            State(bar_index=2, total_bars=3),
            {"SMA_2": {"AAPL": history}},
        )

        assert len(orders) == 1
        assert orders[0].symbol == "AAPL"
        assert orders[0].quantity == 100.0

    @pytest.mark.parametrize("dataframe", [pd.DataFrame, pl.DataFrame, np.asarray])
    @pytest.mark.parametrize(
        ("strategy", "buy_prices", "buy_indicators", "sell_prices", "sell_indicators"),
        [
            (
                AdaptiveRsi(3, 5),
                [100, 101, 99, 102, 98, 103],
                {"RSI_3": [20], "RSI_5": [20]},
                [100, 101, 99, 102, 98, 103],
                {"RSI_3": [80], "RSI_5": [80]},
            ),
            (
                AlphaRsiPro(2, 2),
                [100, 105, 110],
                {"RSI_2": [20]},
                [110, 105, 100],
                {"RSI_2": [90]},
            ),
            (
                HybridAlphaRsi(2, 3, 2),
                [100, 105, 110],
                {"RSI_2": [20], "RSI_3": [20]},
                [110, 105, 100],
                {"RSI_2": [90], "RSI_3": [90]},
            ),
            (
                BollingerMeanReversion(3, 1.0),
                [80],
                {"BB_3_1p0": [110, 100, 90]},
                [120],
                {"BB_3_1p0": [110, 100, 90]},
            ),
            (Macd(2, 3, 2), [100], {"MACD_2_3_2": [2, 1]}, [100], {"MACD_2_3_2": [1, 2]}),
            (Momentum(2, 2), [100, 105, 110], {"SMA_2": [100]}, [110, 105, 100], {"SMA_2": [105]}),
            (RiskAverse(2, 2), [100, 101, 110], {"ATR_2": [1]}, [110, 100, 90], {"ATR_2": [1]}),
            (
                Rsi(2, 3, 1.0),
                [80],
                {"RSI_2": [20], "BB_3_1p0": [110, 100, 90]},
                [100],
                {"RSI_2": [80], "BB_3_1p0": [110, 100, 90]},
            ),
            (
                SmaCrossover(2, 3),
                [100],
                {"SMA_2": [110], "SMA_3": [100]},
                [100],
                {"SMA_2": [90], "SMA_3": [100]},
            ),
            (SmaNaive(2), [110], {"SMA_2": [100]}, [90], {"SMA_2": [100]}),
            (
                DoubleTop(6),
                [90, 95, 100, 110, 100, 109, 100, 111],
                {},
                [90, 95, 100, 110, 100, 109, 100, 90],
                {},
            ),
            (Roc(2), [100, 105, 110], {}, [110, 105, 100], {}),
            (Rsrs(3), [100, 105, 110], {}, [110, 105, 100], {}),
            (Vcp(6, 3), [80, 100, 90, 100, 99, 101], {}, [0, 100, 40, 80, 49, 50], {}),
            (
                TurtleTrading(3, 2, 2),
                [100, 105, 110],
                {"ATR_2": [2]},
                [110, 100, 90],
                {"ATR_2": [2]},
            ),
        ],
    )
    def test_entry_and_exit_signals_cross_the_python_boundary(
        self,
        strategy: Any,
        buy_prices: list[int],
        buy_indicators: dict[str, list[int]],
        sell_prices: list[int],
        sell_indicators: dict[str, list[int]],
        dataframe: Any,
    ) -> None:
        """Preserve funded entries and complete exits for every supported input family."""
        for prices, values, position in (
            (buy_prices, buy_indicators, 0.0),
            (sell_prices, sell_indicators, 2.0),
        ):
            # Every output series has one value per visible bar; use the last snapshot.
            data = np.repeat(np.asarray(prices, dtype=float)[:, None], 4, axis=1)
            outputs = {
                name: {"AAPL": np.tile(np.asarray(value, dtype=float), (len(prices), 1))}
                for name, value in values.items()
            }
            if dataframe is not np.asarray:
                data = dataframe(dict(zip(("open", "high", "low", "close"), data.T, strict=True)))
                outputs = {
                    name: {
                        "AAPL": dataframe({str(i): col for i, col in enumerate(payload["AAPL"].T)})
                    }
                    for name, payload in outputs.items()
                }
            else:
                data = np.rec.fromarrays(data.T, names=("open", "high", "low", "close"))
            portfolio = Portfolio(cash={"USD": 10_000.0}, positions={"AAPL": position})

            orders = strategy.evaluate(
                {"AAPL": data},
                portfolio,
                State(bar_index=len(prices) - 1, total_bars=len(prices)),
                outputs,
            )

            assert len(orders) == 1
            assert orders[0].symbol == "AAPL"
            assert orders[0].order_type == OrderType.Market
            if position == 0.0:
                assert orders[0].quantity > 0.0
            else:
                assert orders[0].quantity == -position
            assert portfolio.positions == {"AAPL": position}
            assert portfolio.cash == {Currency.USD: 10_000.0}
            assert portfolio.orders == []
