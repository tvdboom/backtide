"""Backtide.

Author: Mavs
Description: Unit tests for the data interface and model classes.

"""

import pytest

from backtide.data import (
    Country,
    Currency,
    Exchange,
    Instrument,
    InstrumentProfile,
    InstrumentType,
    Interval,
    Provider,
    fetch_instruments,
    list_instruments,
    resolve_profiles,
)

# ─────────────────────────────────────────────────────────────────────────────
# InstrumentType
# ─────────────────────────────────────────────────────────────────────────────


class TestInstrumentType:
    """Tests for the 'InstrumentType' enum."""

    def test_variants(self):
        """variants() returns all instrument types."""
        v = InstrumentType.variants()
        assert len(v) == 4

    def test_get_default(self):
        """get_default returns stocks."""
        assert str(InstrumentType.get_default()) == "Stocks"

    @pytest.mark.parametrize(
        ("name", "expected"),
        [("stocks", "Stocks"), ("etf", "ETF"), ("forex", "Forex"), ("crypto", "Crypto")],
    )
    def test_from_string(self, name, expected):
        """InstrumentType can be constructed from lowercase string."""
        assert str(InstrumentType(name)) == expected

    @pytest.mark.parametrize("name", ["stocks", "etf"])
    def test_is_equity_true(self, name):
        """Stocks and ETF are equity types."""
        assert InstrumentType(name).is_equity is True

    @pytest.mark.parametrize("name", ["forex", "crypto"])
    def test_is_equity_false(self, name):
        """Forex and Crypto are not equity types."""
        assert InstrumentType(name).is_equity is False

    def test_icon(self):
        """icon() returns a material icon string."""
        assert InstrumentType("stocks").icon().startswith(":material/")

    def test_invalid_raises(self):
        """An invalid instrument_type string raises ValueError."""
        with pytest.raises(ValueError, match="Unknown instrument"):
            InstrumentType("invalid")


# ─────────────────────────────────────────────────────────────────────────────
# Interval
# ─────────────────────────────────────────────────────────────────────────────


class TestInterval:
    """Tests for the 'Interval' enum."""

    def test_variants(self):
        """variants() returns all intervals."""
        v = Interval.variants()
        assert len(v) == 8

    def test_get_default(self):
        """get_default returns 1d."""
        assert str(Interval.get_default()) == "1d"

    @pytest.mark.parametrize("name", ["1m", "5m", "15m", "30m", "1h", "4h", "1d", "1w"])
    def test_from_string(self, name):
        """Interval can be constructed from canonical string."""
        assert str(Interval(name)) == name

    @pytest.mark.parametrize(("name", "expected"), [("1m", True), ("1h", True), ("1d", False)])
    def test_is_intraday(self, name, expected):
        """is_intraday returns correct result."""
        assert Interval(name).is_intraday() == expected

    @pytest.mark.parametrize(("name", "expected"), [("1m", 1), ("1h", 60), ("1d", 1440)])
    def test_minutes(self, name, expected):
        """minutes() returns expected values."""
        assert Interval(name).minutes() == expected

    def test_invalid_raises(self):
        """An invalid interval string raises ValueError."""
        with pytest.raises(ValueError, match="Unknown interval"):
            Interval("invalid")


# ─────────────────────────────────────────────────────────────────────────────
# Provider
# ─────────────────────────────────────────────────────────────────────────────


class TestProvider:
    """Tests for the 'Provider' enum."""

    @pytest.mark.parametrize(
        ("attr", "expected"),
        [
            ("Yahoo", "yahoo"),
            ("Binance", "binance"),
            ("Coinbase", "coinbase"),
            ("Kraken", "kraken"),
        ],
    )
    def test_from_attr(self, attr, expected):
        """Provider can be accessed via class attributes."""
        assert repr(getattr(Provider, attr)) == expected

    def test_intervals(self):
        """intervals() returns a non-empty list of Interval."""
        ivs = Provider.Yahoo.intervals()
        assert len(ivs) > 0
        assert all(isinstance(i, Interval) for i in ivs)

    def test_coinbase_excludes_weekly(self):
        """Coinbase does not support the weekly interval."""
        ivs = Provider.Coinbase.intervals()
        assert Interval("1w") not in ivs

    def test_construct_from_string(self):
        """Provider can be constructed from string."""
        p = Provider("yahoo")
        assert repr(p) == "yahoo"


# ─────────────────────────────────────────────────────────────────────────────
# Country / Currency / Exchange
# ─────────────────────────────────────────────────────────────────────────────


class TestCountry:
    """Tests for the 'Country' enum."""

    def test_variants(self):
        """variants() returns a non-empty list."""
        assert len(Country.variants()) > 0

    def test_attributes(self):
        """Country has alpha2, alpha3, name and flag attributes."""
        c = Country("USA")
        assert c.alpha2 == "US"
        assert c.alpha3 == "USA"
        assert isinstance(c.name, str)
        assert len(c.flag) > 0


class TestCurrency:
    """Tests for the 'Currency' enum."""

    def test_variants(self):
        """variants() returns a non-empty list."""
        assert len(Currency.variants()) > 0

    def test_get_default(self):
        """get_default returns USD."""
        assert str(Currency.get_default()) == "USD"

    def test_attributes(self):
        """Currency has name, symbol, country, decimals and symbol_prefix."""
        c = Currency("USD")
        assert isinstance(c.name, str)
        assert isinstance(c.symbol, str)
        assert isinstance(c.decimals, int)
        assert isinstance(c.symbol_prefix, bool)

    def test_format(self):
        """format() returns a formatted string."""
        result = Currency("USD").format(1234.56)
        assert isinstance(result, str)
        assert len(result) > 0


class TestExchange:
    """Tests for the 'Exchange' enum."""

    def test_variants(self):
        """variants() returns a non-empty list."""
        assert len(Exchange.variants()) > 0

    def test_attributes(self):
        """Exchange has mic, name, country, city, currency and yahoo_code."""
        e = Exchange("XNYS")
        assert e.mic == "XNYS"
        assert isinstance(e.name, str)
        assert isinstance(e.city, str)
        assert isinstance(e.country, Country)
        assert isinstance(e.currency, Currency)


# ─────────────────────────────────────────────────────────────────────────────
# Instrument / InstrumentProfile
# ─────────────────────────────────────────────────────────────────────────────


class TestInstrument:
    """Tests for the 'Instrument' class."""

    def test_construction(self, sample_instrument):
        """Instrument can be constructed with all fields."""
        assert sample_instrument.symbol == "AAPL"
        assert sample_instrument.name == "Apple Inc."
        assert sample_instrument.instrument_type == InstrumentType("stocks")

    def test_repr(self, sample_instrument):
        """__repr__ contains the symbol."""
        assert "AAPL" in repr(sample_instrument)

    def test_base_none_for_stocks(self, sample_instrument):
        """Stocks have base=None."""
        assert sample_instrument.base is None

    def test_base_set_for_crypto(self, sample_instrument_crypto):
        """Crypto instruments have a base currency."""
        assert sample_instrument_crypto.base is not None


class TestInstrumentProfile:
    """Tests for the 'InstrumentProfile' class."""

    def test_construction(self, sample_profile):
        """InstrumentProfile wraps an instrument with metadata."""
        assert sample_profile.symbol == "AAPL"
        assert isinstance(sample_profile.earliest_ts, dict)
        assert isinstance(sample_profile.latest_ts, dict)
        assert isinstance(sample_profile.legs, list)

    def test_delegated_getters(self, sample_profile):
        """Profile delegates symbol/name/instrument_type to instrument."""
        assert sample_profile.name == "Apple Inc."
        assert sample_profile.instrument_type == InstrumentType("stocks")

    def test_repr(self, sample_profile):
        """__repr__ contains InstrumentProfile."""
        assert "InstrumentProfile" in repr(sample_profile)


# ─────────────────────────────────────────────────────────────────────────────
# fetch_instruments
# ─────────────────────────────────────────────────────────────────────────────


class TestFetchInstruments:
    """Tests for the 'fetch_instruments' function."""

    def test_single_str(self):
        """A single string symbol returns a list of Instrument."""
        result = fetch_instruments("AAPL", "stocks")
        assert isinstance(result, list)
        assert len(result) == 1
        assert all(isinstance(i, Instrument) for i in result)

    def test_list_of_str(self):
        """A list of symbols returns one Instrument per symbol."""
        result = fetch_instruments(["AAPL", "MSFT"], "stocks")
        assert isinstance(result, list)
        assert len(result) == 2
        assert all(isinstance(i, Instrument) for i in result)

    def test_instrument_type_as_enum(self):
        """Passing InstrumentType enum works the same as a string."""
        result = fetch_instruments("AAPL", InstrumentType("stocks"))
        assert len(result) == 1
        assert isinstance(result[0], Instrument)

    def test_instrument_as_input(self):
        """An Instrument object can be used in place of a string symbol."""
        instruments = fetch_instruments("AAPL", "stocks")
        result = fetch_instruments(instruments[0], "stocks")
        assert len(result) == 1
        assert result[0].symbol == "AAPL"

    def test_list_of_instruments_as_input(self):
        """A list of Instrument objects works as input."""
        instruments = fetch_instruments(["AAPL", "MSFT"], "stocks")
        result = fetch_instruments(instruments, "stocks")
        assert len(result) == 2

    def test_crypto_symbol(self):
        """Crypto symbols resolve correctly."""
        result = fetch_instruments("BTC-USD", "crypto")
        assert len(result) == 1
        assert result[0].symbol == "BTC-USD"

    def test_invalid_instrument_type_raises(self):
        """An invalid instrument_type string raises ValueError."""
        with pytest.raises(ValueError, match="Unknown instrument_type"):
            fetch_instruments("AAPL", "invalid_type")


# ─────────────────────────────────────────────────────────────────────────────
# resolve_profiles
# ─────────────────────────────────────────────────────────────────────────────


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
        instruments = fetch_instruments("AAPL", "stocks")
        result = resolve_profiles(instruments[0], "stocks", "1d")
        assert len(result) >= 1


# ─────────────────────────────────────────────────────────────────────────────
# list_instruments
# ─────────────────────────────────────────────────────────────────────────────


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
