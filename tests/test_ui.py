"""Backtide.

Author: Mavs
Description: Unit tests for the Streamlit UI pages and utility functions.

"""

from datetime import date
from zoneinfo import ZoneInfo

import pytest

from backtide.data import InstrumentType


# ─────────────────────────────────────────────────────────────────────────────
# UI utility functions (pure logic — no Streamlit dependency)
# ─────────────────────────────────────────────────────────────────────────────


class TestFmtNumber:
    """Tests for _fmt_number."""

    @pytest.mark.parametrize(
        ("n", "expected_substr"),
        [
            (500, "500"),
            (1_500, "1.5k"),
            (2_000_000, "2.00M"),
            (15_000_000, "15.0M"),
        ],
    )
    def test_format(self, n, expected_substr):
        from backtide.ui.utils import _fmt_number

        assert _fmt_number(n) == expected_substr


class TestGetTimezone:
    """Tests for _get_timezone."""

    def test_explicit(self):
        from backtide.ui.utils import _get_timezone

        tz = _get_timezone("UTC")
        assert tz == ZoneInfo("UTC")

    def test_none_returns_local(self):
        from backtide.ui.utils import _get_timezone

        tz = _get_timezone(None)
        assert tz is not None


class TestGetInstrumentTypeDescription:
    """Tests for _get_instrument_type_description."""

    @pytest.mark.parametrize(
        "it",
        [InstrumentType("stocks"), InstrumentType("etf"), InstrumentType("forex"), InstrumentType("crypto")],
    )
    def test_returns_tuple(self, it):
        from backtide.ui.utils import _get_instrument_type_description

        desc = _get_instrument_type_description(it)
        assert isinstance(desc, tuple)
        assert len(desc) == 2
        assert isinstance(desc[0], str)
        assert isinstance(desc[1], str)


class TestMomentToStrftime:
    """Tests for _moment_to_strftime."""

    def test_basic(self):
        from backtide.ui.utils import _moment_to_strftime

        assert _moment_to_strftime("YYYY-MM-DD") == "%Y-%m-%d"
        assert _moment_to_strftime("HH:mm:ss") == "%H:%M:%S"


class TestParseDate:
    """Tests for _parse_date."""

    def test_basic(self):
        from backtide.ui.utils import _parse_date

        result = _parse_date(0, "YYYY-MM-DD", ZoneInfo("UTC"))
        assert result == "1970-01-01"


class TestToPandas:
    """Tests for _to_pandas."""

    def test_passthrough(self):
        import pandas as pd

        from backtide.ui.utils import _to_pandas

        df = pd.DataFrame({"a": [1]})
        assert _to_pandas(df) is df

    def test_polars_conversion(self):
        from backtide.ui.utils import _to_pandas

        try:
            import polars as pl

            pldf = pl.DataFrame({"a": [1]})
            result = _to_pandas(pldf)
            import pandas as pd

            assert isinstance(result, pd.DataFrame)
        except ImportError:
            pytest.skip("polars not installed")


class TestGetLogokitUrl:
    """Tests for _get_logokit_url."""

    def test_stocks(self):
        from backtide.ui.utils import _get_logokit_url

        url = _get_logokit_url("AAPL", InstrumentType("stocks"), "key123")
        assert "logokit.com" in url
        assert "AAPL" in url
        assert "key123" in url

    def test_crypto(self):
        from backtide.ui.utils import _get_logokit_url

        url = _get_logokit_url("BTC-USD", InstrumentType("crypto"), "key")
        assert "crypto" in url
        assert "BTC" in url

    def test_forex(self):
        from backtide.ui.utils import _get_logokit_url

        url = _get_logokit_url("EUR-USD", InstrumentType("forex"), "key")
        assert "ticker" in url


# ─────────────────────────────────────────────────────────────────────────────
# Streamlit page rendering tests
# ─────────────────────────────────────────────────────────────────────────────
# Note: Streamlit AppTest has limitations. We test what we can.


@pytest.fixture
def _app():
    """Provide a working directory so the app finds its assets."""
    # The pages live under backtide/ui/ and reference images/ relative to CWD,
    # so we need to run from the repository root.
    import os

    original = os.getcwd()
    # Walk up until we find the images/ directory (repo root)
    root = original
    while not os.path.isdir(os.path.join(root, "images")):
        parent = os.path.dirname(root)
        if parent == root:
            break
        root = parent
    os.chdir(root)
    yield
    os.chdir(original)


class TestResultsPage:
    """Tests for the Results page."""

    @pytest.mark.usefixtures("_app")
    def test_results_renders(self):
        from streamlit.testing.v1 import AppTest

        at = AppTest.from_file("backtide/ui/results.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestStoragePage:
    """Tests for the Storage page."""

    @pytest.mark.usefixtures("_app")
    def test_storage_renders(self):
        from streamlit.testing.v1 import AppTest

        at = AppTest.from_file("backtide/ui/storage.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestDownloadPage:
    """Tests for the Download page."""

    @pytest.mark.usefixtures("_app")
    def test_download_renders(self):
        from streamlit.testing.v1 import AppTest

        at = AppTest.from_file("backtide/ui/download.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestExperimentPage:
    """Tests for the Experiment page."""

    @pytest.mark.usefixtures("_app")
    def test_experiment_renders(self):
        from streamlit.testing.v1 import AppTest

        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        assert not at.exception
