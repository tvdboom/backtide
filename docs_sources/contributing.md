# Contributing
--------------

Are you interested in contributing to Backtide? Do you want to report a bug?
Do you have a question? Before you do, please read the following guidelines.

<br>


## Submission context

### Question or problem?

For quick questions, there's no need to open an issue. Check first if the
question isn't already answered in the [FAQ][frequently-asked-questions]
section. If not, reach us through the [discussions](https://github.com/tvdboom/backtide/discussions) page.


### Report a bug?

If you found a bug in the source code, you can help by submitting an issue
to the [issue tracker](https://github.com/tvdboom/backtide/issues) in the GitHub repository. Even better, you can
submit a Pull Request with a fix. However, before doing so, please read the
[submission guidelines].


### Missing a feature?

You can request a new feature by submitting an [issue](https://github.com/tvdboom/backtide/issues) to the GitHub
Repository. If you would like to implement a new feature, please submit an
issue with a proposal for your work first. Please consider what kind of
change it is:

* For a **major feature**, first open an issue and outline your proposal so
  that it can be discussed. This will also allow us to better coordinate our
  efforts, prevent duplication of work, and help you to craft the change so
  that it is successfully accepted into the project.

* **Small features and bugs** can be crafted and directly submitted as a Pull
  Request. However, there is no guarantee that your feature will make it into
  `master`, as it's always a matter of opinion whether if benefits the
  overall functionality of the project.

<br><br>


## Project layout

The latest stable release of Backtide is on the `master` branch, whereas the
latest version in development is on the `development` branch. Make sure to
familiarize yourself with the project layout before making any major contributions.

### Folder structure

```text
backtide/                       # Repository root
├── pyproject.toml              # Python package metadata, dependencies & tool config
├── tox.ini                     # Test / CI task runner configuration
├── uv.lock                     # Locked dependency versions (managed by uv)
├── mkdocs.yml                  # Documentation site configuration
├── backtide.config.toml        # Default runtime configuration file
├── .pre-commit-config.yaml     # Pre-commit hook definitions
│
├── backtide/                   # Python package (public API)
│   ├── __init__.py             # Top-level re-exports
│   ├── backtest.py             # Backtest model re-exports
│   ├── cli.py                  # Click CLI entry point (backtide launch / download)
│   ├── config.py               # Configuration re-exports
│   ├── constants.py            # Python-side constants
│   ├── data.py                 # Data re-exports
│   ├── storage.py              # Storage re-exports
│   ├── core.*.pyd              # Compiled Rust extension (built by maturin)
│   ├── ui/                     # Streamlit interactive UI
│   │   ├── app.py              # Main Streamlit application
│   │   ├── download.py         # Download page
│   │   ├── experiment.py       # Experiment page
│   │   ├── results.py          # Results page
│   │   ├── storage.py          # Storage page
│   │   └── utils.py            # UI helpers
│   └── utils/                  # Python utility modules
│       ├── constants.py
│       ├── enum.py
│       └── utils.py
│
├── backtide_core/              # Rust crate (compiled into backtide.core via PyO3)
│   ├── Cargo.toml              # Crate metadata & dependencies
│   ├── Cargo.lock              # Locked Rust dependency versions
│   ├── rustfmt.toml            # Rust formatter configuration
│   ├── src/
│   │   ├── lib.rs              # Crate root & PyO3 module registration
│   │   ├── engine.rs           # Core backtest engine
│   │   ├── constants.rs        # Shared constants
│   │   ├── errors.rs           # Error types
│   │   ├── backtest/           # Backtest models
│   │   ├── config/             # Configuration models & parsing
│   │   ├── data/               # Data layer: models, providers (Yahoo, Binance, Kraken, Coinbase)
│   │   ├── storage/            # Storage layer: DuckDB backend & Storage trait
│   │   └── utils/              # Utility functions & HTTP helpers
│   └── benches/                # Criterion.rs benchmarks
│       ├── storage_bench.rs    # DuckDB storage throughput / latency benchmarks
│       └── data_bench.rs       # Live API download latency benchmarks
│
├── tests/                      # Python unit tests (pytest)
│   ├── __init__.py
│   └── test_config.py
│
├── docs_sources/               # MkDocs documentation sources
│   ├── index.md
│   ├── about.md
│   ├── getting_started.md
│   ├── contributing.md
│   ├── dependencies.md
│   ├── faq.md
│   ├── license.md
│   ├── user_guide/             # User-guide pages
│   ├── api/                    # Auto-generated API reference pages
│   ├── img/                    # Images, icons, logos
│   ├── overrides/              # MkDocs Material theme overrides
│   ├── scripts/                # Build-time hooks (autodocs, autorun)
│   └── stylesheets/            # Custom CSS / JS
│
└── images/                     # Branding assets & provider logos
```

### Key technologies

| Layer          | Technology                                                                   |
|----------------|------------------------------------------------------------------------------|
| Core engine    | **Rust** compiled to a Python extension via [PyO3](https://pyo3.rs) & [maturin](https://github.com/PyO3/maturin) |
| Storage        | **DuckDB** (embedded OLAP database)                                          |
| Python API     | Re-export wrappers around the compiled `backtide.core` module                |
| CLI            | [Click](https://click.palletsprojects.com)                                   |
| UI             | [Streamlit](https://streamlit.io)                                            |
| Docs           | [MkDocs Material](https://squidfunk.github.io/mkdocs-material/)             |
| Testing        | [pytest](https://docs.pytest.org) (Python) · `cargo test` (Rust)            |
| Linting        | [Ruff](https://docs.astral.sh/ruff/) (Python) · `cargo clippy` / `cargo fmt` (Rust) |
| Benchmarking   | [Criterion.rs](https://github.com/bheisler/criterion.rs)                    |
| Task runner    | [tox](https://tox.wiki) with the [tox-uv](https://github.com/tox-dev/tox-uv) plugin |
| Package mgmt   | [uv](https://docs.astral.sh/uv/)                                            |


<br><br>

## Development setup

### 1. Clone the repository

```bash
git clone https://github.com/tvdboom/backtide.git
cd backtide
```

### 2. Create a virtual environment and install

```bash
uv venv
uv sync --all-groups
```

### 3. Build the Rust extension (development mode)

```bash
maturin develop --manifest-path backtide_core/Cargo.toml
```

This compiles the `backtide_core` crate and installs the resulting
`.pyd` / `.so` extension into the active environment so that
`import backtide.core` works without a full wheel build.

### 4. Install pre-commit hooks

```bash
pre-commit install
```

<br><br>


## Running tests

### Python tests

Python tests live in the `tests/` directory and are executed with **pytest**:

```bash
pytest tests/
```

For parallel execution (requires `pytest-xdist`):

```bash
pytest -n auto tests/
```

### Rust tests

Rust unit tests embedded in the `backtide_core` crate:

```bash
cargo test --manifest-path backtide_core/Cargo.toml
```

<br><br>


## Tox

[Tox](https://tox.wiki) is used as the unified task runner for the project. It is configured
in `tox.ini` and uses the [tox-uv](https://github.com/tox-dev/tox-uv) plugin so environments are created with
`uv` instead of plain `venv`.

### Available environments

| Environment       | What it does                                                             |
|-------------------|--------------------------------------------------------------------------|
| `py311` … `py314` | Build the wheel (including Rust compilation) and run pytest on that Python version. |
| `py314-min`       | Test against the **oldest compatible** versions of runtime dependencies. |
| `cargo-test`      | Run `cargo test` on the Rust crate.                                      |
| `pre-commit`      | Run all pre-commit hooks (`ruff`, `ruff-format`, `cargo fmt`, `cargo clippy`, …). |
| `bench`           | Run Criterion.rs benchmarks (see [Benchmarks](#benchmarks)).             |
| `docs`            | Build the MkDocs documentation in strict mode.                           |

<br><br>


## Benchmarks

Performance of the Rust core is tracked with
[Criterion.rs](https://github.com/bheisler/criterion.rs) benchmarks defined in
`backtide_core/benches/`. Criterion generates HTML reports in `backtide_core/target/criterion/report/index.html`.
Two benchmark suites exist:

### Storage benchmarks

Measures DuckDB bulk-insert throughput and query latency using synthetic bar
data. Each iteration creates an isolated temporary database via `tempfile` so
runs never interfere with one another.

### Data benchmarks

Measures end-to-end download latency for the data providers. These benchmarks hit
real network endpoints, so results are inherently noisier and depend on network
conditions.

### Running benchmarks

```bash
# All benchmarks
cargo bench --manifest-path backtide_core/Cargo.toml

# Storage only
cargo bench --manifest-path backtide_core/Cargo.toml --bench storage_bench

# Data/download only
cargo bench --manifest-path backtide_core/Cargo.toml --bench data_bench
```

Or via tox:

```bash
tox -e bench
```

<br><br>


## Pre-commit & linting

The project uses [pre-commit](https://pre-commit.com/) to enforce code quality
on every commit. The hooks are defined in `.pre-commit-config.yaml`. To run all
hooks manually:

```bash
pre-commit run --all-files
```

Or through tox:

```bash
tox -e pre-commit
```

### Python style

* [Ruff](https://docs.astral.sh/ruff/) is the single linter and formatter.
* Maximum line length is **99 characters**. Keep docstrings below 80 characters
  where practical.
* Docstrings follow the **NumPy** convention.

### Rust style

* `cargo fmt` with settings in `backtide_core/rustfmt.toml`.
* `cargo clippy` with `-D warnings` (all warnings are errors).

<br><br>


## Building the documentation

The docs are built with [MkDocs Material](https://squidfunk.github.io/mkdocs-material/)
and live in `docs_sources/`. Build-time hooks in `docs_sources/scripts/` handle
auto-generated API reference pages.

```bash
# Live preview with hot-reload
mkdocs serve

# Production build (strict mode)
mkdocs build --strict
```

Or via tox:

```bash
tox -e docs
```

<br><br>


## Submission guidelines

### Submitting an issue

Before you submit an issue, please search the [issue tracker](https://github.com/tvdboom/backtide/issues),
maybe an issue for your problem already exists, and the discussion
might inform you of workarounds readily available.

We want to fix all the issues as soon as possible, but before fixing a
bug, we need to reproduce and confirm it. In order to reproduce bugs, we
will systematically ask you to provide a minimal reproduction scenario
using the custom issue template.


### Submitting a pull request

Before you submit a pull request, please work through this checklist to
make sure that you have done the necessary so we can efficiently review
and accept your changes.

* Update the documentation so all of your changes are reflected there.
* Adhere to the coding style enforced by Ruff (Python) and rustfmt / Clippy
  (Rust). Run `pre-commit run --all-files` to verify.
* Use a maximum of 99 characters per line. Try to keep docstrings below
  80 characters.
* Update the project unit tests to test your code changes as thoroughly
  as possible — both `tests/` (Python) and `cargo test` (Rust).
* Make sure that your code is properly commented with docstrings and
  comments explaining your rationale behind non-obvious coding practices.
* Run the full tox suite: `tox` and make sure all environments pass.

If your contribution requires a new **Python** library dependency:

* Double-check that the new dependency is easy to install via pip.
* The library should support Python 3.11, 3.12, 3.13 and 3.14.
* Make sure the code works with the latest version of the library.
* Update the dependencies in the documentation.
* Add the library with the minimum required version to `pyproject.toml`.

If your contribution requires a new **Rust** crate dependency:

* Add it to `backtide_core/Cargo.toml` with an explicit version.
* Make sure `cargo clippy` and `cargo test` still pass.

After submitting your pull request, GitHub will automatically run the tests
on your changes and make sure that the updated code builds successfully.
The checks run on all supported Python versions, on Ubuntu and Windows. We also
use services that automatically check code style and test coverage.
