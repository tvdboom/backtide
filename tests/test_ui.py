"""Backtide.

Author: Mavs
Description: Unit tests for the Streamlit UI pages and utility functions.

"""

from datetime import date
import io
import json
import os
from unittest.mock import MagicMock
from zoneinfo import ZoneInfo

import pandas as pd
import polars as pl
import pytest
from streamlit.testing.v1 import AppTest

from backtide.backtest import (
    CodeSnippet,
    CommissionType,
    CurrencyConversionMode,
    DataExpConfig,
    ExperimentConfig,
    GeneralExpConfig,
    StrategyExpConfig,
    StrategyType,
)
from backtide.data import InstrumentType
from backtide.ui.utils import (
    _apply_config_to_state,
    _build_config_toml,
    _check_indicator_code,
    _check_strategy_code,
    _fmt_number,
    _get_instrument_type_description,
    _get_logokit_url,
    _get_timezone,
    _moment_to_strftime,
    _parse_config_upload,
    _parse_date,
    _to_pandas,
)

# ─────────────────────────────────────────────────────────────────────────────
# UI utility functions
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
        """Formatted number matches expected string."""
        assert _fmt_number(n) == expected_substr


class TestGetTimezone:
    """Tests for _get_timezone."""

    def test_explicit(self):
        """Explicit timezone string returns matching ZoneInfo."""
        tz = _get_timezone("UTC")
        assert tz == ZoneInfo("UTC")

    def test_none_returns_local(self):
        """None returns the local timezone."""
        tz = _get_timezone(None)
        assert tz is not None


class TestGetInstrumentTypeDescription:
    """Tests for _get_instrument_type_description."""

    @pytest.mark.parametrize(
        "it",
        [
            InstrumentType("stocks"),
            InstrumentType("etf"),
            InstrumentType("forex"),
            InstrumentType("crypto"),
        ],
    )
    def test_returns_tuple(self, it):
        """Each instrument type returns a (label, icon) tuple."""
        desc = _get_instrument_type_description(it)
        assert isinstance(desc, tuple)
        assert len(desc) == 2
        assert isinstance(desc[0], str)
        assert isinstance(desc[1], str)


class TestMomentToStrftime:
    """Tests for _moment_to_strftime."""

    def test_basic(self):
        """Moment.js format tokens are converted to strftime."""
        assert _moment_to_strftime("YYYY-MM-DD") == "%Y-%m-%d"
        assert _moment_to_strftime("HH:mm:ss") == "%H:%M:%S"


class TestParseDate:
    """Tests for _parse_date."""

    def test_basic(self):
        """Epoch 0 formats to 1970-01-01."""
        assert _parse_date(0, "YYYY-MM-DD", ZoneInfo("UTC")) == "1970-01-01"


class TestToPandas:
    """Tests for _to_pandas."""

    def test_passthrough(self):
        """Pandas DataFrame passes through unchanged."""
        df = pd.DataFrame({"a": [1]})
        assert _to_pandas(df) is df

    def test_polars_conversion(self):
        """Polars DataFrame is converted to pandas."""
        assert isinstance(_to_pandas(pl.DataFrame({"a": [1]})), pd.DataFrame)


class TestGetLogokitUrl:
    """Tests for _get_logokit_url."""

    def test_stocks(self):
        """Stock logokit URL contains ticker and key."""
        url = _get_logokit_url("AAPL", InstrumentType("stocks"), "key123")
        assert "logokit.com" in url
        assert "AAPL" in url
        assert "key123" in url

    def test_forex(self):
        """Forex logokit URL contains ticker path."""
        url = _get_logokit_url("EUR-USD", InstrumentType("forex"), "key")
        assert ":CUR" in url
        assert "ticker" in url

    def test_crypto(self):
        """Crypto logokit URL contains base symbol."""
        url = _get_logokit_url("BTC-USD", InstrumentType("crypto"), "key")
        assert "crypto" in url
        assert "BTC" in url


class TestCheckStrategyCode:
    """Tests for _check_strategy_code validation."""

    def test_valid_code(self):
        """Valid strategy code returns None."""
        code = "def strategy(data, state, indicators):\n    return []"
        assert _check_strategy_code(code) is None

    def test_wrong_signature(self):
        """Wrong function signature returns error message."""
        code = "def strategy(data):\n    return []"
        result = _check_strategy_code(code)
        assert result is not None
        assert "signature" in result

    def test_missing_function(self):
        """Missing strategy function returns error message."""
        code = "def other_func(data):\n    return []"
        result = _check_strategy_code(code)
        assert result is not None
        assert "No function" in result

    def test_syntax_error(self):
        """Syntax error in code returns error message."""
        code = "def strategy(data, state, indicators\n    return []"
        result = _check_strategy_code(code)
        assert result is not None
        assert "Syntax error" in result

    def test_empty_code(self):
        """Empty code returns no-function error."""
        assert _check_strategy_code("") is not None


class TestCheckIndicatorCode:
    """Tests for _check_indicator_code validation."""

    def test_valid_code(self):
        """Valid indicator code returns None."""
        code = "def indicator(data):\n    return {}"
        assert _check_indicator_code(code) is None

    def test_wrong_signature(self):
        """Wrong function signature returns error message."""
        code = "def indicator(data, extra):\n    return {}"
        result = _check_indicator_code(code)
        assert result is not None
        assert "signature" in result

    def test_missing_function(self):
        """Missing indicator function returns error message."""
        code = "x = 1"
        result = _check_indicator_code(code)
        assert result is not None
        assert "No function" in result

    def test_syntax_error(self):
        """Syntax error returns error message."""
        code = "def indicator(data\n    return {}"
        result = _check_indicator_code(code)
        assert result is not None
        assert "Syntax error" in result


class TestBuildConfigToml:
    """Tests for _build_config_toml."""

    def test_defaults(self):
        """Building with empty state and defaults produces valid TOML."""
        result = _build_config_toml({}, "test-exp", ExperimentConfig())
        assert isinstance(result, str)
        assert "test-exp" in result

    def test_with_state_values(self):
        """State values override defaults in the output."""
        state = {
            "tags": ["tag1"],
            "description": "A test",
            "initial_cash": 50000,
            "symbols": ["AAPL", "MSFT"],
            "custom_strategies": [{"code": "x = 1"}],
            "strategy_name_0": "My Strategy",
            "custom_indicators": [{"code": "y = 2"}],
            "indicator_name_0": "My Indicator",
        }
        result = _build_config_toml(state, "my-exp", ExperimentConfig())
        assert "my-exp" in result
        assert "AAPL" in result
        assert "MSFT" in result

    def test_with_dates(self):
        """Start/end dates are included when present."""
        state = {"start_date": "2020-01-01", "end_date": "2023-12-31"}
        result = _build_config_toml(state, "exp", ExperimentConfig())
        assert "2020-01-01" in result
        assert "2023-12-31" in result


class TestParseConfigUpload:
    """Tests for _parse_config_upload."""

    def test_toml(self):
        """Parse a TOML config upload."""
        content = b'[general]\nname = "test"\ntags = []\ndescription = ""\n'
        f = MagicMock()
        f.name = "config.toml"
        f.read.return_value = content
        result = _parse_config_upload(f)
        assert isinstance(result, ExperimentConfig)
        assert result.general.name == "test"

    def test_json(self):
        """Parse a JSON config upload."""
        data = {"general": {"name": "json-exp", "tags": [], "description": ""}}
        f = io.BytesIO(json.dumps(data).encode())
        f.name = "config.json"
        result = _parse_config_upload(f)
        assert isinstance(result, ExperimentConfig)
        assert result.general.name == "json-exp"

    def test_yaml(self):
        """Parse a YAML config upload."""
        content = b"general:\n  name: yaml-exp\n  tags: []\n  description: ''\n"
        f = io.BytesIO(content)
        f.name = "config.yaml"
        result = _parse_config_upload(f)
        assert isinstance(result, ExperimentConfig)
        assert result.general.name == "yaml-exp"

    def test_invalid_raises(self):
        """Invalid file content raises an exception."""
        f = MagicMock()
        f.name = "config.toml"
        f.read.return_value = b"not valid toml {{{"
        with pytest.raises(Exception):  # noqa: B017
            _parse_config_upload(f)


class TestApplyConfigToState:
    """Tests for _apply_config_to_state."""

    def test_applies_all_fields(self):
        """All config fields are written to state."""
        exp = ExperimentConfig(
            general=GeneralExpConfig(name="applied", tags=["t1"], description="d"),
            strategy=StrategyExpConfig(
                custom_strategies=[CodeSnippet(name="s1", code="x=1")],
            ),
        )
        state: dict = {}
        _apply_config_to_state(exp, state, ["code_editor", "upload"])
        assert state["experiment_name"] == "applied"
        assert state["tags"] == ["t1"]
        assert state["description"] == "d"
        assert state["strategy_name_0"] == "s1"
        assert len(state["custom_strategies"]) == 1
        assert state["warmup_period"] == 0
        assert "commission_type" in state

    def test_date_parsing(self):
        """Non-full-history with dates parses them correctly."""
        exp = ExperimentConfig(
            data=DataExpConfig(
                full_history=False,
                start_date="2020-01-15",
                end_date="2023-06-30",
            ),
        )
        state: dict = {}
        _apply_config_to_state(exp, state, ["editor"])
        assert state["full_history"] is False
        assert state["start_date"] == date(2020, 1, 15)
        assert state["end_date"] == date(2023, 6, 30)


# ─────────────────────────────────────────────────────────────────────────────
# Streamlit page rendering tests
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


class TestResultsPage:
    """Tests for the Results page."""

    @pytest.mark.usefixtures("_app")
    def test_results_renders(self):
        """Smoke test: results page renders without error."""
        at = AppTest.from_file("backtide/ui/results.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestAnalysisPage:
    """Tests for the Analysis page."""

    @pytest.mark.usefixtures("_app")
    def test_analysis_renders(self):
        """Smoke test: analysis page renders without error."""
        at = AppTest.from_file("backtide/ui/analysis.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestStoragePage:
    """Tests for the Storage page."""

    @pytest.mark.usefixtures("_app")
    def test_storage_renders(self):
        """Smoke test: storage page renders without error."""
        at = AppTest.from_file("backtide/ui/storage.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestDownloadPage:
    """Tests for the Download page."""

    @pytest.mark.usefixtures("_app")
    def test_download_renders(self):
        """Smoke test: download page renders without error."""
        at = AppTest.from_file("backtide/ui/download.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestExperimentPage:
    """Tests for the Experiment page."""

    @pytest.mark.usefixtures("_app")
    def test_experiment_renders(self):
        """Smoke test: experiment page renders without error."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_invalid_experiment_name(self):
        """Invalid filename characters in experiment name show an error."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.text_input(key="experiment_name").set_value("test<>name").run()
        assert any("not allowed" in e.value for e in at.error)

    @pytest.mark.usefixtures("_app")
    def test_toggle_full_history_off(self):
        """Disabling full_history shows date pickers."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="full_history").set_value(False).run()
        assert not at.exception
        assert len(at.date_input) >= 2

    @pytest.mark.usefixtures("_app")
    def test_toggle_margin_off(self):
        """Disabling margin hides leverage/margin fields."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="allow_margin").set_value(False).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "max_leverage" not in keys

    @pytest.mark.usefixtures("_app")
    def test_toggle_short_selling_off(self):
        """Disabling short selling hides borrow rate."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="allow_short_selling").set_value(False).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "borrow_rate" not in keys

    @pytest.mark.usefixtures("_app")
    def test_commission_fixed(self):
        """Switching commission type to Fixed shows fixed input."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.radio(key="commission_type").set_value(CommissionType("Fixed")).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "commission_fixed" in keys

    @pytest.mark.usefixtures("_app")
    def test_commission_percentage_plus_fixed(self):
        """Switching to PercentagePlusFixed shows both commission inputs."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.radio(key="commission_type").set_value(CommissionType("PercentagePlusFixed")).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "commission_pct" in keys
        assert "commission_fixed" in keys

    @pytest.mark.usefixtures("_app")
    def test_conversion_hold_until_threshold(self):
        """HoldUntilThreshold mode shows threshold input."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.selectbox(key="conversion_mode").set_value(
            CurrencyConversionMode("HoldUntilThreshold")
        ).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "conversion_threshold" in keys

    @pytest.mark.usefixtures("_app")
    def test_conversion_end_of_period(self):
        """EndOfPeriod mode shows period selectbox."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.selectbox(key="conversion_mode").set_value(CurrencyConversionMode("EndOfPeriod")).run()
        assert not at.exception
        keys = [s.key for s in at.selectbox]
        assert "conversion_period" in keys

    @pytest.mark.usefixtures("_app")
    def test_conversion_custom_interval(self):
        """CustomInterval mode shows interval number input."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.selectbox(key="conversion_mode").set_value(
            CurrencyConversionMode("CustomInterval")
        ).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "conversion_interval" in keys

    @pytest.mark.usefixtures("_app")
    def test_add_strategy_button(self):
        """Clicking 'Add strategy' adds a custom strategy entry."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        initial = len(at.text_input)
        at.button[0].click().run()
        assert not at.exception
        assert len(at.text_input) > initial

    @pytest.mark.usefixtures("_app")
    def test_add_indicator_button(self):
        """Clicking 'Add indicator' adds a custom indicator entry."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        initial = len(at.text_input)
        at.button[1].click().run()
        assert not at.exception
        assert len(at.text_input) > initial

    @pytest.mark.usefixtures("_app")
    def test_description_text_area(self):
        """Setting description text area works."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.text_area[0].set_value("My test description").run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_number_input_initial_cash(self):
        """Changing initial cash value works."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.number_input(key="initial_cash").set_value(50000).run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_toggle_trade_on_close(self):
        """Toggling trade_on_close works."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="trade_on_close").set_value(True).run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_config_upload_success(self):
        """Config import success message is shown."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.session_state["_import_success"] = "Loaded config."
        at.run()
        assert any("Loaded" in s.value for s in at.success)

    @pytest.mark.usefixtures("_app")
    def test_config_upload_error(self):
        """Config import error message is shown."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.session_state["_import_error"] = "Failed to parse."
        at.run()
        assert any("Failed" in e.value for e in at.error)

    @pytest.mark.usefixtures("_app")
    def test_invalid_tag(self):
        """Invalid tags show an error."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.session_state["tags"] = ["bad<>tag"]
        at.run()
        assert any("Invalid tag" in e.value for e in at.error)

    @pytest.mark.usefixtures("_app")
    def test_predefined_strategy_selected(self):
        """Selecting predefined strategies shows descriptions."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.multiselect(key="predefined_strategies").set_value([StrategyType("BuyAndHold")]).run()
        assert not at.exception
        assert any("Buy" in m.value for m in at.markdown)

    @pytest.mark.usefixtures("_app")
    def test_current_tab_restored(self):
        """Setting current_tab restores tab selection."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.session_state["current_tab"] = 1
        at.run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_use_storage_toggle(self):
        """Toggling use_storage works."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="use_storage").set_value(True).run()
        assert not at.exception
