"""Backtide.

Author: Mavs
Description: Strategy functionalities for backtide.

"""

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
from backtide.strategies.benchmark import Benchmark

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
