"""Backtide.

Author: Mavs
Description: Unit tests for utility functions and constants.

"""

import pytest

from backtide.utils.constants import (
    INVALID_FILENAME_CHARS,
    MAX_INSTRUMENT_SELECTION,
    MAX_PRELOADED_INSTRUMENTS,
    MOMENT_TO_STRFTIME,
    TAG_PATTERN,
)
from backtide.utils.enum import CaseInsensitiveEnum
from backtide.utils.utils import _format_compact, _to_list


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
        assert _format_compact(n) == expected

    def test_negative(self):
        """Negative numbers use the same brackets."""
        assert "M" in _format_compact(-10_000_000)


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
        assert self._Color("red") == self._Color.Red
        assert self._Color("RED") == self._Color.Red
        assert self._Color("Red") == self._Color.Red

    def test_repr(self):
        assert repr(self._Color.Red) == "Red"

    def test_missing_raises(self):
        with pytest.raises(ValueError, match="has no member"):
            self._Color("yellow")


# ─────────────────────────────────────────────────────────────────────────────
# Constants
# ─────────────────────────────────────────────────────────────────────────────


class TestConstants:
    """Tests for shared constants."""

    @pytest.mark.parametrize("tag", ["my-tag", "tag 1", "hello_world"])
    def test_tag_pattern_match(self, tag):
        assert TAG_PATTERN.match(tag)

    @pytest.mark.parametrize("tag", ["", "a" * 21, "tag<>"])
    def test_tag_pattern_no_match(self, tag):
        assert not TAG_PATTERN.match(tag)

    def test_invalid_filename_chars(self):
        assert INVALID_FILENAME_CHARS.search("<>:")
        assert not INVALID_FILENAME_CHARS.search("hello")

    def test_moment_to_strftime_has_entries(self):
        assert "YYYY" in MOMENT_TO_STRFTIME
        assert MOMENT_TO_STRFTIME["YYYY"] == "%Y"
        assert len(MOMENT_TO_STRFTIME) > 10

    def test_max_constants(self):
        assert MAX_INSTRUMENT_SELECTION == 10
        assert MAX_PRELOADED_INSTRUMENTS == 1500


# ─────────────────────────────────────────────────────────────────────────────
# init_logging / clear_cache
# ─────────────────────────────────────────────────────────────────────────────


class TestCoreUtils:
    """Tests for core utils functions."""

    def test_init_logging_idempotent(self):
        """init_logging can be called multiple times without error."""
        from backtide.core.utils import init_logging

        init_logging("warn")
        init_logging("warn")  # second call is a no-op

    def test_clear_cache(self):
        """clear_cache runs without error."""
        from backtide.core.utils import clear_cache

        clear_cache()

