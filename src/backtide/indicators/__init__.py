"""Backtide.

Author: Mavs
Description: Indicator functionalities for backtide.

"""

from backtide.core.backtest import (
    AverageDirectionalIndex,
    AverageTrueRange,
    BollingerBands,
    CommodityChannelIndex,
    ExponentialMovingAverage,
    MovingAverageConvergenceDivergence,
    OnBalanceVolume,
    RelativeStrengthIndex,
    SimpleMovingAverage,
    StochasticOscillator,
    VolumeWeightedAveragePrice,
    WeightedMovingAverage,
)
from backtide.indicators.base import BaseIndicator

# List all built-in indicators
BUILTIN_INDICATORS = [
    AverageDirectionalIndex,
    AverageTrueRange,
    BollingerBands,
    CommodityChannelIndex,
    ExponentialMovingAverage,
    MovingAverageConvergenceDivergence,
    OnBalanceVolume,
    RelativeStrengthIndex,
    SimpleMovingAverage,
    StochasticOscillator,
    VolumeWeightedAveragePrice,
    WeightedMovingAverage,
]
