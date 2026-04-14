"""Type stubs for `backtide.core.data` (auto-generated)."""

__all__ = [
    "Bar",
    "Country",
    "Currency",
    "DownloadResult",
    "Exchange",
    "Instrument",
    "InstrumentProfile",
    "InstrumentType",
    "Interval",
    "Provider",
    "download_instruments",
    "get_instruments",
    "list_instruments",
    "resolve_profiles",
]

from typing import Any

class Bar:
    """A single OHLCV candle for one symbol at one interval.

    The `adj_close` field is always populated. For instruments where price
    adjustment is meaningless (crypto, forex) it's set equal to `close`.

    Attributes
    ----------
    open_ts : int
        Bar open time in UTC (Unix seconds).

    close_ts : int
        Bar close time in UTC (Unix seconds).

    open_ts_exchange : int
        Bar open time in the exchange's local timezone (Unix seconds).

    open : float
        Price at bar open.

    high : float
        Highest price seen in the interval.

    low : float
        Lowest price seen in the interval.

    close : float
        Price at bar close.

    adj_close : float
        Split- and dividend-adjusted close. Equal to `close` when adjustment
        is not applicable.

    volume : float
        Traded volume in the instruments's native units.

    n_trades: int | None
        Number of trades that occurred this bar.

    See Also
    --------
    - backtide.data:Instrument
    - backtide.data:InstrumentType
    - backtide.data:Interval
    """

    adj_close: float
    close: float
    close_ts: int
    high: float
    low: float
    n_trades: int | None
    open: float
    open_ts: int
    open_ts_exchange: int
    volume: float

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...

class Country:
    """A country identified by its ISO 3166-1 alpha-3 code.

    Variant names are identical to their 3-letter ISO 3166-1 alpha-3 codes.

    Attributes
    ----------
    alpha2 : str
        The ISO 3166-1 alpha-2 code.

    alpha3 : str
        The ISO 3166-1 alpha-3 code.

    name : str
        The name of the country.

    flag : str
        The Unicode regional-indicator flag emoji for the country.

    See Also
    --------
    - backtide.data:Bar
    - backtide.data:Currency
    - backtide.data:Exchange
    """

    alpha2: str
    alpha3: str
    flag: str
    name: str

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __int__(self, /): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    @staticmethod
    def variants() -> list[Country]: ...

class Currency:
    """An ISO 4217 currency tied to a specific country or supranational union.

    Variant names are identical to their 3-letter ISO codes.

    Attributes
    ----------
    name : str
        The human-readable name of the currency.

    symbol : str
        The currency symbol as a UTF-8 string.

    country : [Country]
        The country that issues this currency.

    decimals : int
        The number of decimal places conventionally used when displaying
        amounts in this currency, per ISO 4217.

    symbol_prefix : bool
        Returns `true` if the currency symbol is conventionally placed before
        the numeric amount, or `false` if it follows the amount.

    See Also
    --------
    - backtide.data:Country
    - backtide.data:Exchange
    - backtide.data:Interval
    """

    country: Country
    decimals: int
    name: str
    symbol: str
    symbol_prefix: bool

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __int__(self, /): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    def format(self, amount) -> str: ...
    @staticmethod
    def get_default() -> Currency: ...
    @staticmethod
    def variants() -> list[Currency]: ...

class DownloadResult:
    """Summary returned by [`download_instruments`] after all tasks finish.

    Individual task failures are captured as warnings rather than aborting
    the entire download, so callers can report partial success.

    Attributes
    ----------
    n_succeeded : int
        Number of download tasks that succeeded.

    n_failed : int
        Number of download tasks that failed.

    warnings : list[str]
        Human-readable warning for each failed task.

    See Also
    --------
    - backtide.data:download_instruments
    - backtide.data:get_instruments
    - backtide.data:list_instruments
    """

    n_failed: int
    n_succeeded: int
    warnings: list[str]

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...

class Exchange:
    """A stock exchange.

    Variant names are identical to their 4-letter MIC (Market Identifier Code) codes.

    Attributes
    ----------
    mic : str
        The ISO 10383 Market Identifier Code.

    name : str
        The official name of the exchange.

    country : [Country]
        The country where the exchange is located.

    city : str
        The city where the exchange is headquartered.

    yahoo_code : str
        The Yahoo Finance suffix used to qualify ticker symbols for this
        exchange.

    currency : [Currency]
        The primary trading currency of the exchange.

    See Also
    --------
    - backtide.data:Country
    - backtide.data:Currency
    - backtide.data:Interval
    """

    city: str
    country: Country
    currency: Currency
    mic: str
    name: str
    yahoo_code: str

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __int__(self, /): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    @staticmethod
    def variants() -> list[Exchange]: ...

class Instrument:
    """A tradeable financial instrument.

    Each instrument is uniquely identified by a [symbol][nom-symbol] and
    belongs to exactly one [instrument type].

    Attributes
    ----------
    symbol : str
        Ticker symbol as used on the exchange.

    name : str
        Human-readable name of the instrument.

    base : str | [Currency] | None
        The currency of the tradeable instrument. Only defined for forex and
        crypto pairs.

    quote : str | [Currency]
        The currency the instrument trades on.

    instrument_type : [InstrumentType]
        Instrument type this instrument belongs to.

    exchange : str | [Exchange]
        The exchange this instrument is listed in.

    exchange_name : str
        Human-readable exchange name.

    See Also
    --------
    - backtide.data:Bar
    - backtide.data:InstrumentProfile
    - backtide.data:Interval
    """

    base: str | Currency | None
    exchange: str | Exchange
    instrument_type: InstrumentType
    name: str
    quote: str | Currency
    symbol: str

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...

class InstrumentProfile:
    """A wrapper around an instrument with additional metadata.

    Provides the information required to download an instrument, including the
    download period and required currency conversions to reach the `base_currency`.

    Attributes
    ----------
    instrument : [Instrument]
        Instrument for which to provide the metadata.

    earliest_ts : dict[[Interval], int]
        Per interval, the earliest timestamp for which there is data (in UNIX
        seconds).

    latest_ts : dict[[Interval], int]
        Per interval, the most recent timestamp for which there is data (in UNIX
        seconds).

    legs : list[str]
        Symbols of the currency pairs required to convert from this instrument
        to the base_currency.

    See Also
    --------
    - backtide.data:Bar
    - backtide.data:Instrument
    - backtide.data:Interval
    """

    base: str | Currency | None
    earliest_ts: dict[Interval, int]
    exchange: str | Exchange
    instrument: Instrument
    instrument_type: InstrumentType
    latest_ts: dict[Interval, int]
    legs: list[str]
    name: str
    quote: str | Currency
    symbol: str

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...

class InstrumentType:
    """The category an [`Instrument`] belongs to.

    See Also
    --------
    - backtide.data:Bar
    - backtide.data:Instrument
    - backtide.data:Interval
    """

    is_equity: Any

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __int__(self, /): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    @staticmethod
    def get_default() -> InstrumentType: ...
    def icon(self) -> str: ...
    @staticmethod
    def variants() -> list[InstrumentType]: ...

class Interval:
    """The time resolution of a single [`Bar`].

    Variants map to the canonical durations supported across providers.

    See Also
    --------
    - backtide.data:Bar
    - backtide.data:Instrument
    - backtide.data:InstrumentType
    """

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __int__(self, /): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    @staticmethod
    def get_default() -> Interval: ...
    def is_intraday(self) -> bool: ...
    def minutes(self) -> int: ...
    @staticmethod
    def variants() -> list[Interval]: ...

class Provider:
    """A supported market data provider.

    See Also
    --------
    - backtide.data:Instrument
    - backtide.data:InstrumentType
    - backtide.data:Interval
    """

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __hash__(self, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __int__(self, /): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    def intervals(self) -> list[Interval]: ...

def download_instruments(profiles, start=None, end=None, *, verbose=True) -> DownloadResult:
    """Download OHLCV data for the instruments described in a list of profiles.

    Concurrently downloads all instruments and legs, skipping data already stored
    in the database.

    Parameters
    ----------
    profiles : list[[InstrumentProfile]]
        Resolved instrument profiles (from [`resolve_profiles`]).

    start : int or None, default=None
        Optional start of the download window (Unix timestamp, inclusive). When
        given, per-instrument ranges are clamped so that no data before this timestamp
        is requested. If `None`, it uses the provider's earliest available date.

    end : int or None, default=None
        Optional end of the download window (Unix timestamp, exclusive). When
        given, per-instrument ranges are clamped so that no data after this timestamp
        is requested. If `None`, it uses the provider's latest available date.

    verbose : bool, default=True
        Whether to display a progress bar while downloading.

    Returns
    -------
    [DownloadResult]
        Summary of the download: succeeded/failed counts and per-task warnings.

    See Also
    --------
    - backtide.storage:get_bars
    - backtide.data:get_instruments
    - backtide.data:resolve_profiles

    Examples
    --------
    ```pycon
    from backtide.data import resolve_profiles, download_instruments

    profiles = resolve_profiles(["AAPL", "MSFT"], "stocks", "1d")
    result = download_instruments(profiles)  # no run
    print(result)
    ```
    """

def get_instruments(symbols, instrument_type) -> list[Instrument]:
    """Get instruments given their symbols.

    Parameters
    ----------
    symbols : str | [Instrument] | list[str | [Instrument]]
        Symbols for which to get the instruments. The symbols should be of the
        [canonical form][canonical-symbols] expected by backtide.

    instrument_type : str | [InstrumentType]
        For which [instrument type] to get the instruments.

    Returns
    -------
    list[[Instrument]]
        Instruments corresponding to the provided symbols.

    See Also
    --------
    - backtide.data:download_instruments
    - backtide.data:list_instruments
    - backtide.data:resolve_profiles

    Examples
    --------
    ```pycon
    from backtide.data import get_instruments

    print(get_instruments(["AAPL", "MSFT"], "stocks"))
    ```
    """

def list_instruments(
    instrument_type,
    exchange=None,
    *,
    limit=100,
    verbose=True,
) -> list[Instrument]:
    """List available instruments for a given instrument type.

    The function may not return all available instruments, but a subset of the most
    important ones instead.

    Parameters
    ----------
    instrument_type : str | [InstrumentType]
        For which [instrument type] to list the instruments.

    exchange : str | [Exchange] | list[str | [Exchange]] | None, default=None
        Optional exchange filter. If `None`, a default list of major exchanges is
        used. If specified, only query those exchanges and distribute `limit` evenly
        across them. This parameter is ignored for single-exchange providers.

    limit : int, default=100
        Maximum number of instruments to return. The actual number may be smaller,
        but not larger.

    verbose : bool, default=True
        Whether to display a progress spinner in the terminal.

    Returns
    -------
    list[[Instrument]]
        Instruments for the given instrument type.

    See Also
    --------
    - backtide.data:download_instruments
    - backtide.data:get_instruments
    - backtide.data:resolve_profiles

    Examples
    --------
    ```pycon
    from backtide.data import list_instruments

    print(list_instruments("crypto", limit=5))
    ```
    """

def resolve_profiles(
    symbols,
    instrument_type,
    interval,
    *,
    verbose=True,
) -> list[InstrumentProfile]:
    """Resolve the instrument profiles needed to download a set of symbols.

    Resolves all instruments corresponding to the provided symbols. Also resolves
    the required instruments to convert the given symbols to the base currency,
    including any triangulation intermediaries. Returns a flat, deduplicated list.

    Parameters
    ----------
    symbols : str | [Instrument] | list[str | [Instrument]]
        Symbols for which to get the instruments. The symbols should be of the
        [canonical form][canonical-symbols] expected by backtide.

    instrument_type : str | [InstrumentType]
        For which [instrument type] to get the instruments.

    interval : str | [Interval] | list[str | [Interval]]
        Interval(s) for which to resolve the download information.

    verbose : bool, default=True
        Whether to display a progress bar while resolving.

    Returns
    -------
    list[[InstrumentProfile]]
        Instrument profiles (direct instruments and currency legs, deduplicated).

    See Also
    --------
    - backtide.data:download_instruments
    - backtide.data:get_instruments
    - backtide.data:list_instruments

    Examples
    --------
    ```pycon
    from backtide.data import resolve_profiles

    print(resolve_profiles(["AAPL", "MSFT"], "stocks", "1d"))
    ```
    """
