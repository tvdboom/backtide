"""Backtide.

Author: Mavs
Description: Types shared by the package.

"""

from typing import TYPE_CHECKING, TypeAlias
import pandas as pd


if TYPE_CHECKING:
    import polars as pl

    DataFrameLike: TypeAlias = pd.DataFrame | pl.DataFrame
else:
    DataFrameLike: TypeAlias = pd.DataFrame
