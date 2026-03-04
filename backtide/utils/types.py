"""Backtide.

Author: Mavs
Description: Utility types.

"""

from typing import TypedDict


class IndexSymbol(TypedDict):
    """Index symbol as returned by `pytickersymbols`."""

    yahoo: str
    google: str
    currency: str


class Company(TypedDict):
    """Company data as returned by `pytickersymbols`."""

    name: str
    symbol: str
    country: str
    symbols: list[IndexSymbol]


class IndexData(TypedDict):
    """Index data as returned by `pytickersymbols`."""

    name: str
    companies: list[Company]
