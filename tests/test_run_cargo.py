"""Backtide.

Author: Mavs
Description: Tests for the cross-platform Cargo launcher.

"""

from __future__ import annotations

from pathlib import Path

from scripts import run_cargo


class TestRunCargo:
    """Test the Cargo subprocess environment."""

    def test_adds_active_python_packages_to_pythonpath(self, monkeypatch, tmp_path: Path):
        """Expose active Python packages to embedded PyO3 tests."""
        purelib = tmp_path / "purelib"
        platlib = tmp_path / "platlib"
        purelib.mkdir()
        platlib.mkdir()
        captured = {}

        monkeypatch.setattr(
            run_cargo.sysconfig,
            "get_paths",
            lambda: {"purelib": str(purelib), "platlib": str(platlib)},
        )

        def call(argv, env):
            captured.update(argv=argv, env=env)
            return 0

        monkeypatch.setattr(run_cargo.subprocess, "call", call)

        result = run_cargo.main(["cargo", "test"])

        assert result == 0
        assert captured["argv"] == ["cargo", "test"]
        python_paths = captured["env"]["PYTHONPATH"].split(run_cargo.os.pathsep)
        assert python_paths[:2] == [str(platlib), str(purelib)]
