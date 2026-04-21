"""Backtide.

Author: Mavs
Description: Utility functions.

"""

from collections.abc import Iterable
from typing import Any, TypeVar, overload
from zoneinfo import ZoneInfo

import pandas as pd

T = TypeVar("T")


def _format_compact(n: float) -> str:
    """Transform a number to a nicely formatted string.

    Parameters
    ----------
    n : int | float
        Number ot format.

    Returns
    -------
    str
        Formatted string.

    """
    if abs(n) >= 10_000_000:
        return f"{n / 1_000_000:.0f}M"
    elif abs(n) >= 1_000_000:
        return f"{n / 1_000_000:.1f}M"
    elif abs(n) >= 10_000:
        return f"{n / 1_000:.0f}k"
    elif abs(n) >= 1_000:
        return f"{n / 1_000:.1f}k"
    else:
        return f"{n:.0f}"


@overload
def _to_list(item: Iterable[T]) -> list[T]: ...
@overload
def _to_list(item: T) -> list[T]: ...
def _to_list(item: Any) -> Any:
    """Convert an item to a list with just the one item if not already.

    Parameters
    ----------
    item : T | Iterable[T]
        Item to convert.

    Returns
    -------
    list[T]
        List of item.

    """
    if isinstance(item, Iterable) and not isinstance(item, (str, bytes)):
        return list(item)
    else:
        return [item]


def _ts_to_datetime(series: pd.Series, tz: ZoneInfo) -> pd.Series:
    """Convert a Unix-timestamp column to timezone-aware datetimes."""
    return pd.to_datetime(series, unit="s", utc=True).dt.tz_convert(tz)
