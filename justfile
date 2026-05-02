# Development task runner for backtide.
# Install: uv tool install rust-just
# Usage:   just <recipe>

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# List available recipes
[private]
default:
    @just --list

sync:
    uv sync --upgrade --all-extras --all-groups

# Build the Rust extension and regenerate stubs
build:
    uv pip install -e .
    @just stubs

# Generate stub files from the compiled module
stubs:
    python scripts/generate_stubs.py

# Verify stubs are in sync with the compiled module
check:
    python scripts/generate_stubs.py --check

# Run the test suite (Python + Cargo)
test *args:
    uv run pytest -n=auto {{args}}
    uv run python scripts/run_cargo.py \
        cargo llvm-cov \
            --manifest-path src/backtide_core/Cargo.toml \
            --no-cfg-coverage

# Run Rust benchmarks
bench *args:
    uv run python scripts/run_cargo.py \
        cargo bench \
            --manifest-path src/backtide_core/Cargo.toml \
            --no-default-features \
            {{args}}

# Run pre-commit hooks on all files
lint:
    uv run pre-commit run --all-files

# Run the full CI pipeline locally via tox
tox:
    uv run tox

ty:
    uv run ty check

# Build and serve the docs locally
docs:
    $env:PYTHONPATH="."; uv run mkdocs serve

launch:
    backtide launch
