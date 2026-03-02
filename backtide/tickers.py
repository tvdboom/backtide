"""Backtide.

Author: Mavs
Description: Functions to retrieve available tickers.

"""

from pytickersymbols.indices_data import INDICES
from pytickersymbols import PyTickerSymbols


data = PyTickerSymbols()



def get_stocks() -> list[str]:
    """Get all available stock tickers."""
    return list(filter(lambda x: x.get("name") and x.get("symbols", {}).get("")))
