"""Backtide.

Author: Mavs
Description: Constants shared by the package.

"""

import re

# Link to the documentation page
DOCS_URL = "https://tvdboom.github.io/backtide"

# Regex pattern to which tags must comply
TAG_PATTERN = re.compile(r"^[\s\w-]{1,20}$")

# Characters forbidden in file names (Windows superset covers all platforms)
INVALID_FILENAME_CHARS = re.compile(r'[<>:"/\\|?*\x00-\x1f]')

# Name reserved for the benchmark strategy
BENCHMARK_NAME = "Benchmark"

# Maximum number of instruments to download or backtest at the same time
MAX_INSTRUMENT_SELECTION = 10

# Number of preloaded instruments displayed in the UI
MAX_PRELOADED_INSTRUMENTS = 1500

# Mapping of momentjs codes to a format accepted by Python
MOMENT_TO_STRFTIME = {
    # Year
    "YYYY": "%Y",
    "YY": "%y",
    # Month
    "MMMM": "%B",
    "MMM": "%b",
    "MM": "%m",
    "M": "%-m",  # May not work on Windows
    # Day
    "DD": "%d",
    "D": "%-d",  # May not work on Windows
    # Weekday
    "dddd": "%A",
    "ddd": "%a",
    "dd": "%a",
    # Hour
    "HH": "%H",
    "H": "%-H",
    "hh": "%I",
    "h": "%-I",
    # Minute / Second
    "mm": "%M",
    "m": "%-M",
    "ss": "%S",
    "s": "%-S",
    # AM/PM
    "A": "%p",
    "a": "%p",
    # Timezone
    "Z": "%z",
    "ZZ": "%z",
}
