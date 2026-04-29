"""Backtide.

Author: Mavs
Description: Strategy functionalities for backtide.

"""

from typing import Any, Callable

from backtide.core.backtest import (
    AdaptiveRsi,
    AlphaRsiPro,
    BollingerMeanReversion,
    BuyAndHold,
    DoubleTop,
    HybridAlphaRsi,
    Macd,
    Momentum,
    MultiBollingerRotation,
    RiskAverse,
    Roc,
    RocRotation,
    Rsi,
    Rsrs,
    RsrsRotation,
    SmaCrossover,
    SmaNaive,
    TripleRsiRotation,
    TurtleTrading,
    Vcp,
)
from backtide.strategies.base import BaseStrategy
from backtide.indicators import (
    AverageTrueRange,
    BollingerBands,
    MovingAverageConvergenceDivergence,
    RelativeStrengthIndex,
    SimpleMovingAverage,
)

# List all built-in strategies
BUILTIN_STRATEGIES = [
    AdaptiveRsi,
    AlphaRsiPro,
    BollingerMeanReversion,
    BuyAndHold,
    DoubleTop,
    HybridAlphaRsi,
    Macd,
    Momentum,
    MultiBollingerRotation,
    RiskAverse,
    Roc,
    RocRotation,
    Rsi,
    Rsrs,
    RsrsRotation,
    SmaCrossover,
    SmaNaive,
    TripleRsiRotation,
    TurtleTrading,
    Vcp,
]

# Mapping of built-in strategies to the indicators they conceptually rely on.
# Each entry maps the strategy class to a callable that, given a strategy
# *instance*, returns the list of [Indicator] instances the strategy is
# expected to use. These indicators are auto-included (non-optional) in the
# experiment's indicator set whenever the strategy is selected, so they get
# computed once up-front and made available at evaluate time.
STRATEGY_INDICATORS: dict[type, Callable[[Any], list[Any]]] = {
    AdaptiveRsi: lambda s: [RelativeStrengthIndex(period=int(s.min_period))],
    AlphaRsiPro: lambda s: [RelativeStrengthIndex(period=int(s.period))],
    BollingerMeanReversion: lambda s: [
        BollingerBands(period=int(s.period), std_dev=float(s.std_dev)),
    ],
    HybridAlphaRsi: lambda s: [RelativeStrengthIndex(period=int(s.min_period))],
    Macd: lambda s: [
        MovingAverageConvergenceDivergence(
            fast_period=int(s.fast_period),
            slow_period=int(s.slow_period),
            signal_period=int(s.signal_period),
        ),
    ],
    Momentum: lambda s: [SimpleMovingAverage(period=int(s.ma_period))],
    MultiBollingerRotation: lambda s: [
        BollingerBands(period=int(s.period), std_dev=float(s.std_dev)),
    ],
    Rsi: lambda s: [
        RelativeStrengthIndex(period=int(s.rsi_period)),
        BollingerBands(period=int(s.bb_period), std_dev=float(s.bb_std)),
    ],
    SmaCrossover: lambda s: [
        SimpleMovingAverage(period=int(s.fast_period)),
        SimpleMovingAverage(period=int(s.slow_period)),
    ],
    SmaNaive: lambda s: [SimpleMovingAverage(period=int(s.period))],
    TripleRsiRotation: lambda s: [
        RelativeStrengthIndex(period=int(s.short_period)),
        RelativeStrengthIndex(period=int(s.medium_period)),
        RelativeStrengthIndex(period=int(s.long_period)),
    ],
    TurtleTrading: lambda s: [AverageTrueRange(period=int(s.atr_period))],
}
