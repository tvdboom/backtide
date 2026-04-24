"""Backtide.

Author: Mavs
Description: Unit tests for utility functions and constants.

"""

import pytest

from backtide.utils import clear_cache, init_logging
from backtide.utils.enum import CaseInsensitiveEnum
from backtide.utils.utils import _format_number, _to_list

# ─────────────────────────────────────────────────────────────────────────────
# _to_list
# ─────────────────────────────────────────────────────────────────────────────


class TestToList:
    """Tests for the _to_list helper."""

    def test_string(self):
        """A string is wrapped in a list (not iterated)."""
        assert _to_list("hello") == ["hello"]

    def test_list_passthrough(self):
        """A list is returned as-is."""
        assert _to_list([1, 2, 3]) == [1, 2, 3]

    def test_single_object(self):
        """A non-iterable object is wrapped."""
        assert _to_list(42) == [42]

    def test_tuple(self):
        """A tuple is converted to a list."""
        assert _to_list((1, 2)) == [1, 2]

    def test_generator(self):
        """A generator is consumed into a list."""
        assert _to_list(x for x in range(3)) == [0, 1, 2]

    def test_bytes(self):
        """Bytes are wrapped (not iterated)."""
        assert _to_list(b"abc") == [b"abc"]


# ─────────────────────────────────────────────────────────────────────────────
# _format_compact
# ─────────────────────────────────────────────────────────────────────────────


class TestFormatCompact:
    """Tests for the _format_compact formatter."""

    @pytest.mark.parametrize(
        ("n", "expected"),
        [
            (0, "0"),
            (999, "999"),
            (1_500, "1.5k"),
            (10_000, "10k"),
            (50_000, "50k"),
            (1_500_000, "1.5M"),
            (10_000_000, "10M"),
            (50_000_000, "50M"),
        ],
    )
    def test_magnitude(self, n, expected):
        """Each magnitude bracket formats correctly."""
        assert _format_number(n) == expected

    def test_negative(self):
        """Negative numbers use the same brackets."""
        assert "M" in _format_number(-10_000_000)


# ─────────────────────────────────────────────────────────────────────────────
# CaseInsensitiveEnum
# ─────────────────────────────────────────────────────────────────────────────


class TestCaseInsensitiveEnum:
    """Tests for the CaseInsensitiveEnum base class."""

    class _Color(CaseInsensitiveEnum):
        Red = 1
        Green = 2
        Blue = 3

    def test_case_insensitive(self):
        """Case-insensitive lookup works for all casings."""
        assert self._Color("red") == self._Color.Red
        assert self._Color("RED") == self._Color.Red
        assert self._Color("Red") == self._Color.Red

    def test_repr(self):
        """Repr returns the member name."""
        assert repr(self._Color.Red) == "Red"

    def test_missing_raises(self):
        """Unknown member raises ValueError."""
        with pytest.raises(ValueError, match="has no member"):
            self._Color("yellow")


# ─────────────────────────────────────────────────────────────────────────────
# init_logging / clear_cache
# ─────────────────────────────────────────────────────────────────────────────


class TestCoreUtils:
    """Tests for core utils functions."""

    def test_init_logging_idempotent(self):
        """init_logging can be called multiple times without error."""
        init_logging("warn")
        init_logging("warn")  # second call is a no-op

    def test_clear_cache(self):
        """clear_cache runs without error."""
        clear_cache()
