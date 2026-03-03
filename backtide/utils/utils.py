"""Backtide.

Author: Mavs
Description: Utility functions.

"""


def format_compact(n: float) -> str:
    """Transform a number to a formatted string.

    Parameters
    ----------
    n : int | float
        Number ot format.

    Returns
    -------
    str
        Number with `M` for millions or `k` for thousands.

    """
    if abs(n) >= 1_000_000:
        return f"{n / 1_000_000:.1f}M"
    elif abs(n) >= 1_000:
        return f"{n / 1_000:.1f}k"
    else:
        return int(n)
