"""Backtide.

Author: Mavs
Description: Data models for the UI.

"""

from enum import Enum


class Interval(Enum):
    FiveMinutes = "5m"
    FifteenMinutes = "15m"
    ThirtyMinutes = "30m"
    OneHour = "1h"
    FourHours = "4h"
    OneDay = "1d"
    OneWeek = "1wk"
    OneMonth = "1mo"

    def to_minutes(self) -> int:
        """Number of minutes in this interval."""
        match self:
            case Interval.FiveMinutes:
                return 5
            case Interval.FifteenMinutes:
                return 15
            case Interval.ThirtyMinutes:
                return 30
            case Interval.OneHour:
                return 60
            case Interval.FourHours:
                return 4 * 60
            case Interval.OneDay:
                return 24 * 60
            case Interval.OneWeek:
                return 7 * 24 * 60
            case Interval.OneMonth:
                return 30 * 24 * 60
