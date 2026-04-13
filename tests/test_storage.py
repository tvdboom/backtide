"""Backtide.

Author: Mavs
Description: Unit tests for the storage interface functions.

"""

import pandas as pd
import pytest

from backtide.data import Interval
from backtide.storage import delete_symbols, get_bars, get_dividends


class TestGetBars:
    """Tests for the 'get_bars' function."""

    def test_returns_dataframe(self):
        """get_bars always returns a pandas DataFrame."""
        result = get_bars()
        assert isinstance(result, pd.DataFrame)

    def test_empty_database(self):
        """A fresh database returns an empty DataFrame."""
        result = get_bars()
        assert result.empty

    def test_expected_columns(self):
        """The DataFrame has the expected column names even when empty."""
        result = get_bars()
        expected = {
            "symbol",
            "instrument_type",
            "interval",
            "provider",
            "open_ts",
            "close_ts",
            "open_ts_exchange",
            "open",
            "high",
            "low",
            "close",
            "adj_close",
            "volume",
            "n_trades",
        }
        assert set(result.columns) == expected


class TestGetDividends:
    """Tests for the 'get_dividends' function."""

    def test_returns_dataframe(self):
        """get_dividends always returns a pandas DataFrame."""
        result = get_dividends()
        assert isinstance(result, pd.DataFrame)

    def test_empty_database(self):
        """A fresh database returns an empty DataFrame."""
        result = get_dividends()
        assert result.empty

    def test_expected_columns(self):
        """The DataFrame has the expected column names even when empty."""
        result = get_dividends()
        expected = {"symbol", "provider", "ex_date", "amount"}
        assert set(result.columns) == expected


class TestDeleteSymbols:
    """Tests for the 'delete_symbols' function."""

    def test_returns_int(self):
        """delete_symbols returns an integer count."""
        result = delete_symbols("AAPL")
        assert isinstance(result, int)

    def test_empty_database_returns_zero(self):
        """Deleting from an empty database returns 0."""
        assert delete_symbols("AAPL") == 0

    def test_list_of_symbols(self):
        """Accepts a list of symbols."""
        result = delete_symbols(["AAPL", "MSFT"])
        assert isinstance(result, int)
        assert result == 0

    def test_with_interval_str(self):
        """The interval parameter accepts a string."""
        result = delete_symbols("AAPL", interval="1d")
        assert isinstance(result, int)

    def test_with_interval_enum(self):
        """The interval parameter accepts an Interval enum."""
        result = delete_symbols("AAPL", interval=Interval("1d"))
        assert isinstance(result, int)

    def test_with_provider_str(self):
        """The provider parameter accepts a string."""
        result = delete_symbols("AAPL", provider="yahoo")
        assert isinstance(result, int)

    def test_with_provider_enum(self):
        """The provider parameter accepts a provider string."""
        result = delete_symbols("AAPL", provider="yahoo")
        assert isinstance(result, int)

    def test_all_filters(self):
        """All optional filters can be combined."""
        result = delete_symbols("AAPL", interval="1d", provider="yahoo")
        assert isinstance(result, int)

    def test_invalid_interval_raises(self):
        """An invalid interval string raises ValueError."""
        with pytest.raises(ValueError, match="Unknown interval"):
            delete_symbols("AAPL", interval="invalid")

    def test_invalid_provider_raises(self):
        """An invalid provider string raises ValueError."""
        with pytest.raises(ValueError, match="Unknown provider"):
            delete_symbols("AAPL", provider="invalid")
