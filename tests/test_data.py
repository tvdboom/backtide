"""Backtide.

Author: Mavs
Description: Unit tests for the data interface functions.

"""

import pytest

from backtide.data import (
    Exchange,
    Instrument,
    InstrumentProfile,
    InstrumentType,
    Interval,
    get_instruments,
    list_instruments,
    resolve_profiles,
)


class TestGetInstruments:
    """Tests for the 'get_instruments' function."""

    def test_single_str(self):
        """A single string symbol returns a list of Instrument."""
        result = get_instruments("AAPL", "stocks")
        assert isinstance(result, list)
        assert len(result) == 1
        assert all(isinstance(i, Instrument) for i in result)

    def test_list_of_str(self):
        """A list of symbols returns one Instrument per symbol."""
        result = get_instruments(["AAPL", "MSFT"], "stocks")
        assert isinstance(result, list)
        assert len(result) == 2
        assert all(isinstance(i, Instrument) for i in result)

    def test_instrument_type_as_enum(self):
        """Passing InstrumentType enum works the same as a string."""
        result = get_instruments("AAPL", InstrumentType("stocks"))
        assert len(result) == 1
        assert isinstance(result[0], Instrument)

    def test_instrument_as_input(self):
        """An Instrument object can be used in place of a string symbol."""
        instruments = get_instruments("AAPL", "stocks")
        result = get_instruments(instruments[0], "stocks")
        assert len(result) == 1
        assert result[0].symbol == "AAPL"

    def test_list_of_instruments_as_input(self):
        """A list of Instrument objects works as input."""
        instruments = get_instruments(["AAPL", "MSFT"], "stocks")
        result = get_instruments(instruments, "stocks")
        assert len(result) == 2

    def test_crypto_symbol(self):
        """Crypto symbols resolve correctly."""
        result = get_instruments("BTC-USD", "crypto")
        assert len(result) == 1
        assert result[0].symbol == "BTC-USD"

    def test_invalid_instrument_type_raises(self):
        """An invalid instrument_type string raises ValueError."""
        with pytest.raises(ValueError, match="Unknown instrument_type"):
            get_instruments("AAPL", "invalid_type")


class TestResolveProfiles:
    """Tests for the 'resolve_profiles' function."""

    def test_single_symbol_single_interval(self):
        """Single symbol and interval returns list of InstrumentProfile."""
        result = resolve_profiles("AAPL", "stocks", "1d")
        assert isinstance(result, list)
        assert all(isinstance(p, InstrumentProfile) for p in result)
        assert len(result) >= 1  # At least the symbol itself

    def test_list_of_symbols(self):
        """Multiple symbols return profiles for each."""
        result = resolve_profiles(["AAPL", "MSFT"], "stocks", "1d")
        assert isinstance(result, list)
        assert len(result) >= 2

    def test_list_of_intervals(self):
        """Multiple intervals are resolved into the profile(s)."""
        result = resolve_profiles("AAPL", "stocks", ["1d", "1w"])
        assert isinstance(result, list)
        assert len(result) >= 1  # Deduplicated; intervals are inside the profile

    def test_enum_instrument_type(self):
        """InstrumentType enum works."""
        result = resolve_profiles("AAPL", InstrumentType("stocks"), "1d")
        assert len(result) >= 1

    def test_enum_interval(self):
        """Interval enum works."""
        result = resolve_profiles("AAPL", "stocks", Interval("1d"))
        assert len(result) >= 1

    def test_instrument_as_symbol(self):
        """Instrument object can be used as symbol input."""
        instruments = get_instruments("AAPL", "stocks")
        result = resolve_profiles(instruments[0], "stocks", "1d")
        assert len(result) >= 1


class TestListInstruments:
    """Tests for the 'list_instruments' function."""

    def test_str_instrument_type(self):
        """String instrument type returns a list of Instrument."""
        result = list_instruments("stocks")
        assert isinstance(result, list)
        assert all(isinstance(i, Instrument) for i in result)
        assert len(result) <= 100  # Default limit

    def test_enum_instrument_type(self):
        """InstrumentType enum works."""
        result = list_instruments(InstrumentType("stocks"))
        assert isinstance(result, list)
        assert len(result) <= 100

    def test_limit(self):
        """The limit parameter caps the number of results."""
        result = list_instruments("stocks", limit=5)
        assert len(result) <= 5

    def test_exchange_as_str(self):
        """Exchange filter as string works."""
        result = list_instruments("stocks", exchange="XNYS")
        assert isinstance(result, list)
        assert len(result) <= 100

    def test_exchange_as_enum(self):
        """Exchange filter as enum works."""
        result = list_instruments("stocks", exchange=Exchange("XNYS"))
        assert isinstance(result, list)

    def test_exchange_as_list(self):
        """A list of exchanges works."""
        result = list_instruments("stocks", exchange=["XNYS", "XNAS"])
        assert isinstance(result, list)

    def test_crypto(self):
        """Listing crypto instruments works."""
        result = list_instruments("crypto", limit=5)
        assert isinstance(result, list)
        assert len(result) >= 1  # Provider may return more than limit

    def test_invalid_instrument_type_raises(self):
        """An invalid instrument type string raises ValueError."""
        with pytest.raises(ValueError, match="Unknown instrument_type"):
            list_instruments("invalid_type")
