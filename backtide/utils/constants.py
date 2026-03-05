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
