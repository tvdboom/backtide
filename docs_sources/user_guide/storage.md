# Storage
---------

Backtide persists all downloaded OHLCV bars in a local [DuckDB] database so
that subsequent runs can reuse previously fetched data instead of hitting
the network again. The storage layer is fully automatic — bars are written
as they are downloaded — but two convenience functions are exposed for
inspection and housekeeping.

<br>

## How it works

When you download data via [`get_download_info`] and [`download_assets`],
Backtide checks which bars are already present in the database and only
fetches the missing ranges. Downloaded bars are written immediately, so
even a partially completed download is available for reuse next time.

The database file is located at the path specified by the `storage_path`
setting in [`DataConfig`]. By default, this is `.backtide/database.duckdb`
relative to the working directory.

Every stored row belongs to a unique (symbol, interval, provider) group.
This means the same symbol can have data from multiple providers or
multiple intervals simultaneously without conflicts.

<br>

## Inspecting stored data

Use [`get_summary`] to list every (symbol, interval, provider) group
currently held in the database, together with the date range, row count,
and a sparkline of the most recent prices.

```pycon
from backtide.storage import get_summary

for row in get_summary():
    print(f"{row.symbol:>10} {row.interval:>3}  {row.provider:<8}  {row.n_rows} bars")
```

Each entry in the returned list is a [`StorageSummary`] object with the
following fields:

| Field | Description |
| --- | --- |
| `symbol` | Canonical symbol name. |
| `provider` | Data provider that fetched the bars. |
| `interval` | Bar interval (e.g. `1d`, `1h`). |
| `asset_type` | Asset type (e.g. `stocks`, `crypto`). |
| `first_ts` | Earliest `open_ts` in Unix seconds. |
| `last_ts` | Latest `open_ts` in Unix seconds. |
| `n_rows` | Total number of stored bars. |
| `sparkline` | Last 365 adjusted close values (oldest → newest). |

<br>

## Deleting stored data

Use [`delete_rows`] to remove bars from the database. You can target a
specific symbol or a list of symbols, and optionally filter by interval
and/or provider.

```pycon
from backtide.storage import delete_rows

# Delete everything for a single symbol
n = delete_rows("AAPL")  # norun
print(f"Deleted {n} rows")  # norun

# Delete only daily bars from Binance for multiple symbols
n = delete_rows(["BTC-USDT", "ETH-USDT"], interval="1d", provider="binance")  # norun
print(f"Deleted {n} rows")  # norun
```

If `interval` or `provider` is omitted (or `None`), the filter is not
applied for that dimension — i.e., all intervals or all providers are
matched.

<br>

## Storage location

The database is stored at the path defined by `storage_path` in the
[configuration]. The default value is `.backtide`, which means Backtide
creates a `.backtide/database.duckdb` file in your working directory.

To change the location, update the configuration before any data operations:

```pycon
from backtide.config import get_config, set_config

cfg = get_config()
cfg.data.storage_path = "/path/to/storage"  # norun
set_config(cfg)  # norun
```

[DuckDB]: https://duckdb.org

