"""Cross-platform launcher for cargo commands used by tox.

PyO3-linked binaries (benchmarks, llvm-cov-instrumented tests) need to
locate the Python shared library and installed Python packages at runtime.
The lookup directories differ by platform, so this launcher patches the
appropriate environment variables before invoking `cargo`:

* **POSIX** -- prepend `sysconfig['LIBDIR']` (which holds `libpython`) to
  both `LD_LIBRARY_PATH` (runtime loader) and `LIBRARY_PATH` (link-time
  search, so the linker can resolve `-lpythonX.Y`).
* **Windows** -- prepend the base interpreter directory (`sys.base_prefix`,
  where `pythonXY.dll` lives) to `PATH`. Without this, cargo-spawned bench/test
  `.exe` files fail with `STATUS_DLL_NOT_FOUND` because the venv's `Scripts`
  dir does not contain the DLL.
* **All platforms** -- prepend the active interpreter's `purelib` and `platlib`
  directories to `PYTHONPATH` so embedded PyO3 tests can import dependencies.

Usage:
    python scripts/run_cargo.py cargo bench --manifest-path ...
    python scripts/run_cargo.py cargo llvm-cov --manifest-path ...

"""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import sysconfig


def _prepend(env: dict[str, str], key: str, value: str) -> None:
    """Prepend `value` to the `os.pathsep`-separated variable `key` in `env`.

    If `key` is unset or empty, it is set to `value`. Otherwise, `value`
    is placed before the existing contents, separated by `os.pathsep`
    (`;` on Windows, `:` on POSIX). The mapping is mutated in place.

    Parameters
    ----------
    env : dict[str, str]
        Environment mapping to mutate (typically a copy of `os.environ`).

    key : str
        Name of the environment variable to update (e.g., `"PATH"`).

    value : str
        Path or token to prepend to the variable's current value.

    """
    existing = env.get(key, "")
    env[key] = f"{value}{os.pathsep}{existing}" if existing else value


def _get_environment_value(env: dict[str, str], key: str) -> str | None:
    """Return an environment value using Windows-compatible key matching."""
    folded_key = key.casefold()
    return next((value for name, value in env.items() if name.casefold() == folded_key), None)


def _configure_windows_msvc(env: dict[str, str]) -> None:
    """Load the latest stable Visual Studio C++ developer environment.

    Cargo fingerprints native build scripts using compiler and SDK environment
    variables. Loading one stable MSVC environment here prevents inherited
    ``CC``/``CXX`` values from alternating between Visual Studio installations
    and repeatedly invalidating large native dependencies such as DuckDB.

    Parameters
    ----------
    env : dict[str, str]
        Child-process environment to update in place.

    """
    program_files = _get_environment_value(env, "ProgramFiles(x86)")
    if not program_files:
        return

    vswhere = Path(program_files) / "Microsoft Visual Studio" / "Installer" / "vswhere.exe"
    if not vswhere.is_file():
        return

    query = subprocess.run(
        [
            str(vswhere),
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ],
        check=True,
        capture_output=True,
        env=env,
        text=True,
    )
    installation = query.stdout.strip()
    if not installation:
        return

    developer_command = Path(installation) / "Common7" / "Tools" / "VsDevCmd.bat"
    if not developer_command.is_file():
        return

    configured = subprocess.run(
        [
            "cmd.exe",
            "/d",
            "/c",
            str(developer_command),
            "-no_logo",
            "-arch=x64",
            "-host_arch=x64",
            "&&",
            "set",
        ],
        check=True,
        capture_output=True,
        env=env,
        text=True,
    )
    for line in configured.stdout.splitlines():
        key, separator, value = line.partition("=")
        if separator and key:
            env[key] = value

    tools_directory = _get_environment_value(env, "VCToolsInstallDir")
    if not tools_directory:
        return
    compiler = Path(tools_directory) / "bin" / "HostX64" / "x64" / "cl.exe"
    if compiler.is_file():
        env["CC"] = str(compiler)
        env["CXX"] = str(compiler)


def main(argv: list[str]) -> int:
    """Run a cargo command with the Python shared library on the loader path.

    Builds an environment that lets PyO3-linked cargo artifacts (benchmarks,
    llvm-cov-instrumented tests) locate the Python shared library at runtime,
    then dispatches `argv` via :func:`subprocess.call`.

    The active interpreter's package directories are prepended to
    `PYTHONPATH`. On Windows, the base interpreter directory (where
    `pythonXY.dll` lives) and the directory of `sys.executable` are prepended
    to `PATH`. On POSIX, `sysconfig['LIBDIR']` (where `libpython` lives) is
    prepended to both `LD_LIBRARY_PATH` (runtime) and `LIBRARY_PATH`
    (link-time).

    Parameters
    ----------
    argv : list[str]
        Full command vector to execute. Must be non-empty.

    Returns
    -------
    int
        Exit code from the spawned process. Returns `2` if `argv` is
        empty and `127` if the executable cannot be found.

    """
    if not argv:
        print("run_cargo.py: missing command", file=sys.stderr)
        return 2

    env = os.environ.copy()
    if os.name == "nt":
        _configure_windows_msvc(env)
    package_dirs = dict.fromkeys(sysconfig.get_paths().get(key) for key in ("purelib", "platlib"))
    for package_dir in package_dirs:
        if package_dir and os.path.isdir(package_dir):
            _prepend(env, "PYTHONPATH", package_dir)

    if os.name == "nt":
        # `pythonXY.dll` lives next to the base interpreter; venvs only
        # contain a launcher in `Scripts/`, so the DLL is not on PATH by
        # default for child processes.
        for candidate in (sys.base_prefix, os.path.dirname(sys.executable)):
            if candidate and os.path.isdir(candidate):
                _prepend(env, "PATH", candidate)
    else:
        # `get_config_vars(*names)` returns a *list* of values; iterating it
        # directly would prepend a Python repr (e.g. "['/.../lib']") to
        # `LD_LIBRARY_PATH`, which the loader silently ignores.
        for libdir in sysconfig.get_config_vars("LIBDIR", "LIBPL"):
            if libdir and os.path.isdir(libdir):
                # LD_LIBRARY_PATH  -> runtime loader (needed to *run* the binary)
                # LIBRARY_PATH     -> link-time search (needed to *find* -lpythonX.Y)
                _prepend(env, "LD_LIBRARY_PATH", libdir)
                _prepend(env, "LIBRARY_PATH", libdir)

    try:
        return subprocess.call(argv, env=env)
    except FileNotFoundError as exc:
        print(f"run_cargo.py: {exc}", file=sys.stderr)
        return 127


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
