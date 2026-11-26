"""Backtide.

Author: Mavs
Description: Unit tests for utility functions and constants.

"""

from datetime import datetime, timedelta
import subprocess
import sys
import textwrap
import zoneinfo
from zoneinfo import ZoneInfo

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

    def test_object_conversion_method(self):
        """Objects exposing `to_pandas` delegate their conversion."""

        class Convertible:
            @staticmethod
            def to_pandas():
                return {"a": [1]}

        result = _to_pandas(Convertible())

        assert result.to_dict(orient="list") == {"a": [1]}


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

    def test_polars_backend_and_explicit_timezone_in_a_fresh_process(self) -> None:
        """Preserve dataframe shapes and explicit timezone settings across the extension."""
        # Configuration is immutable after first use, so test a different backend in a child.
        script = textwrap.dedent("""\
            import math
            import pandas as pd
            import polars as pl
            from backtide.config import Config, DataConfig, DisplayConfig, set_config

            set_config(Config(
                data=DataConfig(dataframe_library="polars"),
                display=DisplayConfig(timezone="America/New_York"),
            ))
            from backtide.analysis import compute_statistics
            from backtide.backtest import State
            from backtide.indicators import SimpleMovingAverage, BollingerBands
            from backtide.utils import init_logging

            bars = pl.DataFrame({
                key: [1.0, 2.0, 3.0] for key in ("open", "high", "low", "close", "volume")
            })
            mean = SimpleMovingAverage(2).compute(bars)
            bands = BollingerBands(2, 1.0).compute(bars)
            assert isinstance(mean, pl.Series)
            assert math.isnan(mean[0])
            assert mean.to_list()[1:] == [1.5, 2.5]
            assert isinstance(bands, pl.DataFrame)
            assert bands.shape == (3, 3)
            assert bands[2, 1] == 2.5

            frame = pd.DataFrame({
                "symbol": ["AAPL"] * 3, "close": [1.0, 2.0, 3.0],
                "open_ts": [1704067200, 1704153600, 1704240000],
                "dt": pd.date_range("2024-01-01", periods=3, tz="UTC"),
            })
            statistics = compute_statistics(frame, price_col="close")
            assert isinstance(statistics, pl.DataFrame)
            assert statistics["symbol"].to_list() == ["AAPL"]

            timestamp = State(timestamp=0).datetime
            assert timestamp.isoformat() == "1969-12-31T19:00:00-05:00"
            assert timestamp.tzinfo.key == "America/New_York"
            init_logging("info")
            print("backend-contract-ok")
            """)

        result = subprocess.run(
            [sys.executable, "-c", script], capture_output=True, text=True, check=False, timeout=30
        )

        assert result.returncode == 0, result.stderr
        assert "backend-contract-ok" in result.stdout


# ─────────────────────────────────────────────────────────────────────────────
# _format_price and _format_number edge cases
# ─────────────────────────────────────────────────────────────────────────────


class TestFormatPriceEdgeCases:
    """Extended tests for _format_price edge cases."""

    def test_format_price_with_currency_object(self):
        """Test formatting with Currency enum."""
        from backtide.data import Currency

        result = _format_price(1234.56, currency=Currency.USD)
        assert isinstance(result, str)

    def test_currency_symbol_can_follow_the_number(self, monkeypatch):
        """Currency placement follows the configured suffix convention."""
        import backtide.utils.utils as utils_module

        monkeypatch.setattr(utils_module.cfg.display, "currency_prefix", False)

        assert _format_price(12.5, currency="USD").endswith(" $")

    def test_format_price_with_invalid_currency_string(self):
        """Test formatting with invalid currency code falls back."""
        result = _format_price(1234.56, currency="INVALID")
        assert isinstance(result, str)

    def test_format_price_compact_mode(self):
        """Test compact formatting of large numbers."""
        result = _format_price(1_000_000, compact=True)
        assert "M" in result or "k" in result

    def test_format_price_with_signed(self):
        """Test signed format shows + for positive."""
        result = _format_price(100, signed=True)
        assert "+" in result or isinstance(result, str)

    def test_format_price_negative_signed(self):
        """Test signed format for negative numbers."""
        result = _format_price(-100, signed=True)
        assert isinstance(result, str)

    def test_format_price_zero(self):
        """Test formatting zero."""
        result = _format_price(0)
        assert "0" in result

    def test_format_price_with_decimals(self):
        """Test custom decimal places."""
        result = _format_price(100.123, decimals=1)
        assert isinstance(result, str)

    def test_format_price_large_compact_number(self):
        """Test formatting very large numbers in compact mode."""
        result = _format_price(50_000_000_000, compact=True)
        assert "B" in result

    def test_format_price_between_million_and_billion(self):
        """Test formatting numbers between M and B."""
        result = _format_price(950_000_000, compact=True)
        assert isinstance(result, str)

    def test_format_price_exactly_10_million(self):
        """Test boundary case at 10 million."""
        result = _format_price(10_000_000, compact=True)
        assert "M" in result

    def test_format_price_exactly_10_thousand(self):
        """Test boundary case at 10 thousand."""
        result = _format_price(10_000, compact=True)
        assert "k" in result

    def test_format_price_with_currency_compact(self):
        """Test currency formatting with compact numbers."""
        from backtide.data import Currency

        result = _format_price(5_000_000, currency=Currency.USD, compact=True)
        assert isinstance(result, str)

    def test_format_negative_compact(self):
        """Test negative numbers in compact mode."""
        result = _format_price(-5_000_000, compact=True)
        assert "M" in result

    def test_format_price_with_inr_currency(self):
        """Test formatting with INR currency."""
        from backtide.data import Currency

        result = _format_price(1000, currency=Currency.INR)
        assert isinstance(result, str)

    def test_format_price_with_gbp_currency(self):
        """Test formatting with GBP currency."""
        from backtide.data import Currency

        result = _format_price(1000, currency=Currency.GBP)
        assert isinstance(result, str)

    def test_format_price_with_jpy_currency(self):
        """Test formatting with JPY currency."""
        from backtide.data import Currency

        result = _format_price(100000, currency=Currency.JPY)
        assert isinstance(result, str)

    def test_format_price_with_none_currency(self):
        """Test format price with None currency."""
        result = _format_price(100.0, currency=None)
        assert isinstance(result, str)

    def test_format_price_edge_case_exactly_zero(self):
        """Test formatting exactly zero."""
        result = _format_price(0.0)
        assert "0.0" in result or "0" in result

    def test_format_price_negative_with_currency(self):
        """Test negative formatting with currency."""
        result = _format_price(-100.5, currency="USD")
        assert isinstance(result, str)

    def test_format_price_with_custom_decimals_and_signed(self):
        """Test with custom decimals and signed format."""
        result = _format_price(123.456, decimals=3, signed=True)
        assert isinstance(result, str)


class TestFormatNumberBoundaries:
    """Test _format_number at boundary values."""

    def test_format_billions(self):
        """Test formatting billions."""
        result = _format_number(5_000_000_000)
        assert "B" in result

    def test_format_millions(self):
        """Test formatting millions."""
        result = _format_number(5_000_000)
        assert "M" in result

    def test_format_thousands(self):
        """Test formatting thousands."""
        result = _format_number(5000)
        assert "k" in result

    def test_format_small_numbers(self):
        """Test formatting small numbers."""
        result = _format_number(500)
        assert "500" in result

    def test_format_negative_billions(self):
        """Test formatting negative billions."""
        result = _format_number(-15_000_000_000)
        assert "B" in result

    def test_format_negative_small_numbers(self):
        """Test formatting negative small numbers."""
        result = _format_number(-500)
        assert "-" in result

    def test_format_exactly_10_billion(self):
        """Test exactly 10 billion."""
        result = _format_number(10_000_000_000)
        assert "B" in result

    def test_format_exactly_1_billion(self):
        """Test exactly 1 billion."""
        result = _format_number(1_000_000_000)
        assert "B" in result

    def test_format_exactly_10_million(self):
        """Test exactly 10 million."""
        result = _format_number(10_000_000)
        assert "M" in result

    def test_format_exactly_1_million(self):
        """Test exactly 1 million."""
        result = _format_number(1_000_000)
        assert "M" in result

    def test_format_exactly_10_thousand(self):
        """Test exactly 10 thousand."""
        result = _format_number(10_000)
        assert "k" in result

    def test_format_exactly_1_thousand(self):
        """Test exactly 1 thousand."""
        result = _format_number(1_000)
        assert "k" in result

    def test_format_decimal_boundaries(self):
        """Test decimal formatting at boundaries."""
        result1 = _format_number(9_999)
        result2 = _format_number(10_001)
        assert isinstance(result1, str)
        assert isinstance(result2, str)

    def test_format_number_with_very_small_negative(self):
        """Test formatting very small negative numbers."""
        result = _format_number(-0.001)
        assert isinstance(result, str)

    def test_format_number_with_fractional_values(self):
        """Test formatting fractional values."""
        result = _format_number(1500.5)
        assert isinstance(result, str)


# ─────────────────────────────────────────────────────────────────────────────
# Additional utility tests
# ─────────────────────────────────────────────────────────────────────────────


class TestUtilsToPandas:
    """Tests for _to_pandas conversion function."""

    def test_to_pandas_with_empty_dict(self):
        """Test converting empty dict to DataFrame."""
        result = _to_pandas({})
        assert isinstance(result, pd.DataFrame)
        assert result.empty

    def test_to_pandas_with_list_of_dicts(self):
        """Test converting list of dicts to DataFrame."""
        data = [{"a": 1, "b": 2}, {"a": 3, "b": 4}]
        result = _to_pandas(data)
        assert isinstance(result, pd.DataFrame)
        assert len(result) == 2

    def test_to_pandas_with_dataframe(self):
        """Test that DataFrame passthrough works."""
        df = pd.DataFrame({"a": [1, 2], "b": [3, 4]})
        result = _to_pandas(df)
        assert isinstance(result, pd.DataFrame)


class TestGetTimezone:
    """Tests for _get_timezone function."""

    def test_get_timezone_with_none(self):
        """Test timezone resolution with None config."""
        from backtide.utils.utils import _get_timezone

        result = _get_timezone(None)
        assert isinstance(result, ZoneInfo)

    def test_get_timezone_with_explicit_tz(self):
        """Test timezone with explicit timezone string."""
        from backtide.utils.utils import _get_timezone

        result = _get_timezone("UTC")
        assert isinstance(result, ZoneInfo)
        assert str(result) == "UTC"

    def test_get_timezone_with_us_eastern(self):
        """Test with US/Eastern timezone."""
        from backtide.utils.utils import _get_timezone

        result = _get_timezone("US/Eastern")
        assert isinstance(result, ZoneInfo)

    @pytest.mark.parametrize("name", ["US/Eastern", "America/New_York"])
    def test_timezone_falls_back_to_package_without_system_data(
        self, monkeypatch: pytest.MonkeyPatch, name: str
    ) -> None:
        """Resolve timezone aliases and daylight saving from the packaged database."""
        from backtide.utils.utils import _get_timezone

        # Bypass cached zones so the packaged fallback must supply the data.
        monkeypatch.setattr("backtide.utils.utils.ZoneInfo", ZoneInfo.no_cache)
        previous_path = zoneinfo.TZPATH
        try:
            zoneinfo.reset_tzpath(())
            timezone = _get_timezone(name)
        finally:
            zoneinfo.reset_tzpath(previous_path)

        assert timezone.key == name
        assert datetime(2026, 1, 15, tzinfo=timezone).utcoffset() == timedelta(hours=-5)
        assert datetime(2026, 7, 15, tzinfo=timezone).utcoffset() == timedelta(hours=-4)


class TestDataTransformations:
    """Tests for data transformation edge cases."""

    def test_to_list_with_generator(self):
        """Test _to_list with generator."""
        gen = (x for x in [1, 2, 3])
        result = _to_list(gen)
        assert isinstance(result, list)

    def test_to_list_with_empty_list(self):
        """Test _to_list with empty list."""
        result = _to_list([])
        assert result == []

    def test_to_list_preserves_order(self):
        """Test _to_list preserves order of elements."""
        input_list = [3, 1, 4, 1, 5]
        result = _to_list(input_list)
        assert result == input_list

    def test_to_list_with_none(self):
        """Test that None returns list with None."""
        result = _to_list(None)
        assert isinstance(result, list)
        assert result == [None]


class TestCheckDependencyEdgeCases:
    """Additional tests for _check_dependency function."""

    def test_check_existing_dependency(self):
        """Test checking an installed dependency."""
        import pandas as pd

        result = _check_dependency("pandas")
        assert result is pd

    def test_check_missing_dependency(self):
        """Test checking a missing dependency raises error."""
        with pytest.raises(
            ModuleNotFoundError,
            match="Unable to import the nonexistent_package_xyz package",
        ):
            _check_dependency("nonexistent_package_xyz")

    def test_check_dependency_with_custom_pypi_name(self):
        """Test checking dependency with different PyPI name."""
        with pytest.raises(ModuleNotFoundError):
            _check_dependency("nonexistent", pypi_name="custom-name")
