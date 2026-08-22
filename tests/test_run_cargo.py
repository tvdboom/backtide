"""Backtide.

Author: Mavs
Description: Tests for the cross-platform Cargo launcher.

"""

from __future__ import annotations

import os
from pathlib import Path
from subprocess import CompletedProcess

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
        monkeypatch.setattr(run_cargo, "_configure_windows_msvc", lambda _env: None)

        result = run_cargo.main(["cargo", "test"])

        assert result == 0
        assert captured["argv"] == ["cargo", "test"]
        python_paths = captured["env"]["PYTHONPATH"].split(run_cargo.os.pathsep)
        assert python_paths[:2] == [str(platlib), str(purelib)]

    def test_windows_uses_latest_stable_visual_studio_environment(
        self,
        monkeypatch,
        tmp_path: Path,
    ):
        """Replace inherited compiler paths with the selected stable MSVC toolset."""
        program_files = tmp_path / "Program Files (x86)"
        vswhere = program_files / "Microsoft Visual Studio" / "Installer" / "vswhere.exe"
        vswhere.parent.mkdir(parents=True)
        vswhere.touch()
        installation = tmp_path / "Microsoft Visual Studio" / "Community"
        developer_command = installation / "Common7" / "Tools" / "VsDevCmd.bat"
        developer_command.parent.mkdir(parents=True)
        developer_command.touch()
        tools_directory = installation / "VC" / "Tools" / "MSVC" / "14.52.36615"
        compiler = tools_directory / "bin" / "HostX64" / "x64" / "cl.exe"
        compiler.parent.mkdir(parents=True)
        compiler.touch()
        calls = []

        def run(argv, **kwargs):
            calls.append((argv, kwargs))
            if Path(argv[0]) == vswhere:
                return CompletedProcess(argv, 0, stdout=str(installation), stderr="")
            output = os.linesep.join(
                [
                    f"VCToolsInstallDir={tools_directory}{os.sep}",
                    "INCLUDE=C:\\SDK\\include",
                    "LIB=C:\\SDK\\lib",
                ]
            )
            return CompletedProcess(argv, 0, stdout=output, stderr="")

        monkeypatch.setattr(run_cargo.subprocess, "run", run)
        env = {
            "PROGRAMFILES(X86)": str(program_files),
            "CC": "C:\\Visual Studio\\Insiders\\cl.exe",
            "CXX": "C:\\Visual Studio\\Insiders\\cl.exe",
        }

        run_cargo._configure_windows_msvc(env)

        assert len(calls) == 2
        assert "-prerelease" not in calls[0][0]
        assert env["CC"] == str(compiler)
        assert env["CXX"] == str(compiler)
        assert env["INCLUDE"] == "C:\\SDK\\include"
        assert env["LIB"] == "C:\\SDK\\lib"
