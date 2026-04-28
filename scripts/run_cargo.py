"""Cross-platform launcher for cargo commands used by tox.

PyO3-linked binaries (benchmarks, llvm-cov-instrumented tests) need to locate
the Python shared library at runtime. The lookup directory differs by
platform, so this launcher patches the appropriate environment variable
before invoking ``cargo``:

* **POSIX** -- prepend ``sysconfig['LIBDIR']`` (which holds ``libpython``) to
  ``LD_LIBRARY_PATH``.
* **Windows** -- prepend the base interpreter directory (``sys.base_prefix``,
  where ``pythonXY.dll`` lives) to ``PATH``. Without this, cargo-spawned
  bench/test ``.exe`` files fail with ``STATUS_DLL_NOT_FOUND`` (0xc0000135)
  because the venv's ``Scripts\\`` dir does not contain the DLL.

Usage:
    python scripts/run_cargo.py cargo bench --manifest-path ...
    python scripts/run_cargo.py cargo llvm-cov --manifest-path ...

"""

from __future__ import annotations

import os
import subprocess
import sys
import sysconfig


def _prepend(env: dict[str, str], key: str, value: str) -> None:
    existing = env.get(key, "")
    env[key] = f"{value}{os.pathsep}{existing}" if existing else value


def main(argv: list[str]) -> int:
    if not argv:
        print("run_cargo.py: missing command", file=sys.stderr)
        return 2

    env = os.environ.copy()
    if os.name == "nt":
        # `pythonXY.dll` lives next to the base interpreter; venvs only
        # contain a launcher in `Scripts/`, so the DLL is not on PATH by
        # default for child processes.
        for candidate in (sys.base_prefix, os.path.dirname(sys.executable)):
            if candidate and os.path.isdir(candidate):
                _prepend(env, "PATH", candidate)
    else:
        if libdir := sysconfig.get_config_var("LIBDIR"):
            _prepend(env, "LD_LIBRARY_PATH", libdir)

    try:
        return subprocess.call(argv, env=env)
    except FileNotFoundError as exc:
        print(f"run_cargo.py: {exc}", file=sys.stderr)
        return 127


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
