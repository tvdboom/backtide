"""Backtide.

Author: Mavs
Description: Utility functions.

"""

from __future__ import annotations

from collections.abc import Iterable
import importlib
from types import ModuleType
from typing import TYPE_CHECKING, Any, TypeVar, overload
from zoneinfo import ZoneInfo

import numpy as np
import pandas as pd

from backtide.config import DataFrameLibrary, get_config
from backtide.core.data import Currency

if TYPE_CHECKING:
    import polars as pl


T = TypeVar("T")

cfg = get_config()


def _check_dependency(name: str, pypi_name: str | None = None) -> ModuleType:
    """Check an optional dependency.

    Raise an error if the package is not installed.

    Parameters
    ----------
    name: str
        Name of the package to check.

    pypi_name : str | None, default=None
        Name of the package on PyPI. If None, assumes it's the same as `name`.

    """
    try:
        return importlib.import_module(name)
    except ModuleNotFoundError:
        raise ModuleNotFoundError(
            f"Unable to import the {name} package. Install it using pip install "
            f"{pypi_name or name.replace('_', '-')} or install all of backtide's "
            f"optional dependencies with pip install backtide[full]."
        ) from None


def _format_number(n: float) -> str:
    """Transform a number to a nicely formatted string.

    Parameters
    ----------
    n : int | float
        Number to format.

    Returns
    -------
    str
        Formatted string.

    """
    if abs(n) >= 10_000_000_000:
        return f"{int(n / 1_000_000_000)}B"
    elif abs(n) >= 1_000_000_000:
        return f"{n / 1_000_000_000:.1f}B"
    elif abs(n) >= 10_000_000:
        return f"{int(n / 1_000_000)}M"
    elif abs(n) >= 1_000_000:
        return f"{n / 1_000_000:.1f}M"
    elif abs(n) >= 10_000:
        return f"{int(n / 1_000)}k"
    elif abs(n) >= 1_000:
        return f"{n / 1_000:.1f}k"
    else:
        return str(n)


def _format_price(
    n: float,
    decimals: int | None = None,
    currency: str | Currency | None = None,
    *,
    compact: bool = False,
) -> str:
    """Format a price using a currency's symbol and placement convention.

    Parameters
    ----------
    n : int | float
        Number to format.

    decimals : int | None, default=None
        Number of decimal places. If None and the currency is recognized, it uses
        the currency's decimal places (non-compact) or 0 (compact). Else it
        uses 2 (non-compact) or 0 (compact).

    currency : str | Currency | None, default=None
        Currency code to use for formatting. If None, no currency symbol
        is shown.

    compact : bool, default=False
        If True, format the numeric part with `_format_number`.

    Returns
    -------
    str
        Formatted string.

    """
    dec = 2 if decimals is None else decimals

    if currency:
        if not isinstance(currency, Currency):
            try:
                currency = Currency(currency)
            except ValueError:
                return _format_number(n) if compact else f"{n:,.{dec}f}"

        if compact:
            num = _format_number(n)
        else:
            num = f"{n:,.{currency.decimals if decimals is None else dec}f}"

        if cfg.display.currency_prefix:
            return f"{currency.symbol}{num}"
        else:
            return f"{num} {currency.symbol}"

    return _format_number(n) if compact else f"{n:,.{dec}f}"


def _make_dummy_bars(
    backend: DataFrameLibrary, n: int = 5
) -> np.ndarray | pd.DataFrame | pl.DataFrame:
    """Create a dummy OHLCV dataset matching the configured backend."""
    rng = np.random.default_rng(42)

    c = 100.0 + np.cumsum(rng.standard_normal(n))
    o = c + rng.uniform(-1.0, 1.0, n)
    h = c + rng.uniform(0.5, 2.0, n)
    l = c - rng.uniform(0.5, 2.0, n)  # noqa: E741
    v = rng.uniform(1_000, 10_000, n)

    match backend:
        case DataFrameLibrary.Numpy:
            result = np.column_stack([o, h, l, c, v])
        case DataFrameLibrary.Pandas:
            result = pd.DataFrame({"open": o, "high": h, "low": l, "close": c, "volume": v})
        case DataFrameLibrary.Polars:
            pl = _check_dependency("polars")
            result = pl.DataFrame({"open": o, "high": h, "low": l, "close": c, "volume": v})

    return result


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


def _to_pandas(data: Any) -> pd.DataFrame:
    """Ensure an object is converted to a pandas dataframe."""
    if isinstance(data, pd.DataFrame):
        return data

    if hasattr(data, "to_pandas"):
        data = data.to_pandas()

    return pd.DataFrame(data)


def _ts_to_datetime(series: pd.Series, tz: ZoneInfo) -> pd.Series:
    """Convert a Unix-timestamp column to timezone-aware datetimes."""
    return pd.to_datetime(series, unit="s", utc=True).dt.tz_convert(tz)
