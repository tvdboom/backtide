"""Backtide.

Author: Mavs
Description: Position sizing functionalities for backtide.

"""

from backtide.core.sizers import (
    EqualWeight,
    FixedFractional,
    FixedNotional,
    FixedQuantity,
    KellyCriterion,
    RiskBased,
    VolatilityScaled,
)
from backtide.sizers.base import BaseSizer

BUILTIN_SIZERS = (
    EqualWeight,
    FixedFractional,
    FixedNotional,
    FixedQuantity,
    KellyCriterion,
    RiskBased,
    VolatilityScaled,
)

__all__ = [
    "BUILTIN_SIZERS",
    "BaseSizer",
    "EqualWeight",
    "FixedFractional",
    "FixedNotional",
    "FixedQuantity",
    "KellyCriterion",
    "RiskBased",
    "VolatilityScaled",
]
