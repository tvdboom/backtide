"""Backtide.

Author: Mavs
Description: Unit tests for utility functions and constants.

"""

from zoneinfo import ZoneInfo

import numpy as np
import pandas as pd
import polars as pl
import pytest

from backtide.config import DataFrameLibrary
from backtide.utils import clear_cache, init_logging
from backtide.utils.constants import INVALID_FILENAME_CHARS, MOMENT_TO_STRFTIME, TAG_PATTERN
from backtide.utils.enum import CaseInsensitiveEnum
from backtide.utils.utils import (
    _check_dependency,
    _format_number,
    _format_price,
    _make_dummy_bars,
    _to_list,
    _to_pandas,
    _ts_to_datetime,
)

# ─────────────────────────────────────────────────────────────────────────────
# _to_list
# ─────────────────────────────────────────────────────────────────────────────


class TestToList:
    """Tests for the _to_list helper."""

    def test_string(self):
        """A string is wrapped in a list (not iterated)."""
        assert _to_list("hello") == ["hello"]

    def test_list_passthrough(self):
        """A list is returned as-is."""
        assert _to_list([1, 2, 3]) == [1, 2, 3]

    def test_single_object(self):
        """A non-iterable object is wrapped."""
        assert _to_list(42) == [42]

    def test_tuple(self):
        """A tuple is converted to a list."""
        assert _to_list((1, 2)) == [1, 2]

    def test_generator(self):
        """A generator is consumed into a list."""
        assert _to_list(x for x in range(3)) == [0, 1, 2]

    def test_bytes(self):
        """Bytes are wrapped (not iterated)."""
        assert _to_list(b"abc") == [b"abc"]


# ─────────────────────────────────────────────────────────────────────────────
# _format_compact
# ─────────────────────────────────────────────────────────────────────────────


class TestFormatCompact:
    """Tests for the _format_compact formatter."""

    @pytest.mark.parametrize(
        ("n", "expected"),
        [
            (0, "0"),
            (999, "999"),
            (1_500, "1.5k"),
            (10_000, "10k"),
            (50_000, "50k"),
            (1_500_000, "1.5M"),
            (10_000_000, "10M"),
            (50_000_000, "50M"),
            (1_500_000_000, "1.5B"),
            (10_000_000_000, "10B"),
        ],
    )
    def test_magnitude(self, n, expected):
        """Each magnitude bracket formats correctly."""
        assert _format_number(n) == expected

    def test_negative(self):
        """Negative numbers use the same brackets."""
        assert "M" in _format_number(-10_000_000)

    def test_negative_billion(self):
        """Negative billion numbers format correctly."""
        assert "B" in _format_number(-2_000_000_000)


# ─────────────────────────────────────────────────────────────────────────────
# _check_dependency
# ─────────────────────────────────────────────────────────────────────────────


class TestCheckDependency:
    """Tests for the _check_dependency helper."""

    def test_existing_module(self):
        """Return module when the dependency exists."""
        mod = _check_dependency("json")
        assert mod is not None
        assert hasattr(mod, "dumps")

    def test_missing_module(self):
        """Raise ModuleNotFoundError for a missing dependency."""
        with pytest.raises(ModuleNotFoundError, match="nonexistent_module"):
            _check_dependency("nonexistent_module")

    def test_missing_module_custom_pypi(self):
        """Raise ModuleNotFoundError with custom pypi name."""
        with pytest.raises(ModuleNotFoundError, match="custom-pypi"):
            _check_dependency("nonexistent_module", pypi_name="custom-pypi")


# ─────────────────────────────────────────────────────────────────────────────
# _format_price
# ─────────────────────────────────────────────────────────────────────────────


class TestFormatPrice:
    """Tests for the _format_price helper."""

    def test_no_currency(self):
        """Format without currency uses 2 decimal places."""
        result = _format_price(1234.5)
        assert result == "1,234.50"

    def test_valid_currency(self):
        """Format with a valid currency includes symbol."""
        result = _format_price(1234.5, currency="USD")
        assert "$" in result

    def test_invalid_currency_fallback(self):
        """Format with invalid currency falls back to plain format."""
        result = _format_price(1234.5, currency="FAKE")
        assert "1,234.50" in result

    def test_custom_decimals(self):
        """Custom decimals override default."""
        result = _format_price(1234.5678, decimals=4)
        assert "1,234.5678" in result


# ─────────────────────────────────────────────────────────────────────────────
# _make_dummy_bars
# ─────────────────────────────────────────────────────────────────────────────


class TestMakeDummyBars:
    """Tests for the _make_dummy_bars helper."""

    def test_numpy_backend(self):
        """Numpy backend returns an ndarray."""
        result = _make_dummy_bars(DataFrameLibrary.Numpy)
        assert isinstance(result, np.ndarray)
        assert result.shape == (5, 5)

    def test_pandas_backend(self):
        """Pandas backend returns a DataFrame."""
        result = _make_dummy_bars(DataFrameLibrary.Pandas)
        assert isinstance(result, pd.DataFrame)
        assert list(result.columns) == ["open", "high", "low", "close", "volume"]

    def test_polars_backend(self):
        """Polars backend returns a polars DataFrame."""
        result = _make_dummy_bars(DataFrameLibrary.Polars)
        assert isinstance(result, pl.DataFrame)
        assert result.columns == ["open", "high", "low", "close", "volume"]

    def test_custom_n(self):
        """Custom row count is respected."""
        result = _make_dummy_bars(DataFrameLibrary.Pandas, n=10)
        assert len(result) == 10


# ─────────────────────────────────────────────────────────────────────────────
# _ts_to_datetime
# ─────────────────────────────────────────────────────────────────────────────


class TestTsToDatetime:
    """Tests for the _ts_to_datetime helper."""

    def test_conversion(self):
        """Convert a known timestamp to datetime."""
        series = pd.Series([0, 86400])
        result = _ts_to_datetime(series, ZoneInfo("UTC"))
        assert result.iloc[0] == pd.Timestamp("1970-01-01", tz="UTC")
        assert result.iloc[1] == pd.Timestamp("1970-01-02", tz="UTC")


# ─────────────────────────────────────────────────────────────────────────────
# _to_pandas
# ─────────────────────────────────────────────────────────────────────────────


class TestToPandasInUtils:
    """Tests for the _to_pandas helper in utils."""

    def test_dict_input(self):
        """Dict is converted to DataFrame."""
        result = _to_pandas({"a": [1, 2], "b": [3, 4]})
        assert isinstance(result, pd.DataFrame)
        assert list(result.columns) == ["a", "b"]


# ─────────────────────────────────────────────────────────────────────────────
# Constants
# ─────────────────────────────────────────────────────────────────────────────


class TestConstants:
    """Tests for regex patterns and constants."""

    @pytest.mark.parametrize("valid", ["hello", "my-tag", "test_123", "a b c"])
    def test_tag_pattern_valid(self, valid):
        """TAG_PATTERN matches valid tags."""
        assert TAG_PATTERN.fullmatch(valid) is not None

    @pytest.mark.parametrize("invalid", ["bad<tag", "too" * 10, ""])
    def test_tag_pattern_invalid(self, invalid):
        """TAG_PATTERN rejects invalid tags."""
        assert TAG_PATTERN.fullmatch(invalid) is None

    @pytest.mark.parametrize("char", ["<", ">", ":", '"', "|", "?", "*"])
    def test_invalid_filename_chars(self, char):
        """INVALID_FILENAME_CHARS matches forbidden characters."""
        assert INVALID_FILENAME_CHARS.search(char) is not None

    def test_valid_filename(self):
        """INVALID_FILENAME_CHARS does not match a valid filename."""
        assert INVALID_FILENAME_CHARS.search("my-experiment_v2") is None

    def test_moment_to_strftime_keys(self):
        """MOMENT_TO_STRFTIME contains expected keys."""
        assert "YYYY" in MOMENT_TO_STRFTIME
        assert "MM" in MOMENT_TO_STRFTIME
        assert "DD" in MOMENT_TO_STRFTIME
        assert "HH" in MOMENT_TO_STRFTIME


# ─────────────────────────────────────────────────────────────────────────────
# CaseInsensitiveEnum
# ─────────────────────────────────────────────────────────────────────────────


class TestCaseInsensitiveEnum:
    """Tests for the CaseInsensitiveEnum base class."""

    class _Color(CaseInsensitiveEnum):
        Red = 1
        Green = 2
        Blue = 3

    def test_case_insensitive(self):
        """Case-insensitive lookup works for all casings."""
        assert self._Color("red") == self._Color.Red
        assert self._Color("RED") == self._Color.Red
        assert self._Color("Red") == self._Color.Red

    def test_repr(self):
        """Repr returns the member name."""
        assert repr(self._Color.Red) == "Red"

    def test_missing_raises(self):
        """Unknown member raises ValueError."""
        with pytest.raises(ValueError, match="has no member"):
            self._Color("yellow")


# ─────────────────────────────────────────────────────────────────────────────
# init_logging / clear_cache
# ─────────────────────────────────────────────────────────────────────────────


class TestCoreUtils:
    """Tests for core utils functions."""

    def test_init_logging_idempotent(self):
        """init_logging can be called multiple times without error."""
        init_logging("warn")
        init_logging("warn")  # second call is a no-op

    def test_clear_cache(self):
        """clear_cache runs without error."""
        clear_cache()
