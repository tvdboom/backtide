# Storage
---------

Backtide persists all downloaded OHLCV bars and dividend data in a local
[DuckDB](https://duckdb.org/) database so that subsequent runs can reuse previously fetched
data instead of hitting the network again. The storage layer is fully automatic
— bars are written as they are downloaded — but convenience functions are
exposed for inspection and housekeeping.

The database file is located at the path specified by the `storage_path`
setting in [`DataConfig`]. By default, this is `.backtide/database.duckdb`,
relative to the working directory.

<br>

## Querying data

All query functions return a dataframe whose type matches the configured [`DataFrameLibrary`].

| Function | Description |
|---|---|
| [`query_bars`] | Retrieve stored OHLCV bars, optionally filtered by symbol, interval and provider. |
| [`query_bars_summary`] | Get a compact summary of every stored series (symbol, interval, provider, date range, row count). |
| [`query_dividends`] | Retrieve stored dividend records, optionally filtered by symbol and provider. |
| [`query_instruments`] | List the instrument metadata cached during download. |


```pycon
from backtide.storage import query_bars, query_bars_summary

# Show everything in the database
summary = query_bars_summary()
print(summary.head())

# Fetch daily bars for a specific symbol
df = query_bars("AAPL", "1d")
print(df.head())
```

<br>

## Deleting data

Use [`delete_symbols`] to remove bars (and any orphaned dividend records)
from the database. You can target specific symbols, intervals and providers,
or pass a list of `(symbol, interval, provider)` triples for batch deletion.

```pycon
from backtide.storage import delete_symbols

# Delete all daily bars for AAPL
delete_symbols("AAPL", "1d")  # norun

# Delete everything for a specific provider
delete_symbols(provider="yahoo")  # norun
```

<br>

## Storage in the UI

The **Storage** page in the Streamlit application provides a visual overview
of all stored series. From there you can inspect date ranges, row counts and
sparklines, select series for analysis, or delete them in bulk.

The **Experiment** page also offers a *Use stored data* toggle. When enabled,
the backtest draws exclusively from the local database without downloading
new data — the available date range is determined entirely by what has already
been stored.
