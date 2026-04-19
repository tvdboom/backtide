"""Backtide.

Author: Mavs
Description: Unit tests for the storage interface functions.

"""

import pandas as pd
import pytest

from backtide.storage import (
    delete_symbols,
    query_bars,
    query_bars_summary,
    query_dividends,
    query_instruments,
)


# ─────────────────────────────────────────────────────────────────────────────
# query_bars
# ─────────────────────────────────────────────────────────────────────────────


class TestQueryBars:
    """Tests for the 'query_bars' function."""

    def test_returns_dataframe(self):
        """query_bars always returns a pandas DataFrame."""
        assert isinstance(query_bars(), pd.DataFrame)

    def test_empty_database(self):
        """A fresh database returns an empty DataFrame."""
        result = query_bars()
        assert result.empty

    def test_expected_columns(self):
        """The DataFrame has the expected column names even when empty."""
        expected = {
            "symbol",
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
        assert set(query_bars().columns) == expected


# ─────────────────────────────────────────────────────────────────────────────
# query_dividends
# ─────────────────────────────────────────────────────────────────────────────


class TestQueryDividends:
    """Tests for the 'query_dividends' function."""

    def test_returns_dataframe(self):
        """query_dividends always returns a pandas DataFrame."""
        assert isinstance(query_dividends(), pd.DataFrame)

    def test_empty_database(self):
        """A fresh database returns an empty DataFrame."""
        assert query_dividends().empty

    def test_expected_columns(self):
        """The DataFrame has the expected column names even when empty."""
        expected = {"symbol", "provider", "ex_date", "amount"}
        assert set(query_dividends().columns) == expected


# ─────────────────────────────────────────────────────────────────────────────
# query_bars_summary
# ─────────────────────────────────────────────────────────────────────────────


class TestQueryBarsSummary:
    """Tests for the 'query_bars_summary' function."""

    def test_returns_dataframe(self):
        """query_bars_summary returns a pandas DataFrame."""
        assert isinstance(query_bars_summary(), pd.DataFrame)

    def test_empty_database(self):
        """A fresh database returns an empty DataFrame."""
        assert query_bars_summary().empty


# ─────────────────────────────────────────────────────────────────────────────
# query_instruments
# ─────────────────────────────────────────────────────────────────────────────


class TestQueryInstruments:
    """Tests for the 'query_instruments' function."""

    def test_returns_list(self):
        """query_instruments returns a list."""
        assert isinstance(query_instruments(), list)


# ─────────────────────────────────────────────────────────────────────────────
# delete_symbols
# ─────────────────────────────────────────────────────────────────────────────


class TestDeleteSymbols:
    """Tests for the 'delete_symbols' function."""

    def test_returns_int(self):
        """delete_symbols returns an integer count."""
        assert isinstance(delete_symbols("AAPL"), int)

    def test_empty_database_returns_zero(self):
        """Deleting from an empty database returns 0."""
        assert delete_symbols("AAPL") == 0

    def test_with_interval(self):
        """The interval parameter is accepted."""
        assert isinstance(delete_symbols("AAPL", interval="1d"), int)

    def test_with_provider(self):
        """The provider parameter is accepted."""
        assert isinstance(delete_symbols("AAPL", provider="yahoo"), int)

    def test_all_filters(self):
        """All optional filters can be combined."""
        assert isinstance(delete_symbols("AAPL", interval="1d", provider="yahoo"), int)
