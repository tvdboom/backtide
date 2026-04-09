# Storage
---------

Backtide persists all downloaded OHLCV bars in a local [DuckDB](https://duckdb.org/) database
so that subsequent runs can reuse previously fetched data instead of hitting
the network again. The storage layer is fully automatic — bars are written as
they are downloaded — but convenience functions are exposed for inspection and
housekeeping.

The database file is located at the path specified by the `storage_path`
setting in [`DataConfig`]. By default, this is `.backtide/database.duckdb`,
relative to the working directory.
