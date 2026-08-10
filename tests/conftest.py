"""Backtide.

Author: Mavs
Description: Shared fixtures for the test suite.

"""

from pathlib import Path
import shutil
import tempfile

import pytest

from backtide.config import Config, DataConfig, set_config
from backtide.data import (
    Instrument,
    InstrumentProfile,
    Interval,
)

# ─────────────────────────────────────────────────────────────────────────────
# Storage path: every pytest run gets a fresh tempdir.
#
# A pre-built DuckDB containing AAPL daily bars (2024-01-01 → 2024-03-01)
# lives at `tests/_data/database.duckdb` and is copied into the tempdir so
# tests can run real backtests fully offline.
# ─────────────────────────────────────────────────────────────────────────────

db_location = Path(__file__).resolve().parent / "data" / "database.duckdb"
temp_location = Path(tempfile.mkdtemp(prefix="backtide_test_storage_"))

# Copy test database to temp location to not overwrite.
shutil.copy(db_location, temp_location / "database.duckdb")

set_config(
    Config(
        data=DataConfig(
            storage_path=str(temp_location),
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
