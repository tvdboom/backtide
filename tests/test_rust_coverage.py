"""Backtide.

Author: Mavs
Description: Tests for the Rust coverage runner.

"""

from __future__ import annotations

from pathlib import Path
from subprocess import CompletedProcess
import sys

import pytest

from scripts import rust_coverage


class TestCoverageArguments:
    """Test resource bounds for native and Python coverage workers."""

    @pytest.mark.parametrize(
        ("cpu_count", "expected"), [(None, 1), (1, 1), (2, 2), (4, 4), (12, 12), (64, 12)]
    )
    def test_default_workers_match_available_cpus(
        self, monkeypatch: pytest.MonkeyPatch, cpu_count: int | None, expected: int
    ) -> None:
        """Keep coverage builds within the runner's CPU capacity and worker limit."""
        monkeypatch.setattr(rust_coverage.os, "cpu_count", lambda: cpu_count)
        monkeypatch.setattr(sys, "argv", ["rust_coverage.py"])

        args = rust_coverage._parse_args()

        assert args.workers == expected

    def test_explicit_workers_override_cpu_default(self, monkeypatch: pytest.MonkeyPatch) -> None:
        """Preserve explicit parallelism requested by the developer."""
        monkeypatch.setattr(rust_coverage.os, "cpu_count", lambda: 2)
        monkeypatch.setattr(sys, "argv", ["rust_coverage.py", "--workers", "8"])

        args = rust_coverage._parse_args()

        assert args.workers == 8


class TestCoverageEnvironment:
    """Test coverage compiler environment configuration."""

    def test_decodes_shell_escaped_values(self, monkeypatch, tmp_path: Path):
        """Remove shell quoting without changing environment value contents."""
        output = r"""export RUSTFLAGS='-C instrument-coverage'
export LLVM_PROFILE_FILE='/tmp/backtide coverage-%p.profraw'
export CARGO_TARGET_DIR='C:\repos\backtide\target'
export ASSIGNMENT='coverage=enabled'
export EMPTY=''"""
        calls = []

        def run(command, **_kwargs):
            calls.append([str(part) for part in command])
            stdout = output if "show-env" in command else ""
            return CompletedProcess(command, 0, stdout=stdout, stderr="")

        monkeypatch.setattr(rust_coverage, "_run", run)

        configured = rust_coverage._coverage_environment(
            tmp_path,
            tmp_path / "Cargo.toml",
            {"EXISTING": "preserved"},
        )

        assert "--sh" in calls[1]
        assert configured == {
            "ASSIGNMENT": "coverage=enabled",
            "CARGO_TARGET_DIR": "C:\\repos\\backtide\\target",
            "EMPTY": "",
            "EXISTING": "preserved",
            "LLVM_PROFILE_FILE": "/tmp/backtide coverage-%p.profraw",
            "RUSTFLAGS": "-C instrument-coverage",
        }
