"""Backtide.

Author: Mavs
Description: Shared fixtures for the test suite.

"""

import os
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
# lives at ``tests/_data/database.duckdb`` and is copied into the
# tempdir so tests can run real backtests fully offline. Run
# ``python tests/bootstrap_data.py`` once to create the fixture.
# ─────────────────────────────────────────────────────────────────────────────

_FIXTURE_DB = Path(__file__).resolve().parent / "_data" / "database.duckdb"
_TEST_STORAGE = Path(tempfile.mkdtemp(prefix="backtide_test_storage_"))

if _FIXTURE_DB.exists():
    shutil.copy(_FIXTURE_DB, _TEST_STORAGE / "database.duckdb")

set_config(
    Config(
        data=DataConfig(
            storage_path=str(_TEST_STORAGE),
            providers={"crypto": "yahoo"},
        ),
    )
)


def fixture_db_available() -> bool:
    """Return True iff the pre-built test DuckDB fixture is present."""
    return _FIXTURE_DB.exists()


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
