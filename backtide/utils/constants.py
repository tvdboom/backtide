"""Backtide.

Author: Mavs
Description: Constants shared by the package.

"""

import re


# Maximum number of assets to download or backtest at the same time
MAX_ASSET_SELECTION = 10

# Regex pattern to which tags must comply
TAG_PATTERN = re.compile(r"^[\w-]{1,15}$")
