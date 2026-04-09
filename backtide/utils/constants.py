"""Backtide.

Author: Mavs
Description: Constants shared by the package.

"""

import re

# Regex pattern to which tags must comply
TAG_PATTERN = re.compile(r"^[\w-]{1,15}$")

# Maximum number of assets to download or backtest at the same time
MAX_ASSET_SELECTION = 10

# Number of preloaded assets displayed in the UI
MAX_PRELOADED_ASSETS = 1500

STRATEGY_PLACEHOLDER = """\
def strategy(data, state, indicators):
    '''Function that decides the orders to place this tick.

    Parameters
    ---------
    data : pd.DataFrame
        Ticker data.

    state : State
        Current portfolio, etc...

    indicators: dict[str, dict[str, float]] | None
        Indicators calculated on the historical data. The first key is the
        symbol and the second key is the name of the indicator. None if no
        indicators were selected.

    Returns
    -------
    list[Order]
        Orders to place.

    '''
    orders = []

    # ── Write your logic here ──────────────────────────

    return orders
"""


INDICATOR_PLACEHOLDER = """\
def indicator(data):
    '''Compute a custom indicator value for the current bar.

    Parameters
    ----------
    data : pd.DataFrame
        Historical OHLCV data up to and including the current bar.

    Returns
    -------
    dict[str, float]
        A mapping of indicator name(s) to their computed value(s).
        Example: {"my_signal": 0.75, "my_trend": 1.0}

    '''
    result = {}

    # ── Write your logic here ──────────────────────────

    return result
"""
