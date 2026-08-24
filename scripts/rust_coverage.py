"""Measure Rust coverage across native and Python integration tests.

Cargo unit tests cannot exercise most PyO3 wrappers. This runner builds an
instrumented wheel, executes both test suites against it, and merges the
resulting profiles into one LCOV report.

Run this script through ``scripts/run_cargo.py`` so native compiler and Python
library paths are configured on every platform.

"""

from __future__ import annotations

import argparse
from collections.abc import Sequence
import json
import logging
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile

LOGGER = logging.getLogger("rust-coverage")


def _parse_shell_environment(output: str) -> dict[str, str]:
    """Parse POSIX shell exports emitted by ``cargo llvm-cov show-env --sh``."""
    parsed = {}
    for line in output.splitlines():
        if not line:
            continue
        try:
            tokens = shlex.split(line, comments=False, posix=True)
        except ValueError as exc:
            raise RuntimeError(f"Invalid cargo llvm-cov environment line: {line!r}.") from exc
        if len(tokens) != 2 or tokens[0] != "export":
            raise RuntimeError(f"Invalid cargo llvm-cov environment line: {line!r}.")
        key, separator, value = tokens[1].partition("=")
        if not separator or not key:
            raise RuntimeError(f"Invalid cargo llvm-cov environment line: {line!r}.")
        parsed[key] = value
    return parsed


def _run(
    command: Sequence[str | Path],
    *,
    cwd: Path,
    env: dict[str, str],
    capture_output: bool = False,
) -> subprocess.CompletedProcess[str]:
    """Run one checked subprocess and log its command."""
    args = [str(part) for part in command]
    rendered = subprocess.list2cmdline(args)
    if len(rendered) > 500:
        rendered = f"{subprocess.list2cmdline(args[:6])} ... ({len(args) - 6} more arguments)"
    LOGGER.info("Running: %s", rendered)
    return subprocess.run(
        args,
        check=True,
        capture_output=capture_output,
        cwd=cwd,
        env=env,
        text=True,
    )


def _coverage_environment(root: Path, manifest: Path, env: dict[str, str]) -> dict[str, str]:
    """Return Cargo's environment for externally executed coverage tests."""
    _run(
        ["cargo", "llvm-cov", "clean", "--workspace", "--manifest-path", manifest],
        cwd=root,
        env=env,
    )
    result = _run(
        [
            "cargo",
            "llvm-cov",
            "show-env",
            "--manifest-path",
            manifest,
            "--no-cfg-coverage",
            "--sh",
        ],
        cwd=root,
        env=env,
        capture_output=True,
    )
    configured = env.copy()
    configured.update(_parse_shell_environment(result.stdout))
    return configured


def _test_executable(
    root: Path,
    manifest: Path,
    env: dict[str, str],
    workers: int,
) -> Path:
    """Build tests and return the native unit-test executable path."""
    result = _run(
        [
            "cargo",
            "test",
            "--manifest-path",
            manifest,
            "--no-default-features",
            "--no-run",
            "--message-format=json",
            "--jobs",
            str(workers),
        ],
        cwd=root,
        env=env,
        capture_output=True,
    )
    for line in result.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = message.get("target", {})
        profile = message.get("profile", {})
        executable = message.get("executable")
        if (
            message.get("reason") == "compiler-artifact"
            and target.get("name") == "backtide_core"
            and "lib" in target.get("kind", [])
            and profile.get("test")
            and executable
        ):
            return Path(executable).resolve()
    raise RuntimeError("Cargo did not report the backtide_core test executable.")


def _llvm_tools(root: Path, env: dict[str, str]) -> tuple[Path, Path]:
    """Locate llvm-profdata and llvm-cov from the active Rust toolchain."""
    version = _run(["rustc", "-vV"], cwd=root, env=env, capture_output=True).stdout
    host_match = re.search(r"^host: (.+)$", version, flags=re.MULTILINE)
    if host_match is None:
        raise RuntimeError("Could not determine the active Rust host triple.")
    sysroot = Path(
        _run(
            ["rustc", "--print", "sysroot"], cwd=root, env=env, capture_output=True
        ).stdout.strip()
    )
    suffix = ".exe" if os.name == "nt" else ""
    directory = sysroot / "lib" / "rustlib" / host_match.group(1) / "bin"
    profdata = directory / f"llvm-profdata{suffix}"
    llvm_cov = directory / f"llvm-cov{suffix}"
    if not profdata.is_file() or not llvm_cov.is_file():
        raise RuntimeError("LLVM tools are missing; install the llvm-tools-preview component.")
    return profdata, llvm_cov


def _installed_extension(root: Path, env: dict[str, str]) -> Path:
    """Return the native extension loaded by the active interpreter."""
    result = _run(
        [
            sys.executable,
            "-c",
            "import backtide.core; print(backtide.core.__file__)",
        ],
        cwd=root,
        env=env,
        capture_output=True,
    )
    extension = Path(result.stdout.strip()).resolve()
    if not extension.is_file():
        raise RuntimeError(f"Installed extension was not found at {extension}.")
    return extension


def _write_report(
    root: Path,
    env: dict[str, str],
    test_executable: Path,
    extension: Path,
    profiles: list[Path],
    output: Path,
    temporary: Path,
) -> float:
    """Merge raw profiles, write LCOV, and return Rust line coverage."""
    profdata, llvm_cov = _llvm_tools(root, env)
    merged = temporary / "backtide.profdata"
    _run(
        [profdata, "merge", "-sparse", *profiles, "-o", merged],
        cwd=root,
        env=env,
    )
    objects = [test_executable, "--object", extension, "--instr-profile", merged]
    summary = _run(
        [llvm_cov, "export", "--summary-only", *objects],
        cwd=root,
        env=env,
        capture_output=True,
    )
    percent = float(json.loads(summary.stdout)["data"][0]["totals"]["lines"]["percent"])

    output.parent.mkdir(parents=True, exist_ok=True)
    args = [str(llvm_cov), "export", "--format=lcov", *(str(item) for item in objects)]
    LOGGER.info("Writing LCOV report: %s", output)
    with output.open("w", encoding="utf-8", newline="\n") as stream:
        subprocess.run(args, check=True, cwd=root, env=env, stdout=stream, text=True)
    return percent


def _parse_args() -> argparse.Namespace:
    """Parse command-line arguments."""
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=root / "src" / "backtide_core" / "Cargo.toml",
    )
    parser.add_argument("--output", type=Path, default=root / "rust-coverage.info")
    parser.add_argument("--fail-under", type=float, default=70.0)
    parser.add_argument("--workers", type=int, default=12)
    return parser.parse_args()


def main() -> int:
    """Build, test, merge profiles, and enforce the line-coverage floor."""
    args = _parse_args()
    if not 1 <= args.workers <= 12:
        raise ValueError("--workers must be between 1 and 12.")

    root = Path(__file__).resolve().parents[1]
    manifest = args.manifest.resolve()
    output = args.output.resolve()
    base_env = os.environ.copy()
    base_env["CARGO_BUILD_JOBS"] = str(args.workers)
    base_env["PYO3_PYTHON"] = sys.executable
    env = _coverage_environment(root, manifest, base_env)

    with tempfile.TemporaryDirectory(prefix="backtide-coverage-") as temp_name:
        temporary = Path(temp_name)
        profile_directory = temporary / "profiles"
        profile_directory.mkdir()
        env["LLVM_PROFILE_FILE"] = str(profile_directory / "backtide-%p-%m.profraw")
        wheel_dir = temporary / "wheel"
        _run(
            [
                sys.executable,
                "-m",
                "maturin",
                "build",
                "--profile",
                "dev",
                "--interpreter",
                sys.executable,
                "--out",
                wheel_dir,
            ],
            cwd=root,
            env=env,
        )
        wheels = list(wheel_dir.glob("*.whl"))
        if len(wheels) != 1:
            raise RuntimeError(f"Expected one instrumented wheel, found {len(wheels)}.")
        _run(
            ["uv", "pip", "install", "--python", sys.executable, "--reinstall", wheels[0]],
            cwd=root,
            env=env,
        )

        test_executable = _test_executable(root, manifest, env, args.workers)
        for profile in profile_directory.glob("*.profraw"):
            profile.unlink()
        _run(
            [
                "cargo",
                "test",
                "--manifest-path",
                manifest,
                "--no-default-features",
                "--jobs",
                str(args.workers),
            ],
            cwd=root,
            env=env,
        )
        _run(
            [sys.executable, "-m", "pytest", "-n", str(args.workers), root / "tests"],
            cwd=root,
            env=env,
        )

        profiles = sorted(profile_directory.glob("*.profraw"))
        if not profiles:
            raise RuntimeError(f"No coverage profiles were written under {profile_directory}.")
        percent = _write_report(
            root,
            env,
            test_executable,
            _installed_extension(root, env),
            profiles,
            output,
            temporary,
        )

    LOGGER.info("Rust line coverage: %.2f%%", percent)
    if percent + sys.float_info.epsilon < args.fail_under:
        LOGGER.error("Coverage %.2f%% is below the %.2f%% threshold.", percent, args.fail_under)
        return 1
    return 0


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    raise SystemExit(main())
