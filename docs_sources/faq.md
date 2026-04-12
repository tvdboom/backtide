# Frequently asked questions
----------------------------

Here we try to give answers to some questions that have popped up
regularly. If you have any other questions, don't hesitate to create
a new [discussion](https://github.com/tvdboom/backtide/discussions)!

??? faq "What Python versions does Backtide support?"
    Backtide supports Python 3.11, 3.12, 3.13 and 3.14. The compiled Rust
    extension (`.pyd` / `.so`) is built separately for each version, so make
    sure the wheel you install matches your interpreter. See the
    [dependencies] page for more details.

??? faq "Do I need Rust installed to use Backtide?"
    **No.** Pre-built wheels on PyPI include the compiled Rust extension, so
    `pip install backtide` is all you need. You only need a Rust toolchain if
    you are building from source or [contributing][contributing] to the project.

??? faq "Which data providers are supported?"
    Backtide ships with four built-in providers: **Yahoo Finance**, **Binance**,
    **Coinbase** and **Kraken**. Yahoo covers stocks, ETFs, forex and crypto.
    Binance and Coinbase are crypto-only. Kraken supports crypto and major
    forex pairs. You can configure which provider is used for each instrument type
    in the [configuration].

??? faq "Can I use Backtide without an API key?"
    Yes. All four bundled providers use public REST endpoints that do not
    require authentication. No API key or account is needed to download
    market data.

??? faq "How far back does historical data go?"
    It depends on the provider and the interval. For daily bars, Yahoo
    Finance typically goes back to a symbol's first trade date. Intraday
    intervals have shorter rolling windows (e.g., 1-minute bars from Yahoo
    only cover the last 7 days). See the [data user guide][data] for the
    exact limits per provider and interval.

??? faq "Where is downloaded data stored?"
    All OHLCV bars are persisted in a local [DuckDB](https://duckdb.org/)
    database. By default the database lives at `.backtide/database.duckdb`
    relative to the working directory. You can change the path via the
    `storage_path` setting in [`DataConfig`]. See the [storage user guide][storage]
    for more details.

??? faq "How do I clear or reset the local database?"
    You can delete specific series through the Python API using [`delete_symbols`],
    or simply delete the `.backtide/` directory to start fresh.

??? faq "Can I use multiple providers for the same instrument type?"
    Not within a single configuration. Each instrument type maps to exactly one
    provider. If you need data from a different provider for the same instrument
    type, you can run separate sessions with different configuration files.

??? faq "Does Backtide support live / paper trading?"
    Not yet. Backtide is currently focused on historical backtesting. Live
    and paper trading are on the roadmap but not yet implemented.

??? faq "How do I launch the interactive UI?"
    Run `backtide launch` from the command line. This starts a local
    [Streamlit](https://streamlit.io/) server where you can download data,
    configure experiments, run backtests and analyze results — all from
    your browser. You can customize the address and port with the `--address`
    and `--port` flags.

??? faq "What is currency conversion and how does it work?"
    When an instrument is quoted in a different currency than your portfolio's
    `base_currency`, Backtide automatically downloads the required forex
    conversion pairs (legs) and converts prices to the base currency. The
    conversion path is controlled by `triangulation_strategy` in the
    [configuration]. See the [currency conversion][data] section for details.

??? faq "Can I run Backtide on macOS?"
    While the CI pipeline does not currently test on macOS, the code is
    platform-independent and the Rust core compiles on macOS. If you
    encounter issues, please open an
    [issue](https://github.com/tvdboom/backtide/issues).

??? faq "How do I run the benchmarks?"
    Backtide uses [Criterion.rs](https://github.com/bheisler/criterion.rs)
    for performance benchmarking. Run all benchmarks with
    `cargo bench --manifest-path backtide_core/Cargo.toml`, or use
    `tox -e bench`. See the [contributing] page for more details.

??? faq "How do I report a bug or request a feature?"
    Open an [issue](https://github.com/tvdboom/backtide/issues) on GitHub.
    For bugs, include a minimal reproduction scenario. For features, describe
    the use case and expected behavior. See the [contributing] guidelines for
    more details.
