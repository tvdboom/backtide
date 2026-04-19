# Development task runner for backtide.
# Install: uv tool install rust-just
# Usage:   just <recipe>

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# List available recipes
[private]
default:
    @just --list

# Build the Rust extension and regenerate stubs
build:
    maturin develop -m backtide_core/Cargo.toml
    @just stubs

# Generate stub files from the compiled module
stubs:
    python scripts/generate_stubs.py

# Verify stubs are in sync with the compiled module
check:
    python scripts/generate_stubs.py --check

# Run the test suite (Python + Cargo) with coverage ≥50%
test *args:
    uv run pytest -n=auto --cov=backtide --cov-fail-under=40 {{args}}
    cargo llvm-cov --manifest-path backtide_core/Cargo.toml --fail-under-lines 40

# Run pre-commit hooks on all files
lint:
    uv run pre-commit run --all-files

# Run the full CI pipeline locally via tox
tox:
    uv run tox

# Build and serve the docs locally
docs:
    uv run mkdocs serve
