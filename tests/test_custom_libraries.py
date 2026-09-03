"""Backtide.

Author: Mavs
Description: Tests for shared custom-extension library behavior.

"""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast

import pytest

from backtide.indicators import BaseIndicator
from backtide.indicators.utils import (
    _build_custom_indicator,
    _check_indicator_code,
    _get_indicator_label,
    _is_builtin_indicator,
    _load_stored_indicators,
    _save_indicator,
)
from backtide.metrics import BaseMetric
from backtide.metrics.utils import _build_custom_metric, _load_stored_metrics, _save_metric
from backtide.sizers import BaseSizer
from backtide.sizers.utils import (
    _build_custom_sizer,
    _check_sizer_code,
    _load_stored_sizers,
    _restore_sizer,
    _save_sizer,
)
from backtide.strategies import BaseStrategy, BuyAndHold
from backtide.strategies.utils import (
    _build_custom_strategy,
    _check_strategy_code,
    _get_strategy_label,
    _is_builtin_strategy,
    _load_stored_strategies,
    _resolve_auto_indicators,
    _save_strategy,
)
from backtide.utils.library import _save_pickle

Builder = Callable[[str], Any]
Saver = Callable[[Any, str, Any], None]
Loader = Callable[[Any], dict[str, Any]]

_CUSTOM_TYPES: list[tuple[Builder, Saver, Loader, type[Any], str, str]] = [
    (
        _build_custom_strategy,
        _save_strategy,
        _load_stored_strategies,
        BaseStrategy,
        "strategies",
        """from backtide.strategies import BaseStrategy

class CustomType(BaseStrategy):
    calls = 0

    def __init__(self):
        type(self).calls += 1

    def evaluate(self, data, portfolio, state, indicators):
        return []

CustomType()
""",
    ),
    (
        _build_custom_indicator,
        _save_indicator,
        _load_stored_indicators,
        BaseIndicator,
        "indicators",
        """from backtide.indicators import BaseIndicator

class CustomType(BaseIndicator):
    calls = 0

    def __init__(self):
        type(self).calls += 1

    def compute(self, data):
        return data

CustomType()
""",
    ),
    (
        _build_custom_sizer,
        _save_sizer,
        _load_stored_sizers,
        BaseSizer,
        "sizers",
        """from backtide.sizers import BaseSizer

class CustomType(BaseSizer):
    calls = 0

    def __init__(self):
        type(self).calls += 1

    def calculate(self, equity, price, stop_distance=None, atr=None):
        return 1.0

CustomType()
""",
    ),
    (
        _build_custom_metric,
        _save_metric,
        _load_stored_metrics,
        BaseMetric,
        "metrics",
        """from backtide.metrics import BaseMetric

class CustomType(BaseMetric):
    '''Return a constant metric.'''

    calls = 0

    def __init__(self):
        type(self).calls += 1

    def compute(self, equity_curve, trades):
        return 1.0

CustomType()
""",
    ),
]


def _config(path: Path) -> Any:
    """Return the minimal configuration used by library persistence helpers."""
    return SimpleNamespace(data=SimpleNamespace(storage_path=path))


class TestCustomLibrary:
    """Tests for behavior shared by all custom extension types."""

    @pytest.mark.parametrize(
        ("builder", "_save", "_load", "_base", "_folder", "source"), _CUSTOM_TYPES
    )
    def test_final_expression_is_evaluated_once(
        self,
        builder: Builder,
        _save: Saver,
        _load: Loader,
        _base: type[Any],
        _folder: str,
        source: str,
    ) -> None:
        """Building custom source invokes its constructor exactly once."""
        instance = builder(source)

        assert type(instance).calls == 1
        assert instance._source_code == source

    @pytest.mark.parametrize(
        ("builder", "_save", "_load", "base", "_folder", "source"), _CUSTOM_TYPES
    )
    def test_saved_instance_round_trips(
        self,
        tmp_path: Path,
        builder: Builder,
        _save: Saver,
        _load: Loader,
        base: type[Any],
        _folder: str,
        source: str,
    ) -> None:
        """Every custom library uses the same durable persistence contract."""
        config = _config(tmp_path)
        instance = builder(source)

        _save(instance, "Saved", config)
        loaded = _load(config)

        assert isinstance(loaded["Saved"], base)
        assert loaded["Saved"]._source_code == source

    @pytest.mark.parametrize(
        ("builder", "save", "load", "_base", "folder", "source"), _CUSTOM_TYPES
    )
    def test_corrupt_entry_does_not_hide_valid_entries(
        self,
        tmp_path: Path,
        caplog: pytest.LogCaptureFixture,
        builder: Builder,
        save: Saver,
        load: Loader,
        _base: type[Any],
        folder: str,
        source: str,
    ) -> None:
        """A corrupt pickle is logged and isolated from other saved values."""
        directory = tmp_path / folder
        directory.mkdir()
        (directory / "broken.pkl").write_bytes(b"not a pickle")
        save(builder(source), "valid", _config(tmp_path))

        assert list(load(_config(tmp_path))) == ["valid"]
        assert "Failed to load" in caplog.text

    @pytest.mark.parametrize("builder", [case[0] for case in _CUSTOM_TYPES])
    def test_final_statement_must_be_an_expression(self, builder: Builder) -> None:
        """Custom source rejects a definition without a final instance."""
        with pytest.raises(ValueError, match="last statement"):
            builder("value = 1")

    @pytest.mark.parametrize("builder", [case[0] for case in _CUSTOM_TYPES])
    def test_final_expression_must_have_the_expected_type(self, builder: Builder) -> None:
        """Custom source rejects a final expression of an unrelated type."""
        with pytest.raises(TypeError, match="Expected"):
            builder("object()")

    def test_failed_save_preserves_previous_file(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        """An interrupted serialization leaves the previous value intact."""
        target = tmp_path / "saved.pkl"
        target.write_bytes(b"previous")

        def fail_dump(_value: Any, _stream: Any) -> None:
            raise OSError("disk full")

        monkeypatch.setattr("backtide.utils.library.cloudpickle.dump", fail_dump)

        with pytest.raises(OSError, match="disk full"):
            _save_pickle("new", tmp_path, "saved", temporary_prefix=".test-")

        assert target.read_bytes() == b"previous"
        assert list(tmp_path.glob("*.tmp")) == []

    def test_empty_sizer_file_is_removed(self, tmp_path: Path) -> None:
        """Legacy zero-byte sizer files are cleaned up during loading."""
        path = tmp_path / "sizers" / "empty.pkl"
        path.parent.mkdir()
        path.touch()

        assert _load_stored_sizers(_config(tmp_path)) == {}
        assert not path.exists()


class TestCustomLibraryValidation:
    """Tests for extension-specific source validation and labels."""

    indicator_template = """from backtide.indicators import BaseIndicator

class CustomIndicator(BaseIndicator):
    def compute(self, data):
        {body}

CustomIndicator()
"""
    strategy_template = """from backtide.strategies import BaseStrategy

class CustomStrategy(BaseStrategy):
    def evaluate(self, data, portfolio, state, indicators):
        {body}

CustomStrategy()
"""
    sizer_template = """from backtide.sizers import BaseSizer

class CustomSizer(BaseSizer):
    def calculate(self, equity, price, stop_distance, atr):
        {body}

CustomSizer()
"""

    @pytest.mark.parametrize(
        ("code", "message"),
        [
            ("class Broken(", "Syntax error"),
            ("raise RuntimeError('no instance')\nobject()", "Failed to instantiate indicator"),
            (
                (
                    "from backtide.indicators import BaseIndicator\n"
                    "class Wrong(BaseIndicator):\n"
                    "    def compute(self): return 1\n"
                    "Wrong()"
                ),
                "doesn't have signature",
            ),
            (
                indicator_template.format(body="raise LookupError('missing')"),
                "LookupError: missing",
            ),
            (indicator_template.format(body="return None"), "returned `None`"),
        ],
    )
    def test_indicator_validation_reports_invalid_source(self, code: str, message: str) -> None:
        """Indicator validation reports each public contract failure."""
        from backtide.config import get_config

        assert message in str(_check_indicator_code(code, get_config()))

    def test_indicator_validation_accepts_a_result(self) -> None:
        """Indicator validation accepts the required signature and a non-null result."""
        from backtide.config import get_config

        code = self.indicator_template.format(body="return data['close']")

        assert _check_indicator_code(code, get_config()) is None

    @pytest.mark.parametrize(
        ("code", "message"),
        [
            ("class Broken(", "Syntax error"),
            ("raise RuntimeError('no instance')\nobject()", "Failed to instantiate strategy"),
            (
                (
                    "from backtide.strategies import BaseStrategy\n"
                    "class Wrong(BaseStrategy):\n"
                    "    def evaluate(self, data): return []\n"
                    "Wrong()"
                ),
                "doesn't have signature",
            ),
            (strategy_template.format(body="data = data"), "must return a list"),
            (strategy_template.format(body="return"), "not None"),
            (strategy_template.format(body="return 3"), "not a constant"),
        ],
    )
    def test_strategy_validation_reports_invalid_source(self, code: str, message: str) -> None:
        """Strategy validation reports malformed source and return contracts."""
        assert message in str(_check_strategy_code(code))

    def test_strategy_validation_accepts_list_return(self) -> None:
        """Strategy validation accepts a list-returning evaluate method."""
        assert _check_strategy_code(self.strategy_template.format(body="return []")) is None

    @pytest.mark.parametrize(
        ("code", "message"),
        [
            ("object()", "Failed to instantiate sizer"),
            (
                (
                    "from backtide.sizers import BaseSizer\n"
                    "class Wrong(BaseSizer):\n"
                    "    def calculate(self, equity): return 1\n"
                    "Wrong()"
                ),
                "must have parameters",
            ),
            (sizer_template.format(body="raise ArithmeticError('bad input')"), "ArithmeticError"),
            (sizer_template.format(body="return float('inf')"), "finite number"),
        ],
    )
    def test_sizer_validation_reports_invalid_source(self, code: str, message: str) -> None:
        """Sizer validation reports construction, signature, and numeric failures."""
        assert message in str(_check_sizer_code(code))

    def test_sizer_validation_accepts_finite_quantity(self) -> None:
        """Sizer validation accepts a finite numeric result."""
        assert _check_sizer_code(self.sizer_template.format(body="return 2")) is None

    def test_builtin_and_custom_labels_are_distinct(self) -> None:
        """Library labels expose built-in metadata and mark custom assets."""
        from backtide.indicators import SimpleMovingAverage

        custom_indicator = _build_custom_indicator(
            self.indicator_template.format(body="return data['close']")
        )
        custom_strategy = _build_custom_strategy(self.strategy_template.format(body="return []"))
        builtin_indicator = cast(BaseIndicator, SimpleMovingAverage(5))

        assert _is_builtin_indicator(builtin_indicator) is True
        assert "period=5" in _get_indicator_label("SMA", builtin_indicator)
        assert _is_builtin_indicator(custom_indicator) is False
        assert "Custom" in _get_indicator_label("Mine", custom_indicator)
        assert _is_builtin_strategy(BuyAndHold()) is True
        assert "Single Asset" in _get_strategy_label("Hold", BuyAndHold())
        assert _is_builtin_strategy(custom_strategy) is False
        assert "Custom" in _get_strategy_label("Mine", custom_strategy)

    def test_auto_indicators_are_deduplicated_and_missing_hooks_are_skipped(
        self,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        """Automatic indicator discovery keeps the first deterministic requirement."""
        first = object()
        second = object()

        class WithIndicators:
            name = "Source"

            @staticmethod
            def required_indicators() -> list[object]:
                return [first, second]

        monkeypatch.setattr(
            "backtide.strategies.utils._indicator_deterministic_name",
            lambda _indicator: "same",
        )

        assert _resolve_auto_indicators(
            [object(), WithIndicators(), SimpleNamespace(required_indicators=[])]
        ) == [("same", first, "Source")]

    @pytest.mark.parametrize(
        ("value", "message"),
        [
            (
                {"format": "backtide.builtin-sizer.v1", "type": "Missing", "parameters": {}},
                "Unknown built-in sizer",
            ),
            (
                {
                    "format": "backtide.builtin-sizer.v1",
                    "type": "FixedQuantity",
                    "parameters": [],
                },
                "parameters must be a mapping",
            ),
        ],
    )
    def test_builtin_sizer_restore_rejects_invalid_records(
        self,
        value: dict[str, Any],
        message: str,
    ) -> None:
        """Stable built-in sizer records reject unknown types and malformed parameters."""
        with pytest.raises(ValueError, match=message):
            _restore_sizer(value)
