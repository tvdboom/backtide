"""Backtide.

Author: Mavs
Description: Indicator functionalities for backtide.

"""

from backtide.core.indicators import (
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
    _indicator_deterministic_name,
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
