# Development task runner for backtide.
# Install: uv tool install rust-just
# Usage: just <recipe>

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

version := shell("uv run --no-sync python -c \"import tomllib, pathlib; print(tomllib.loads(pathlib.Path('pyproject.toml').read_text(encoding='utf-8'))['project']['version'])\"")

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
    uv run python scripts/generate_stubs.py

# Verify stubs are in sync with the compiled module
check:
    uv run python scripts/generate_stubs.py --check

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

# Build and serve the documentation without replacing a running development install
docs:
    uv run --isolated --no-editable --no-default-groups --group docs python scripts/generate_stubs.py
    $env:PYTHONPATH="."; uv run --isolated --no-editable --no-default-groups --group docs mkdocs serve

launch:
    # Launch the checkout while keeping Rust rebuilds explicit.
    # Run `just build` explicitly after changing Rust code.
    $env:PYTHONPATH=(Resolve-Path "src").Path; uv run --no-sync backtide launch

# Install frontend development dependencies (not required by package users)
frontend-sync:
    pnpm --dir frontend install --frozen-lockfile

# Run frontend unit tests
frontend-test:
    pnpm --dir frontend test

# Rebuild the production SPA bundled into Python wheels
frontend-build:
    pnpm --dir frontend build

publish:
    git --no-pager status
    git --no-pager pull
    git tag -a v{{version}} -m "v{{version}}"
    git push origin v{{version}}
