"""Backtide.

Author: Mavs
Description: Tests for the Rust coverage runner.

"""

from __future__ import annotations

from pathlib import Path
from subprocess import CompletedProcess

from scripts import rust_coverage


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
