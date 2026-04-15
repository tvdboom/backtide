"""Type stubs for `backtide.core.storage` (auto-generated)."""

__all__ = [
    "delete_symbols",
    "query_bars",
    "query_bars_summary",
    "query_dividends",
    "query_instruments",
]

import pandas as pd
import polars as pl

from backtide.core.data import Instrument

def delete_symbols(symbol=None, interval=None, provider=None, *, series=None) -> int:
    """Delete bars (and orphaned dividends) from the database.

    Accepts either individual arguments for a single symbol (or list of
    symbols), or a `series` list of `(symbol, interval, provider)` triples
    for bulk deletion. All deletions run in a single database transaction.

    Parameters
    ----------
    symbol : str | list[str] | None = None
        One or more symbols to delete. Mutually exclusive with `series`.

    interval : str | [Interval] | None = None
        The bar interval to remove. Applies to every symbol when `symbol`
        is given. Ignored when `series` is given.

    provider : str | [Provider] | None = None
        The data provider to remove. Applies to every symbol when `symbol`
        is given. Ignored when `series` is given.

    series : list[tuple[str, str, str]] | None = None
        Explicit list of `(symbol, interval, provider)` triples to delete.
        Mutually exclusive with `symbol`.

    Returns
    -------
    int
        Number of bar rows deleted.

    See Also
    --------
    - backtide.data:download_bars
    - backtide.storage:query_bars
    - backtide.storage:query_dividends

    Examples
    --------
    ```pycon
    from backtide.storage import delete_symbols

    # Delete all stored data for a single symbol
    delete_symbols("AAPL")  # norun

    # Delete daily bars for multiple symbols
    delete_symbols(["BTC-USDT", "ETH-USDT"], interval="1d")  # norun

    # Bulk-delete specific series
    delete_symbols(series=[("AAPL", "1d", "yahoo"), ("MSFT", "1h", "yahoo")])  # norun
    ```

    """

def query_bars() -> pd.DataFrame | pl.DataFrame:
    """Return all stored OHLCV bars as a dataframe.

    Each row represents a single bar. The dataframe columns are:
    `symbol`, `interval`, `provider`, `open_ts`, `close_ts`,
    `open_ts_exchange`, `open`, `high`, `low`, `close`, `adj_close`,
    `volume`, and `n_trades`.

    Returns
    -------
    pd.DataFrame | pl.DataFrame
        All bars currently held in the database.

    See Also
    --------
    - backtide.data:download_bars
    - backtide.storage:query_bars_summary
    - backtide.storage:query_dividends

    Examples
    --------
    ```pycon
    from backtide.storage import query_bars

    df = query_bars()
    print(df.head())
    ```

    """

def query_bars_summary() -> pd.DataFrame | pl.DataFrame:
    """Return a pre-aggregated summary of stored bars as a dataframe.

    Each row represents one (symbol, interval, provider) series. The `sparkline`
    column contains the last 365 `adj_close` values.

    Returns
    -------
    pd.DataFrame | pl.DataFrame
        One summary row per stored series.

    Examples
    --------
    ```pycon
    from backtide.storage import query_bars_summary

    df = query_bars_summary()
    print(df.head())
    ```

    """

def query_dividends() -> pd.DataFrame | pl.DataFrame:
    """Return all stored dividend events as a dataframe.

    Each row represents a single dividend payment. The DataFrame columns
    are: `symbol`, `provider`, `ex_date`, and `amount`.

    Returns
    -------
    pd.DataFrame | pl.DataFrame
        All dividend events currently held in the database.

    See Also
    --------
    - backtide.storage:delete_symbols
    - backtide.data:download_bars
    - backtide.storage:query_bars

    Examples
    --------
    ```pycon
    from backtide.storage import query_dividends

    df = query_dividends()
    print(df.head())
    ```

    """

def query_instruments(
    instrument_type=None,
    provider=None,
    exchange=None,
    *,
    limit=None,
) -> list[Instrument]:
    """Return stored instrument metadata, optionally filtered.

    When called with no arguments, returns all instruments. When
    ``instrument_type``, ``provider``, and/or ``exchange`` are given, only
    matching rows are returned.

    Parameters
    ----------
    instrument_type : str | InstrumentType | None, default=None
        Filter by instrument type.

    provider : str | Provider | None, default=None
        Filter by data provider.

    exchange : str | Exchange | list[str | Exchange] | None, default=None
        Filter by exchange. Accepts a single exchange or a list.

    limit : int | None, default=None
        Maximum number of instruments to return. ``None`` means no limit.

    Returns
    -------
    list[Instrument]
        Matching instruments from the database.

    Examples
    --------
    ```pycon
    from backtide.storage import query_instruments

    # All instruments
    all_instruments = query_instruments()

    # Filtered
    stocks = query_instruments("stocks", "yahoo", limit=100)

    # Filtered by exchange
    nyse = query_instruments("stocks", exchange="XNYS")
    ```

    """
