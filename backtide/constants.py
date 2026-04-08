"""Backtide.

Author: Mavs
Description: Constants shared by the package.

"""

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
