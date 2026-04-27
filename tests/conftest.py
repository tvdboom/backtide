"""Backtide.

Author: Mavs
Description: Shared fixtures for the test suite.

"""

import os
import tempfile

import pytest

from backtide.config import Config, DataConfig, set_config
from backtide.data import (
    Instrument,
    InstrumentProfile,
    Interval,
)

# Set a deterministic config.
set_config(
    Config(
        data=DataConfig(
            storage_path=tempfile.mkdtemp(prefix="backtide_test_storage_"),
            providers={"crypto": "yahoo"},
        ),
    )
)


# ─────────────────────────────────────────────────────────────────────────────
# Reusable model fixtures
# ─────────────────────────────────────────────────────────────────────────────


@pytest.fixture
def sample_instrument():
    """Return a minimal stock Instrument for testing."""
    return Instrument(
        symbol="AAPL",
        name="Apple Inc.",
        base=None,
        quote="USD",
        instrument_type="stocks",
        exchange="XNAS",
        provider="yahoo",
    )


@pytest.fixture
def sample_instrument_crypto():
    """Return a minimal crypto Instrument for testing."""
    return Instrument(
        symbol="BTC-USD",
        name="Bitcoin USD",
        base="BTC",
        quote="USD",
        instrument_type="crypto",
        exchange="crypto",
        provider="yahoo",
    )


@pytest.fixture
def sample_profile(sample_instrument):
    """Return a minimal InstrumentProfile for testing."""
    return InstrumentProfile(
        instrument=sample_instrument,
        earliest_ts={Interval("1d"): 1_000_000},
        latest_ts={Interval("1d"): 2_000_000},
        legs=[],
    )


# ─────────────────────────────────────────────────────────────────────────────
# Streamlit test helpers
# ─────────────────────────────────────────────────────────────────────────────


@pytest.fixture
def _app():
    """Provide a working directory so the app finds its assets."""
    original = os.getcwd()
    root = original
    while not os.path.isdir(os.path.join(root, "images")):
        parent = os.path.dirname(root)
        if parent == root:
            break
        root = parent
    os.chdir(root)
    yield
    os.chdir(original)
