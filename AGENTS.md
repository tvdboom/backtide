# Backtide contributor and agent guide

This file applies to the entire repository. Treat it as the default contract for every change.
More-specific `AGENTS.md` files may add rules for their subtree, but may not weaken these rules.

## Project intent

Backtide is a local-first trading research application. It combines a Python API and web server,
a Rust/PyO3 core, an embedded DuckDB database, and a Vue single-page application. The main use
cases are downloading market data, configuring and running backtests, running paper-trading
sessions from live WebSocket data, persisting results, and analysing those results with Plotly.

Keep the package approachable for Python users. A released wheel must contain the compiled Rust
extension and built frontend assets; end users install Backtide with `pip` and do not need Rust,
Node.js, or pnpm. Node/pnpm are development-only tools for rebuilding the SPA.

## Repository map

```text
.
|-- pyproject.toml              Python package, maturin, Ruff, coverage, dependency config
|-- uv.lock                     Locked Python development/runtime environment
|-- justfile                    Canonical local task shortcuts
|-- tox.ini                     CI environments and supported Python versions
|-- mkdocs.yml                  Documentation navigation and MkDocs configuration
|-- backtide.config.toml        Example/default runtime configuration
|-- application/                Vue 3 + Vite application and frontend tests
|-- src/
|   |-- backtide/               Installed Python package and public Python API
|   |   |-- analysis/           Plotly plots and Python-facing analysis helpers
|   |   |-- strategies/         Custom-strategy base class and public re-exports
|   |   |-- indicators/         Custom-indicator base class and public re-exports
|   |   |-- sizers/             Custom-sizer base class and public re-exports
|   |   |-- ui/                 Python HTTP/API service and generated production SPA assets
|   |   |-- core/               Generated `.pyi` files for the PyO3 extension
|   |   |-- backtest.py         High-level experiment wrapper and model re-exports
|   |   |-- live.py             High-level paper-trading facade and model re-exports
|   |   |-- data.py             Market-data re-exports
|   |   |-- storage.py          Storage re-exports
|   |   |-- config.py           Configuration re-exports
|   |   `-- cli.py              `backtide` Click commands
|   `-- backtide_core/          Rust crate compiled as `backtide.core`
|       |-- database/           Canonical DuckDB table schemas, one SQL file per table
|       |-- src/
|       |   |-- backtest/       Historical simulation engine, orders, FX, margin, models
|       |   |-- live/           Paper engine, live models, WebSocket providers, PyO3 API
|       |   |-- data/           Historical providers, data models, resolution/download logic
|       |   |-- storage/        `Storage` trait and mutex-protected DuckDB implementation
|       |   |-- config/         Runtime and experiment configuration
|       |   |-- strategies/     Built-in Rust strategies and extension interface
|       |   |-- indicators/     Built-in Rust indicators and extension interface
|       |   |-- utils/          HTTP, progress, logging, Python conversion helpers
|       |   |-- engine.rs       Process-wide engine, providers, runtime, caches, database
|       |   `-- lib.rs          Crate modules and PyO3 registration
|       `-- benches/            Criterion storage, data, backtest, and live benchmarks
|-- tests/                      Pytest suite; shared fixtures are in `tests/conftest.py`
|-- docs_sources/               MkDocs pages, API directives, hooks, styles, documentation media
|-- images/                     README branding, screenshots, and provider logos
|-- scripts/                    Cargo wrapper and generated-stub tooling
`-- .github/workflows/          Publish and cross-platform test pipelines
```

Do not commit `target/`, Python caches, local DuckDB/WAL files, frontend dependency directories,
or other generated local state. `src/backtide/core/*.pyi` and the production SPA bundle are the
exceptions: they are generated, but shipped in wheels and therefore must be kept in sync.

## Architecture and ownership boundaries

- Python is the ergonomic public surface. Modules such as `backtide.data`, `backtide.storage`,
  and `backtide.config` intentionally stay thin and re-export `backtide.core` objects.
- Rust owns CPU-heavy backtest/live execution, built-in strategies and indicators, provider
  clients, data validation, caching, and DuckDB persistence. Do not duplicate this logic in the
  web service or Python wrappers.
- `Engine` is a process-wide singleton. It owns the historical-data Tokio runtime, shared provider
  `Arc`s, TTL caches, and the storage trait object. Live WebSockets use their own process-wide
  runtime so exchange feeds never trigger storage or Yahoo initialization. New code must not create
  a runtime, HTTP client, or database connection per request/tick when a process-wide resource can
  be reused.
- Each Rust feature directory conventionally separates `models.rs` (data types), `traits.rs`
  (extension contract, when applicable), `engine.rs` (implementation on `Engine`),
  `interface.rs` (Python-visible functions/classes), and `mod.rs` (exports and registration).
- Python-visible Rust additions must be registered from the feature's `mod.rs` and ultimately
  from `lib.rs`, re-exported through the appropriate Python facade, and reflected in generated
  stubs. Run the stub check after changing signatures or Rust docstrings.
- Custom Python `BaseStrategy`, `BaseIndicator`, and `BaseSizer` subclasses are supported extension
  points. Preserve their call signatures, accepted pandas/polars inputs, and snapshot semantics.
- The Vue app calls the local Python JSON API. Components must not access DuckDB files, import the
  extension directly, execute Python, or reimplement portfolio/backtest/live rules.
- The Python UI service owns request validation, serialization, cancellation/session coordination,
  static-file serving, and calls into the public Python/backend APIs. Keep route handlers thin;
  put reusable work in the service layer.
- `application/` is the source of truth for UI code. The production assets under
  `src/backtide/ui/static/` are build output; rebuild them instead of editing minified files.
- Paper trading is simulation only. Never add broker order submission, credentials, or claims of
  guaranteed execution. Live providers normalize incoming messages before the paper engine sees
  them, and disconnect/cancel paths must release tasks and sockets deterministically.

## Database schema discipline

- Keep the canonical DuckDB table definitions in `src/backtide_core/database/`, with exactly one
  `.sql` file per table. Embed and execute every schema file from `DuckDb::init`; do not duplicate
  table definitions in Rust strings.
- Every table schema must use `CREATE TABLE IF NOT EXISTS`. Initialization creates missing tables
  and assumes every existing table already has the one current, correct schema. It must not drop,
  replace, truncate, alter, migrate, or validate existing tables.
- Never add database migrations, migration frameworks, migration or schema-version tables,
  startup compatibility transforms, or support for historical schemas. Schema changes describe
  only the complete layout of a newly created database.
- Tests and development fixtures must use a newly created database when they need a changed
  schema. Normal application startup must preserve all data in an existing database.

## Python style

Follow the existing code and `pyproject.toml` exactly.

- Target Python 3.11 through 3.14. Use modern annotations (`X | None`, built-in generics), four
  spaces, double-quoted strings as formatted by Ruff, and a 99-character line limit.
- Start normal source/test modules with the existing module docstring form:

  ```python
  """Backtide.

  Author: Mavs
  Description: Short module purpose.

  """
  ```

- Put `from __future__ import annotations` immediately after the module docstring when forward
  references or import deferral are useful. Imports are Ruff/isort ordered: future, standard
  library, third party, first party, local. Do not manually preserve a different order.
- Fully annotate new public functions and non-obvious helpers. Put imports used only for typing
  inside `if TYPE_CHECKING:` when that avoids runtime imports or optional dependencies.
- Public docstrings use the configured NumPy convention: imperative one-line summary, optional
  explanation, then only the applicable `Parameters`, `Attributes`, `Returns`, `Raises`,
  `See Also`, and `Examples` sections. Keep the blank line between parameter entries used by the
  current API. State defaults in the type line (for example, `bool, default=True`).
- Documentation is Markdown-aware. Use `[Type]` or `[label][anchor]` for internal documentation
  references, backticks for values/identifiers, and fenced `python`/`pycon` examples. Examples must
  be short, runnable, and use the public import path.
- Private helpers still need a concise docstring when their purpose, mutation, units, or shape is
  not obvious. Comments explain why or a domain invariant, not a line-by-line translation.
- Prefer explicit validation and specific exceptions. Do not use bare `except`, silently discard
  failures, use mutable default arguments, or treat falsy values as missing. Preserve `False`,
  `0`, empty collections, and `None` according to the documented contract.
- Avoid hidden global mutable state. Existing configuration/UI globals are compatibility points;
  do not add more. Make cleanup idempotent and use context managers/finally blocks for resources.
- DataFrame functions must document required columns, accept the declared dataframe family, avoid
  mutating caller-owned frames, preserve time-zone/currency semantics, and return a stable schema
  for empty input.
- Keep public facade modules thin. If logic is reusable or performance-sensitive, put it in the
  relevant Rust engine or a focused Python service/helper rather than a re-export module.

Ruff is authoritative. Do not add `# noqa`, per-file ignores, or lint exclusions merely to land a
change; a suppression must be narrow and explain an unavoidable interface constraint.

## Rust style

Follow `rustfmt.toml`, Cargo lints, and the conventions of the surrounding feature module.

- Use standard Rust naming: `snake_case` modules/functions/locals, `UpperCamelCase` types/traits,
  and `SCREAMING_SNAKE_CASE` constants. Keep imports rustfmt-ordered and format match blocks with
  trailing commas.
- In named structs whose fields each have `///` documentation, put one blank line between fields:
  after a field and before the next field's documentation. Keep undocumented/internal struct field
  lists compact; do not add blank lines between those fields.
- Start substantive modules with `//!` module documentation. Add `///` rustdoc to public types,
  fields, traits, and functions; document units (`seconds`, percentages versus fractions), signs,
  ownership, ordering, empty behavior, and error conditions.
- Python-visible `#[pyclass]`, `#[pymethods]`, and `#[pyfunction]` documentation is also the Python
  docstring and stub source. Use the same NumPy-style sections and cross-reference syntax as the
  Python API. Keep `#[pyo3(signature = (...))]` defaults and inspect annotations synchronized with
  the implementation and docs.
- Use `thiserror` feature-specific enums and `Result` aliases. Add context at the layer that knows
  it and convert errors to `PyErr` only at the Python boundary. Production paths must not
  `unwrap`, `expect`, or `panic` on provider data, persisted data, lock poisoning, user input, or
  runtime state; return a typed error. Assertions/unwraps are acceptable in tests for setup.
- Prefer borrowing (`&str`, slices, references) to allocation. Clone only for a required ownership
  boundary, persisted snapshot, cache value, or task hand-off. In hot bar/tick loops, review every
  `String`, `Vec`, `HashMap`, dataframe conversion, and `.clone()` and preallocate known sizes.
- Share provider/HTTP clients with `Arc`; do not put an `Arc` around short-lived owned data merely
  to avoid designing lifetimes. Do not hold a mutex guard over network I/O, Python calls, logging,
  or expensive computation. Keep DuckDB critical sections bounded and preserve transaction
  atomicity on failure.
- Async work must be bounded, cancellable, and timeout-aware. Avoid unbounded task spawning and
  unbounded channels. WebSocket reconnect loops need backoff, cancellation, terminal-state
  reporting, and no busy spin.
- Keep market-data normalization at the provider boundary. The engine consumes canonical symbols,
  intervals, timestamps, bars/ticks, and currencies independent of provider payload shape.
- Preserve determinism in pure strategy/indicator/backtest logic. Do not introduce wall-clock or
  network access into kernels that can accept explicit timestamps/data.
- Use `tracing` fields for operational context and avoid logging secrets, full payloads, or a line
  per bar/tick at normal levels.
- `unsafe` requires a documented invariant and a test that exercises the boundary. Prefer safe
  APIs. Do not broaden the crate-level lint allowances.

## Vue/frontend style

- Use Vue 3 Composition API single-file components with `<script setup>` and Vite. Keep components
  focused; put reusable API/state behavior in composables and shared formatting in utilities.
- Component files are kebab-case. JavaScript and CSS use two-space indentation, single quotes, and
  no semicolons, matching the reference application. Follow the repository formatter/linter when
  it is stricter.
- Use semantic HTML and labelled, keyboard-accessible controls. Preserve visible focus, sufficient
  contrast, reduced-motion behavior, and responsive layouts. Never encode status by color alone.
- Centralize the Backtide palette, spacing, typography, and chart defaults in theme/style helpers.
  Do not scatter one-off colors or duplicate Plotly layout objects across views.
- Network state must be explicit: loading, empty, error, retry/reconnect, and canceled/finished
  states all need stable UI. Debounce search and stale-request cancellation where appropriate.
- Treat all API/provider text as untrusted. Render text normally, avoid `v-html`, and do not expose
  local filesystem paths or stack traces in browser responses.
- Unit-test composables, data transformations, state transitions, and interaction behavior. Mock
  HTTP/WebSocket boundaries; do not make unit tests depend on a running backend or public network.

## Testing and benchmarks

Every behavior change needs a regression test at the lowest useful layer. Tests must be
deterministic, isolated, and able to run in parallel.

- Structure tests as Arrange, Act, Assert. Keep setup focused, exercise one behavior, and assert
  externally observable state or a specific error rather than duplicating the implementation.
- Python tests live in `tests/test_<area>.py`. Use `TestThing` classes to group a public unit and
  `test_<behavior>` names with a one-sentence docstring. Reuse `tests/conftest.py` fixtures, pytest
  parametrization, `monkeypatch`, and `pytest.raises(..., match=...)` rather than custom harnesses.
- Python storage tests use temporary/copied databases and must never touch the user's configured
  database. Provider tests mock responses and should not require the internet.
- Rust unit tests live in `#[cfg(test)] mod tests` beside the implementation. Test pure helpers and
  edge/error cases directly; use `rstest` for meaningful input matrices and `wiremock`/stub traits
  for network boundaries. Use a temporary DuckDB and a stub `DataProvider` for engine tests.
- Async tests use deterministic mocked HTTP/WebSocket input, explicit cancellation, and bounded
  timeouts. Do not use real provider endpoints or timing sleeps to make a race pass.
- Cross-language tests belong in pytest when they verify the installed public Python contract.
  Keep Rust tests for kernels, parsing, state machines, provider normalization, storage, and
  concurrency/cancellation invariants.
- Criterion files live in `src/backtide_core/benches/`, declare `harness = false` in Cargo, use
  deterministic synthetic fixtures, move setup outside `b.iter`, and benchmark a stable named
  operation. Use batched setup when the operation mutates its input. A benchmark is not a test:
  assert correctness separately before interpreting timing.
- Live tests use deterministic tick streams and explicit time; cover order/fill/accounting state,
  disconnect/reconnect, malformed messages, cancellation, and bounded buffering. Never benchmark
  a public WebSocket endpoint.
- Frontend tests use the repository's Jest-compatible test command and mock the API. Test user
  behavior and rendered states rather than component implementation details.

Do not weaken coverage thresholds, skip a failing test, add timing sleeps, or update snapshots
without inspecting the semantic change.

## Documentation and MkDocs

- User guides live in `docs_sources/user_guide/`; CLI pages in `docs_sources/cli/`; API directive
  pages in `docs_sources/api/`. Add a page to `mkdocs.yml` navigation when it should be reachable.
- API reference pages use the existing `:: module:object` directives. Their content comes from
  Python/Rust docstrings, so fix source documentation instead of duplicating it in generated text.
- Use repository-relative Markdown links and existing source anchors for internal pages. Do not
  embed local filesystem paths; verify moved/renamed pages and media with the strict MkDocs build.
- MkDocs hooks execute examples and render Plotly output. Keep examples safe, reproducible, and
  reasonably fast. `uv run mkdocs build --strict` must have no warnings.
- After a Python-visible Rust signature, class, enum, or docstring changes, build the extension and
  run `uv run python scripts/generate_stubs.py`; commit the resulting `.pyi` changes. Verify with
  `uv run python scripts/generate_stubs.py --check`.
- After frontend source changes, run the frontend build and commit the refreshed production bundle
  under `src/backtide/ui/static/`. Do not patch hashes/minified assets manually.
- Screenshots in `images/scenery/` and documentation media must show current UI, contain no local
  secrets or personal paths, and use stable seeded/demo data where possible.

## Required validation

Use focused commands while iterating, but focused checks never replace the complete validation
required before handoff. Every change must finish with all pre-commit hooks, all Python tests, all
Rust tests, and all frontend tests passing:

```text
uv run pre-commit run --all-files --show-diff-on-failure
just test
pnpm --dir application test
```

Do not hand off with any failing hook or test. Fix every failure, including failures in code outside
the immediate patch when the change exposes them. The only exception is a check that cannot run
because a required tool, service, platform library, or network resource is unavailable; report the
exact command and failure in that case.

Additional focused and release-oriented validation commands include:

```text
uv run pytest -n=auto tests/<focused-file>.py
cargo test --manifest-path src/backtide_core/Cargo.toml --no-default-features <focused-test>
cargo fmt --manifest-path src/backtide_core/Cargo.toml -- --check
cargo clippy --manifest-path src/backtide_core/Cargo.toml --all-targets --all-features -- -D warnings
uv run ruff check src/backtide tests scripts --exclude src/backtide/core
uv run ruff format --check src/backtide tests scripts --exclude src/backtide/core
uv run ty check
uv run python scripts/generate_stubs.py --check
uv run mkdocs build --strict
```

For the SPA, run its package scripts from `application/` (install dependencies only in a development
checkout):

```text
pnpm install
pnpm test
pnpm build
```

Useful aggregate commands are `just test`, `just lint`, `just bench`, and `just tox`. Focused live
validation is `pytest tests/test_live.py`, `cargo test --manifest-path src/backtide_core/Cargo.toml
live::`, and `cargo bench --manifest-path src/backtide_core/Cargo.toml --bench live_bench --no-run`.

Never claim a check passed when it was skipped.

## Change discipline

- Preserve public compatibility unless the task explicitly authorizes a break. Update Python,
  Rust, stubs, web API, tests, and docs together when a contract genuinely changes.
- Keep patches scoped. Do not opportunistically rewrite unrelated modules, reformat generated
  output by hand, or add dependencies when the standard library/current stack is sufficient.
- Validate all paths before deletion. Storage cleanup must remain under the configured Backtide
  storage directory and be best-effort only where the public contract says so.
- Treat prices, quantities, fees, leverage, timestamps, currency conversion, and order status as
  correctness-critical. State units and rounding rules; test boundaries, non-finite values,
  missing bars/ticks, duplicate/out-of-order data, and partial failure.
- For performance work, preserve correctness first and measure with Criterion or a representative
  profile. Do not trade away clear ownership, cancellation, or typed error handling for an
  unmeasured micro-optimization.
