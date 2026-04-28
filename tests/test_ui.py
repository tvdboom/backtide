"""Backtide.

Author: Mavs
Description: Unit tests for the Streamlit UI pages and utility functions.

"""

from datetime import date
import io
import json
from typing import cast
from unittest.mock import MagicMock, patch
from zoneinfo import ZoneInfo

import pandas as pd
import polars as pl
import pytest
import streamlit as st
from streamlit.testing.v1 import AppTest

from backtide.backtest import (
    CommissionType,
    CurrencyConversionMode,
    DataExpConfig,
    EngineExpConfig,
    ExchangeExpConfig,
    ExperimentConfig,
    GeneralExpConfig,
    StrategyExpConfig,
)
from backtide.config import Config, DataConfig, get_config
from backtide.data import Instrument, InstrumentProfile, InstrumentType, Interval, Provider
from backtide.indicators import BUILTIN_INDICATORS, BaseIndicator
from backtide.indicators.utils import (
    _build_custom_indicator,
    _check_indicator_code,
    _get_indicator_label,
    _is_builtin_indicator,
    _load_stored_indicators,
    _save_indicator,
)
from backtide.strategies import BUILTIN_STRATEGIES
from backtide.strategies.utils import (
    _build_custom_strategy,
    _check_strategy_code,
    _get_strategy_label,
    _is_builtin_strategy,
    _load_stored_strategies,
    _save_strategy,
)
from backtide.ui.experiment import (
    _apply_config_to_state,
    _build_config_toml,
    _on_config_upload,
    _parse_config_upload,
)
from backtide.ui.utils import (
    _clear_state,
    _default,
    _draw_cards,
    _fmt_number,
    _get_instrument_type_description,
    _get_logokit_url,
    _get_provider_logo,
    _get_timezone,
    _moment_to_strftime,
    _parse_date,
    _persist,
    _to_upper_values,
)
from backtide.utils.utils import _to_pandas

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

    def test_crypto_use_quote(self):
        """Crypto logokit URL with use_quote returns quote symbol."""
        url = _get_logokit_url("BTC-USD", InstrumentType("crypto"), "key", use_quote=True)
        assert "USD" in url


class TestClearState:
    """Tests for _clear_state."""

    def test_clears_keys(self):
        """Clear keys sets values to empty lists and removes shadows."""
        st.session_state["test_key"] = ["a", "b"]
        st.session_state["_test_key"] = "shadow"
        _clear_state("test_key")
        assert st.session_state["test_key"] == []
        assert "_test_key" not in st.session_state


class TestDefault:
    """Tests for _default."""

    def test_returns_fallback_when_missing(self):
        """Return fallback when shadow key is missing."""
        st.session_state.pop("_missing_key", None)
        assert _default("missing_key", "fallback") == "fallback"

    def test_returns_shadow_value(self):
        """Return shadow value when present."""
        st.session_state["_existing_key"] = "shadow_value"
        assert _default("existing_key") == "shadow_value"
        st.session_state.pop("_existing_key", None)


class TestPersist:
    """Tests for _persist."""

    def test_copies_to_shadow(self):
        """Copy widget value to shadow key."""
        st.session_state["widget_key"] = "widget_value"
        _persist("widget_key")
        assert st.session_state["_widget_key"] == "widget_value"
        st.session_state.pop("widget_key", None)
        st.session_state.pop("_widget_key", None)

    def test_no_op_if_missing(self):
        """Do nothing if key is not in session state."""
        st.session_state.pop("nonexistent_key", None)
        _persist("nonexistent_key")
        assert "_nonexistent_key" not in st.session_state


class TestToUpperValues:
    """Tests for _to_upper_values."""

    def test_uppercases_strings(self):
        """Uppercase string values in session state."""
        st.session_state["upper_key"] = ["aapl", "msft"]
        _to_upper_values("upper_key")
        assert st.session_state["upper_key"] == ["AAPL", "MSFT"]
        st.session_state.pop("upper_key", None)

    def test_no_op_if_missing(self):
        """Do nothing if key is not in session state."""
        st.session_state.pop("nonexistent_upper", None)
        _to_upper_values("nonexistent_upper")

    def test_non_string_values_preserved(self):
        """Non-string values are preserved as-is."""
        st.session_state["mixed_key"] = ["aapl", 42]
        _to_upper_values("mixed_key")
        assert st.session_state["mixed_key"] == ["AAPL", 42]
        st.session_state.pop("mixed_key", None)


class TestMomentToStrftimeExtended:
    """Extended tests for _moment_to_strftime."""

    def test_time_parts(self):
        """Time format tokens are converted correctly."""
        assert _moment_to_strftime("HH:mm:ss") == "%H:%M:%S"

    def test_ampm(self):
        """AM/PM tokens are converted."""
        assert _moment_to_strftime("hh:mm A") == "%I:%M %p"


class TestGetProviderLogo:
    """Tests for _get_provider_logo."""

    @pytest.mark.usefixtures("_app")
    def test_returns_data_uri(self):
        """Return a base64-encoded data URI for a known provider."""
        result = _get_provider_logo.__wrapped__(Provider.Yahoo)  # ty: ignore[unresolved-attribute]

        assert result.startswith("data:image/png;base64,")


class TestDrawCards:
    """Tests for _draw_cards."""

    pytestmark = pytest.mark.usefixtures("_app")

    def test_estimate_rows(self):
        """Draw cards with estimate_rows returns HTML and total rows."""
        cfg = Config()
        tz = _get_timezone(cfg.display.timezone)

        inst = Instrument(
            symbol="AAPL",
            name="Apple Inc.",
            base=None,
            quote="USD",
            instrument_type="stocks",
            exchange="XNAS",
            provider="yahoo",
        )
        profile = InstrumentProfile(
            instrument=inst,
            earliest_ts={Interval("1d"): 1_700_000_000},
            latest_ts={Interval("1d"): 1_710_000_000},
            legs=[],
        )

        html, total_rows = _draw_cards(
            [profile],
            cfg=cfg,
            tz=tz,
            instrument_type=InstrumentType("stocks"),
            full_history=True,
            start_ts=date(2023, 11, 14),
            end_ts=date(2024, 3, 10),
            estimate_rows=True,
        )
        assert isinstance(html, str)
        assert total_rows > 0
        assert "AAPL" in html

    def test_estimate_rows_crypto(self):
        """Draw cards for crypto instruments with estimation."""
        cfg = Config()
        tz = _get_timezone(cfg.display.timezone)

        inst = Instrument(
            symbol="BTC-USD",
            name="Bitcoin USD",
            base="BTC",
            quote="USD",
            instrument_type="crypto",
            exchange="crypto",
            provider="yahoo",
        )
        profile = InstrumentProfile(
            instrument=inst,
            earliest_ts={Interval("1d"): 1_700_000_000},
            latest_ts={Interval("1d"): 1_710_000_000},
            legs=[],
        )

        html, total_rows = _draw_cards(
            [profile],
            cfg=cfg,
            tz=tz,
            instrument_type=InstrumentType("crypto"),
            full_history=True,
            start_ts=date(2023, 11, 14),
            end_ts=date(2024, 3, 10),
            estimate_rows=True,
        )
        assert isinstance(html, str)
        assert total_rows > 0

    def test_with_legs(self):
        """Draw cards for profiles with FX legs."""
        cfg = Config()
        tz = _get_timezone(cfg.display.timezone)

        inst = Instrument(
            symbol="AAPL",
            name="Apple Inc.",
            base=None,
            quote="USD",
            instrument_type="stocks",
            exchange="XNAS",
            provider="yahoo",
        )
        profile = InstrumentProfile(
            instrument=inst,
            earliest_ts={Interval("1d"): 1_700_000_000},
            latest_ts={Interval("1d"): 1_710_000_000},
            legs=["EUR-USD"],
        )

        html, _ = _draw_cards(
            [profile],
            cfg=cfg,
            tz=tz,
            instrument_type=InstrumentType("stocks"),
            full_history=True,
            start_ts=date(2023, 11, 14),
            end_ts=date(2024, 3, 10),
            estimate_rows=True,
        )
        assert "EUR-USD" in html

    def test_estimate_rows_forex(self):
        """Draw cards for forex instruments with estimation."""
        cfg = Config()
        tz = _get_timezone(cfg.display.timezone)

        inst = Instrument(
            symbol="EUR-USD",
            name="Euro Dollar",
            base="EUR",
            quote="USD",
            instrument_type="forex",
            exchange="forex",
            provider="yahoo",
        )
        profile = InstrumentProfile(
            instrument=inst,
            earliest_ts={Interval("1d"): 1_700_000_000},
            latest_ts={Interval("1d"): 1_710_000_000},
            legs=[],
        )

        html, total_rows = _draw_cards(
            [profile],
            cfg=cfg,
            tz=tz,
            instrument_type=InstrumentType("forex"),
            full_history=True,
            start_ts=date(2023, 11, 14),
            end_ts=date(2024, 3, 10),
            estimate_rows=True,
        )
        assert isinstance(html, str)
        assert total_rows > 0

    def test_not_full_history(self):
        """Draw cards with partial history adjusts date range."""
        cfg = Config()
        tz = _get_timezone(cfg.display.timezone)

        inst = Instrument(
            symbol="AAPL",
            name="Apple Inc.",
            base=None,
            quote="USD",
            instrument_type="stocks",
            exchange="XNAS",
            provider="yahoo",
        )
        profile = InstrumentProfile(
            instrument=inst,
            earliest_ts={Interval("1d"): 1_700_000_000},
            latest_ts={Interval("1d"): 1_710_000_000},
            legs=[],
        )

        html, total_rows = _draw_cards(
            [profile],
            cfg=cfg,
            tz=tz,
            instrument_type=InstrumentType("stocks"),
            full_history=False,
            start_ts=date(2023, 12, 1),
            end_ts=date(2024, 2, 1),
            estimate_rows=True,
        )
        assert isinstance(html, str)
        assert total_rows > 0

    def test_intraday_equity(self):
        """Draw cards for equity with intraday interval."""
        cfg = Config()
        tz = _get_timezone(cfg.display.timezone)

        inst = Instrument(
            symbol="AAPL",
            name="Apple Inc.",
            base=None,
            quote="USD",
            instrument_type="stocks",
            exchange="XNAS",
            provider="yahoo",
        )
        profile = InstrumentProfile(
            instrument=inst,
            earliest_ts={Interval("1h"): 1_700_000_000},
            latest_ts={Interval("1h"): 1_701_000_000},
            legs=[],
        )

        _, total_rows = _draw_cards(
            [profile],
            cfg=cfg,
            tz=tz,
            instrument_type=InstrumentType("stocks"),
            full_history=True,
            start_ts=date(2023, 11, 14),
            end_ts=date(2023, 11, 26),
            estimate_rows=True,
        )
        assert total_rows > 0

    def test_intraday_forex(self):
        """Draw cards for forex with intraday interval."""
        cfg = Config()
        tz = _get_timezone(cfg.display.timezone)

        inst = Instrument(
            symbol="EUR-USD",
            name="Euro Dollar",
            base="EUR",
            quote="USD",
            instrument_type="forex",
            exchange="forex",
            provider="yahoo",
        )
        profile = InstrumentProfile(
            instrument=inst,
            earliest_ts={Interval("1h"): 1_700_000_000},
            latest_ts={Interval("1h"): 1_701_000_000},
            legs=[],
        )

        _, total_rows = _draw_cards(
            [profile],
            cfg=cfg,
            tz=tz,
            instrument_type=InstrumentType("forex"),
            full_history=True,
            start_ts=date(2023, 11, 14),
            end_ts=date(2023, 11, 26),
            estimate_rows=True,
        )
        assert total_rows > 0

    def test_multi_year_range(self):
        """Draw cards with multi-year range shows year display."""
        cfg = Config()
        tz = _get_timezone(cfg.display.timezone)

        inst = Instrument(
            symbol="AAPL",
            name="Apple Inc.",
            base=None,
            quote="USD",
            instrument_type="stocks",
            exchange="XNAS",
            provider="yahoo",
        )
        # Span over 2 years
        profile = InstrumentProfile(
            instrument=inst,
            earliest_ts={Interval("1d"): 1_600_000_000},
            latest_ts={Interval("1d"): 1_710_000_000},
            legs=[],
        )

        html, _ = _draw_cards(
            [profile],
            cfg=cfg,
            tz=tz,
            instrument_type=InstrumentType("stocks"),
            full_history=True,
            start_ts=date(2020, 9, 13),
            end_ts=date(2024, 3, 10),
            estimate_rows=True,
        )
        assert "y " in html  # year display like "3y 120d"


# ─────────────────────────────────────────────────────────────────────────────
# Indicator utils tests
# ─────────────────────────────────────────────────────────────────────────────


class TestBuildCustomIndicator:
    """Tests for _build_custom_indicator."""

    @pytest.fixture(autouse=True)
    def _import(self):
        self._build = _build_custom_indicator

    def test_valid_code(self):
        """Build a valid custom indicator from code."""
        code = (
            "from backtide.indicators import BaseIndicator\n"
            "class MyInd(BaseIndicator):\n"
            "    def compute(self, data):\n"
            "        return data['close'] if hasattr(data, '__getitem__') else data[:, 3]\n"
            "MyInd()\n"
        )
        result = self._build(code)
        assert result is not None
        assert hasattr(result, "_source_code")

    def test_no_trailing_expression(self):
        """Raise ValueError when code has no trailing expression."""
        code = (
            "from backtide.indicators import BaseIndicator\n"
            "class MyInd(BaseIndicator):\n"
            "    def compute(self, data):\n"
            "        return data\n"
        )
        with pytest.raises(ValueError, match="last statement"):
            self._build(code)

    def test_not_base_indicator(self):
        """Raise TypeError when result is not BaseIndicator."""
        code = "class NotInd:\n    pass\nNotInd()\n"
        with pytest.raises(TypeError, match="BaseIndicator"):
            self._build(code)


class TestCheckIndicatorCodeExtended:
    """Extended tests for _check_indicator_code."""

    @pytest.fixture(autouse=True)
    def _import(self):
        self._check = _check_indicator_code

    def test_compute_returns_none(self):
        """Return error when compute returns None."""
        code = (
            "from backtide.indicators import BaseIndicator\n"
            "class MyInd(BaseIndicator):\n"
            "    def compute(self, data):\n"
            "        return None\n"
            "MyInd()\n"
        )
        result = self._check(code, get_config())
        assert result is not None
        assert "None" in result

    def test_compute_raises_exception(self):
        """Return error when compute raises an exception."""
        code = (
            "from backtide.indicators import BaseIndicator\n"
            "class MyInd(BaseIndicator):\n"
            "    def compute(self, data):\n"
            "        raise RuntimeError('oops')\n"
            "MyInd()\n"
        )
        result = self._check(code, get_config())
        assert result is not None
        assert "oops" in result


class TestIndicatorLabel:
    """Tests for _get_indicator_label."""

    def test_custom_indicator_label(self):
        """Custom indicator label includes 'Custom'."""
        ind = MagicMock()
        ind.__class__.__module__ = "user_code"
        label = _get_indicator_label("TestInd", ind)
        assert "Custom" in label

    def test_builtin_indicator_label(self):
        """Builtin indicator label includes the indicator name."""
        ind = cast(BaseIndicator, BUILTIN_INDICATORS[0]())
        label = _get_indicator_label("SMA", ind)
        assert "SMA" in label


class TestIsBuiltinIndicator:
    """Tests for _is_builtin_indicator."""

    def test_builtin(self):
        """Return True for a built-in indicator."""
        # BUILTIN_INDICATORS contains classes - instantiate one
        ind = BUILTIN_INDICATORS[0]()
        assert _is_builtin_indicator(ind) is True

    def test_custom(self):
        """Return False for a custom indicator."""
        ind = MagicMock()
        ind.__class__.__module__ = "user_code"
        assert _is_builtin_indicator(ind) is False


class TestSaveLoadIndicator:
    """Tests for _save_indicator and _load_stored_indicators."""

    def test_save_and_load(self, tmp_path):
        """Save and load an indicator from disk."""
        cfg = Config(data=DataConfig(storage_path=str(tmp_path)))
        (tmp_path / "indicators").mkdir()
        ind = cast(BaseIndicator, BUILTIN_INDICATORS[0]())
        _save_indicator(ind, "test_sma", cfg)
        loaded = _load_stored_indicators(cfg)
        assert "test_sma" in loaded

    def test_load_corrupt_file(self, tmp_path):
        """Corrupt pkl file shows an error but continues."""
        cfg = Config(data=DataConfig(storage_path=str(tmp_path)))
        (tmp_path / "indicators").mkdir()
        (tmp_path / "indicators" / "bad.pkl").write_bytes(b"corrupt")
        with patch("backtide.indicators.utils.st") as mock_st:
            result = _load_stored_indicators(cfg)
            mock_st.error.assert_called_once()
        assert "bad" not in result


# ─────────────────────────────────────────────────────────────────────────────
# Strategy utils tests
# ─────────────────────────────────────────────────────────────────────────────


class TestBuildCustomStrategy:
    """Tests for _build_custom_strategy."""

    @pytest.fixture(autouse=True)
    def _import(self):
        self._build = _build_custom_strategy

    def test_valid_code(self):
        """Build a valid custom strategy from code."""
        code = (
            "from backtide.strategies import BaseStrategy\n"
            "class S(BaseStrategy):\n"
            "    def evaluate(self, data, portfolio, state, indicators):\n"
            "        return []\n"
            "S()\n"
        )
        result = self._build(code)
        assert result is not None
        assert hasattr(result, "_source_code")

    def test_no_trailing_expression(self):
        """Raise ValueError when code has no trailing expression."""
        code = (
            "from backtide.strategies import BaseStrategy\n"
            "class S(BaseStrategy):\n"
            "    def evaluate(self, data, portfolio, state, indicators):\n"
            "        return []\n"
        )
        with pytest.raises(ValueError, match="last statement"):
            self._build(code)

    def test_not_base_strategy(self):
        """Raise TypeError when result is not BaseStrategy."""
        code = "class NotStrat:\n    pass\nNotStrat()\n"
        with pytest.raises(TypeError, match="BaseStrategy"):
            self._build(code)


class TestCheckStrategyCodeExtended:
    """Extended tests for _check_strategy_code."""

    @pytest.fixture(autouse=True)
    def _import(self):
        self._check = _check_strategy_code

    def test_no_return_statement(self):
        """Return error when evaluate has no return statement."""
        code = (
            "from backtide.strategies import BaseStrategy\n"
            "class S(BaseStrategy):\n"
            "    def evaluate(self, data, portfolio, state, indicators):\n"
            "        pass\n"
            "S()\n"
        )
        result = self._check(code)
        assert result is not None
        assert "return" in result.lower()

    def test_bare_return(self):
        """Return error when evaluate has a bare return statement."""
        code = (
            "from backtide.strategies import BaseStrategy\n"
            "class S(BaseStrategy):\n"
            "    def __init__(self):\n"
            "        super().__init__()\n"
            "    def evaluate(self, data, portfolio, state, indicators):\n"
            "        return\n"
            "S()\n"
        )
        result = self._check(code)
        assert result is not None
        assert "None" in result

    def test_return_none(self):
        """Return error when evaluate returns a constant None."""
        code = (
            "from backtide.strategies import BaseStrategy\n"
            "class S(BaseStrategy):\n"
            "    def evaluate(self, data, portfolio, state, indicators):\n"
            "        return None\n"
            "S()\n"
        )
        result = self._check(code)
        assert result is not None
        assert "None" in result

    def test_return_constant(self):
        """Return error when evaluate returns a constant."""
        code = (
            "from backtide.strategies import BaseStrategy\n"
            "class S(BaseStrategy):\n"
            "    def evaluate(self, data, portfolio, state, indicators):\n"
            "        return 42\n"
            "S()\n"
        )
        result = self._check(code)
        assert result is not None
        assert "constant" in result.lower()


class TestStrategyLabel:
    """Tests for _get_strategy_label."""

    def test_custom_strategy_label(self):
        """Custom strategy label includes 'Custom'."""
        strat = MagicMock()
        strat.__class__.__module__ = "user_code"
        label = _get_strategy_label("TestStrat", strat)
        assert "Custom" in label

    def test_builtin_strategy_label(self):
        """Builtin strategy label includes the strategy name."""
        strat = BUILTIN_STRATEGIES[0]()
        label = _get_strategy_label("TestStrat", strat)
        assert "TestStrat" in label


class TestIsBuiltinStrategy:
    """Tests for _is_builtin_strategy."""

    def test_builtin(self):
        """Return True for a built-in strategy."""
        assert _is_builtin_strategy(BUILTIN_STRATEGIES[0]()) is True

    def test_custom(self):
        """Return False for a custom strategy."""
        strat = MagicMock()
        strat.__class__.__module__ = "user_code"
        assert _is_builtin_strategy(strat) is False


class TestSaveLoadStrategy:
    """Tests for _save_strategy and _load_stored_strategies."""

    def test_save_and_load(self, tmp_path):
        """Save and load a strategy from disk."""
        cfg = Config(data=DataConfig(storage_path=str(tmp_path)))
        strat = BUILTIN_STRATEGIES[0]()
        _save_strategy(strat, "test_strategy", cfg)
        loaded = _load_stored_strategies(cfg)
        assert "test_strategy" in loaded

    def test_load_corrupt_file(self, tmp_path):
        """Corrupt pkl file shows an error but continues."""
        cfg = Config(data=DataConfig(storage_path=str(tmp_path)))
        (tmp_path / "strategies").mkdir()
        (tmp_path / "strategies" / "bad.pkl").write_bytes(b"corrupt")
        with patch("backtide.strategies.utils.st") as mock_st:
            result = _load_stored_strategies(cfg)
            mock_st.error.assert_called_once()
        assert "bad" not in result

    def test_load_empty_directory(self, tmp_path):
        """Empty directory returns empty dict."""
        cfg = Config(data=DataConfig(storage_path=str(tmp_path)))
        (tmp_path / "strategies").mkdir()
        result = _load_stored_strategies(cfg)
        assert result == {}


# ─────────────────────────────────────────────────────────────────────────────
# Original check code tests
# ─────────────────────────────────────────────────────────────────────────────


class TestCheckStrategyCode:
    """Tests for _check_strategy_code validation."""

    @pytest.fixture(autouse=True)
    def _import(self):
        self._check = _check_strategy_code

    def test_valid_code(self):
        """Valid strategy code returns None."""
        code = (
            "from backtide.strategies import BaseStrategy\n"
            "class S(BaseStrategy):\n"
            "    def evaluate(self, data, portfolio, state, indicators):\n"
            "        return []\n"
            "S()\n"
        )
        assert self._check(code) is None

    def test_wrong_signature(self):
        """Wrong evaluate signature returns error message."""
        code = (
            "from backtide.strategies import BaseStrategy\n"
            "class S(BaseStrategy):\n"
            "    def evaluate(self, data):\n"
            "        return []\n"
            "S()\n"
        )
        result = self._check(code)
        assert result is not None
        assert "signature" in result.lower()

    def test_not_base_strategy(self):
        """Code that doesn't subclass BaseStrategy returns error."""
        code = "class S:\n    pass\nS()\n"
        result = self._check(code)
        assert result is not None

    def test_syntax_error(self):
        """Syntax error in code returns error message."""
        code = "def strategy(data, state, indicators\n    return []"
        result = self._check(code)
        assert result is not None
        assert "Syntax error" in result

    def test_empty_code(self):
        """Empty code returns error."""
        assert self._check("") is not None


class TestCheckIndicatorCode:
    """Tests for _check_indicator_code validation."""

    @pytest.fixture(autouse=True)
    def _import(self):
        self._check = _check_indicator_code

    def test_valid_code(self):
        """Valid indicator code with compute(self, data) returns None."""
        code = (
            "from backtide.indicators import BaseIndicator\n"
            "class MyInd(BaseIndicator):\n"
            "    def compute(self, data):\n"
            "        return data['close']\n"
            "MyInd()\n"
        )
        assert self._check(code, get_config()) is None

    def test_wrong_signature(self):
        """Wrong compute signature returns error message."""
        code = (
            "from backtide.indicators import BaseIndicator\n"
            "class MyInd(BaseIndicator):\n"
            "    def compute(self, data, extra):\n"
            "        return data\n"
            "MyInd()\n"
        )
        result = self._check(code, get_config())
        assert result is not None

    def test_missing_class(self):
        """Missing BaseIndicator subclass returns error message."""
        code = "x = 1"
        result = self._check(code, get_config())
        assert result is not None

    def test_syntax_error(self):
        """Syntax error returns error message."""
        code = "class MyInd(BaseIndicator\n    pass"
        result = self._check(code, get_config())
        assert result is not None
        assert "Syntax error" in result


class TestBuildConfigToml:
    """Tests for _build_config_toml."""

    @pytest.fixture(autouse=True)
    def _import(self):
        self._build = _build_config_toml

    def test_defaults(self):
        """Building with empty state and defaults produces valid TOML."""
        result = self._build({}, "test-exp", ExperimentConfig())
        assert isinstance(result, str)
        assert "test-exp" in result

    def test_with_state_values(self):
        """State values override defaults in the output."""
        state = {
            "tags": ["tag1"],
            "description": "A test",
            "initial_cash": 50000,
            "symbols": ["AAPL", "MSFT"],
            "strategies": ["s1"],
            "strategy_name_0": "My Strategy",
            "custom_indicators": [{"code": "y = 2"}],
            "indicator_name_0": "My Indicator",
        }
        result = self._build(state, "my-exp", ExperimentConfig())
        assert "my-exp" in result
        assert "AAPL" in result
        assert "MSFT" in result

    def test_with_dates(self):
        """Start/end dates are included when present."""
        state = {"start_date": "2020-01-01", "end_date": "2023-12-31"}
        result = self._build(state, "exp", ExperimentConfig())
        assert "2020-01-01" in result
        assert "2023-12-31" in result

    def test_none_values_coerced_to_defaults(self):
        """None values in state must not break the Rust constructors.

        Streamlit text widgets can return ``None`` (e.g. an empty
        ``text_area`` after the user clears it). The builder must coerce
        those back to the dataclass defaults rather than passing ``None``
        through to the typed Rust fields.
        """
        # Simulate every str-typed widget being explicitly None.
        state = {
            "description": None,
            "tags": None,
            "benchmark": None,
            "symbols": None,
        }
        # Should not raise.
        result = self._build(state, "exp", ExperimentConfig())
        assert "exp" in result

    def test_empty_experiment_name_uses_blank(self):
        """An empty experiment name is allowed (mirrors the UI default)."""
        result = self._build({}, "", ExperimentConfig())
        assert isinstance(result, str)


class TestParseConfigUpload:
    """Tests for _parse_config_upload."""

    @pytest.fixture(autouse=True)
    def _import(self):
        self._parse = _parse_config_upload

    def test_toml(self):
        """Parse a TOML config upload."""
        content = b'[general]\nname = "test"\ntags = []\ndescription = ""\n'
        f = MagicMock()
        f.name = "config.toml"
        f.read.return_value = content
        result = self._parse(f)
        assert isinstance(result, ExperimentConfig)
        assert result.general.name == "test"

    def test_json(self):
        """Parse a JSON config upload."""
        data = {"general": {"name": "json-exp", "tags": [], "description": ""}}
        f = io.BytesIO(json.dumps(data).encode())
        f.name = "config.json"
        result = self._parse(f)
        assert isinstance(result, ExperimentConfig)
        assert result.general.name == "json-exp"

    def test_yaml(self):
        """Parse a YAML config upload."""
        content = b"general:\n  name: yaml-exp\n  tags: []\n  description: ''\n"
        f = io.BytesIO(content)
        f.name = "config.yaml"
        result = self._parse(f)
        assert isinstance(result, ExperimentConfig)
        assert result.general.name == "yaml-exp"


class TestApplyConfigToState:
    """Tests for _apply_config_to_state."""

    @pytest.fixture(autouse=True)
    def _import(self):
        self._apply = _apply_config_to_state

    def test_applies_all_fields(self):
        """All config fields are written to state."""
        exp = ExperimentConfig(
            general=GeneralExpConfig(name="applied", tags=["t1"], description="d"),
            strategy=StrategyExpConfig(
                strategies=["s1"],
            ),
        )
        state = {}
        self._apply(exp, state)
        assert state["experiment_name"] == "applied"
        assert state["tags"] == ["t1"]
        assert state["description"] == "d"
        assert len(state["strategies"]) == 1
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
        self._apply(exp, state)
        assert state["full_history"] is False
        assert state["start_date"] == date(2020, 1, 15)
        assert state["end_date"] == date(2023, 6, 30)

    def test_exchange_fields(self):
        """Exchange fields are written to state."""
        exp = ExperimentConfig(
            exchange=ExchangeExpConfig(
                slippage=0.5,
                conversion_threshold=500.0,
            ),
        )
        state: dict = {}
        self._apply(exp, state)
        assert state["slippage"] == 0.5
        assert state["conversion_threshold"] == 500.0

    def test_engine_fields(self):
        """Engine fields are written to state."""
        exp = ExperimentConfig(
            engine=EngineExpConfig(
                warmup_period=10,
                trade_on_close=True,
            ),
        )
        state: dict = {}
        self._apply(exp, state)
        assert state["warmup_period"] == 10
        assert state["trade_on_close"] is True


# ─────────────────────────────────────────────────────────────────────────────
# Experiment helper tests
# ─────────────────────────────────────────────────────────────────────────────


class TestOnConfigUpload:
    """Tests for _on_config_upload."""

    def test_none_upload(self):
        """None upload returns early without error."""
        st.session_state.pop("config_upload", None)
        _on_config_upload()  # should not raise


# ─────────────────────────────────────────────────────────────────────────────
# Streamlit page rendering tests
# ─────────────────────────────────────────────────────────────────────────────


class TestIndicatorsPage:
    """Tests for the Indicators page."""

    @pytest.mark.usefixtures("_app")
    def test_indicators_renders(self):
        """Smoke test: indicators page renders without error."""
        at = AppTest.from_file("src/backtide/ui/indicators.py", default_timeout=30)
        at.run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_add_builtin_button(self):
        """Clicking 'Add built-in' shows the builtin indicator form."""
        at = AppTest.from_file("src/backtide/ui/indicators.py", default_timeout=30)
        at.run()
        at.button[0].click().run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_add_custom_button(self):
        """Clicking 'Add custom' shows the custom indicator form."""
        at = AppTest.from_file("src/backtide/ui/indicators.py", default_timeout=30)
        at.run()
        at.button[1].click().run()
        assert not at.exception


class TestStrategiesPage:
    """Tests for the Strategies page."""

    @pytest.mark.usefixtures("_app")
    def test_strategies_renders(self):
        """Smoke test: strategies page renders without error."""
        at = AppTest.from_file("src/backtide/ui/strategies.py", default_timeout=30)
        at.run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_add_builtin_button(self):
        """Clicking 'Add built-in' shows the builtin strategy form."""
        at = AppTest.from_file("src/backtide/ui/strategies.py", default_timeout=30)
        at.run()
        at.button[0].click().run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_add_custom_button(self):
        """Clicking 'Add custom' shows the custom strategy form."""
        at = AppTest.from_file("src/backtide/ui/strategies.py", default_timeout=30)
        at.run()
        at.button[1].click().run()
        assert not at.exception


class TestResultsPage:
    """Tests for the Results page."""

    @pytest.mark.usefixtures("_app")
    def test_results_renders(self):
        """Smoke test: results page renders without error."""
        at = AppTest.from_file("src/backtide/ui/results.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestAnalysisPage:
    """Tests for the Analysis page."""

    @pytest.mark.usefixtures("_app")
    def test_analysis_renders(self):
        """Smoke test: analysis page renders without error."""
        at = AppTest.from_file("src/backtide/ui/analysis.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestStoragePage:
    """Tests for the Storage page."""

    @pytest.mark.usefixtures("_app")
    def test_storage_renders(self):
        """Smoke test: storage page renders without error."""
        at = AppTest.from_file("src/backtide/ui/storage.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestDownloadPage:
    """Tests for the Download page."""

    @pytest.mark.usefixtures("_app")
    def test_download_renders(self):
        """Smoke test: download page renders without error."""
        at = AppTest.from_file("src/backtide/ui/download.py", default_timeout=30)
        at.run()
        assert not at.exception


class TestExperimentPage:
    """Tests for the Experiment page."""

    @pytest.mark.usefixtures("_app")
    def test_experiment_renders(self):
        """Smoke test: experiment page renders without error."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_invalid_experiment_name(self):
        """Invalid filename characters in experiment name show an error."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.text_input(key="experiment_name").set_value("test<>name").run()
        assert any("not allowed" in e.value for e in at.error)

    @pytest.mark.usefixtures("_app")
    def test_toggle_full_history_off(self):
        """Disabling full_history shows date pickers."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="full_history").set_value(False).run()
        assert not at.exception
        assert len(at.date_input) >= 2

    @pytest.mark.usefixtures("_app")
    def test_toggle_margin_off(self):
        """Disabling margin hides leverage/margin fields."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="allow_margin").set_value(False).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "max_leverage" not in keys

    @pytest.mark.usefixtures("_app")
    def test_toggle_short_selling_off(self):
        """Disabling short selling hides borrow rate."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="allow_short_selling").set_value(False).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "borrow_rate" not in keys

    @pytest.mark.usefixtures("_app")
    def test_commission_fixed(self):
        """Switching commission type to Fixed shows fixed input."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.radio(key="commission_type").set_value(CommissionType("Fixed")).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "commission_fixed" in keys

    @pytest.mark.usefixtures("_app")
    def test_commission_percentage_plus_fixed(self):
        """Switching to PercentagePlusFixed shows both commission inputs."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.radio(key="commission_type").set_value(CommissionType("PercentagePlusFixed")).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "commission_pct" in keys
        assert "commission_fixed" in keys

    @pytest.mark.usefixtures("_app")
    def test_conversion_hold_until_threshold(self):
        """HoldUntilThreshold mode shows threshold input."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
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
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.selectbox(key="conversion_mode").set_value(CurrencyConversionMode("EndOfPeriod")).run()
        assert not at.exception
        keys = [s.key for s in at.selectbox]
        assert "conversion_period" in keys

    @pytest.mark.usefixtures("_app")
    def test_conversion_custom_interval(self):
        """CustomInterval mode shows interval number input."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.selectbox(key="conversion_mode").set_value(
            CurrencyConversionMode("CustomInterval")
        ).run()
        assert not at.exception
        keys = [n.key for n in at.number_input]
        assert "conversion_interval" in keys

    @pytest.mark.usefixtures("_app")
    def test_add_strategy_button(self):
        """Clicking 'Create new strategy' navigates to the strategies page."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.button[0].click().run()
        # switch_page raises in AppTest since strategies.py is not a registered page
        assert any("strategies" in str(e.value).lower() for e in at.exception)

    @pytest.mark.usefixtures("_app")
    def test_add_indicator_button(self):
        """Clicking 'Add indicator' navigates to the indicators page."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.button[1].click().run()
        # switch_page raises in AppTest since indicators.py is not a registered page
        assert any("indicators" in str(e.value).lower() for e in at.exception)

    @pytest.mark.usefixtures("_app")
    def test_description_text_area(self):
        """Setting description text area works."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.text_area[0].set_value("My test description").run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_number_input_initial_cash(self):
        """Changing initial cash value works."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.number_input(key="initial_cash").set_value(50000).run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_toggle_trade_on_close(self):
        """Toggling trade_on_close works."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="trade_on_close").set_value(True).run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_config_upload_success(self):
        """Config import success message is shown."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.session_state["_import_success"] = "Loaded config."
        at.run()
        assert any("Loaded" in s.value for s in at.success)

    @pytest.mark.usefixtures("_app")
    def test_config_upload_error(self):
        """Config import error message is shown."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.session_state["_import_error"] = "Failed to parse."
        at.run()
        assert any("Failed" in e.value for e in at.error)

    @pytest.mark.usefixtures("_app")
    def test_invalid_tag(self):
        """Invalid tags show an error."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.session_state["tags"] = ["bad<>tag"]
        at.run()
        assert any("Invalid tag" in e.value for e in at.error)

    @pytest.mark.usefixtures("_app")
    def test_strategy_selected(self):
        """Selecting saved strategies shows labels."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        # strategies multiselect only appears when there are saved strategies
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_current_tab_restored(self):
        """Setting current_tab restores tab selection."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.session_state["current_tab"] = 1
        at.run()
        assert not at.exception

    @pytest.mark.usefixtures("_app")
    def test_use_storage_toggle(self):
        """Toggling use_storage works."""
        at = AppTest.from_file("src/backtide/ui/experiment.py", default_timeout=30)
        at.run()
        at.toggle(key="use_storage").set_value(True).run()
        assert not at.exception
