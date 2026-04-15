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
    "download_bars",
    "fetch_instruments",
    "list_instruments",
    "resolve_profiles",
]

from typing import Any, ClassVar

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

    AFG: ClassVar[Country]
    AGO: ClassVar[Country]
    ALB: ClassVar[Country]
    ARE: ClassVar[Country]
    ARG: ClassVar[Country]
    ARM: ClassVar[Country]
    AUS: ClassVar[Country]
    AUT: ClassVar[Country]
    AZE: ClassVar[Country]
    BDI: ClassVar[Country]
    BEL: ClassVar[Country]
    BEN: ClassVar[Country]
    BGD: ClassVar[Country]
    BGR: ClassVar[Country]
    BHR: ClassVar[Country]
    BIH: ClassVar[Country]
    BLR: ClassVar[Country]
    BOL: ClassVar[Country]
    BRA: ClassVar[Country]
    BRN: ClassVar[Country]
    BTN: ClassVar[Country]
    BWA: ClassVar[Country]
    CAN: ClassVar[Country]
    CHE: ClassVar[Country]
    CHL: ClassVar[Country]
    CHN: ClassVar[Country]
    CIV: ClassVar[Country]
    CMR: ClassVar[Country]
    COD: ClassVar[Country]
    COG: ClassVar[Country]
    COL: ClassVar[Country]
    CRI: ClassVar[Country]
    CUB: ClassVar[Country]
    CYM: ClassVar[Country]
    CYP: ClassVar[Country]
    CZE: ClassVar[Country]
    DEU: ClassVar[Country]
    DNK: ClassVar[Country]
    DOM: ClassVar[Country]
    DZA: ClassVar[Country]
    ECU: ClassVar[Country]
    EGY: ClassVar[Country]
    ESP: ClassVar[Country]
    EST: ClassVar[Country]
    ETH: ClassVar[Country]
    EUR: ClassVar[Country]
    FIN: ClassVar[Country]
    FJI: ClassVar[Country]
    FRA: ClassVar[Country]
    GBR: ClassVar[Country]
    GEO: ClassVar[Country]
    GHA: ClassVar[Country]
    GRC: ClassVar[Country]
    GTM: ClassVar[Country]
    HKG: ClassVar[Country]
    HND: ClassVar[Country]
    HRV: ClassVar[Country]
    HTI: ClassVar[Country]
    HUN: ClassVar[Country]
    IDN: ClassVar[Country]
    IND: ClassVar[Country]
    IRL: ClassVar[Country]
    IRN: ClassVar[Country]
    IRQ: ClassVar[Country]
    ISL: ClassVar[Country]
    ISR: ClassVar[Country]
    ITA: ClassVar[Country]
    JAM: ClassVar[Country]
    JOR: ClassVar[Country]
    JPN: ClassVar[Country]
    KAZ: ClassVar[Country]
    KEN: ClassVar[Country]
    KGZ: ClassVar[Country]
    KHM: ClassVar[Country]
    KOR: ClassVar[Country]
    KWT: ClassVar[Country]
    LAO: ClassVar[Country]
    LBN: ClassVar[Country]
    LBY: ClassVar[Country]
    LKA: ClassVar[Country]
    LTU: ClassVar[Country]
    LUX: ClassVar[Country]
    LVA: ClassVar[Country]
    MAC: ClassVar[Country]
    MAR: ClassVar[Country]
    MDA: ClassVar[Country]
    MDV: ClassVar[Country]
    MEX: ClassVar[Country]
    MKD: ClassVar[Country]
    MLT: ClassVar[Country]
    MMR: ClassVar[Country]
    MNE: ClassVar[Country]
    MNG: ClassVar[Country]
    MOZ: ClassVar[Country]
    MRT: ClassVar[Country]
    MUS: ClassVar[Country]
    MYS: ClassVar[Country]
    NAM: ClassVar[Country]
    NGA: ClassVar[Country]
    NIC: ClassVar[Country]
    NLD: ClassVar[Country]
    NOR: ClassVar[Country]
    NPL: ClassVar[Country]
    NZL: ClassVar[Country]
    OMN: ClassVar[Country]
    PAK: ClassVar[Country]
    PAN: ClassVar[Country]
    PER: ClassVar[Country]
    PHL: ClassVar[Country]
    PNG: ClassVar[Country]
    POL: ClassVar[Country]
    PRT: ClassVar[Country]
    PRY: ClassVar[Country]
    PSE: ClassVar[Country]
    QAT: ClassVar[Country]
    ROU: ClassVar[Country]
    RUS: ClassVar[Country]
    RWA: ClassVar[Country]
    SAU: ClassVar[Country]
    SDN: ClassVar[Country]
    SEN: ClassVar[Country]
    SGP: ClassVar[Country]
    SLV: ClassVar[Country]
    SRB: ClassVar[Country]
    SUR: ClassVar[Country]
    SVK: ClassVar[Country]
    SVN: ClassVar[Country]
    SWE: ClassVar[Country]
    SYC: ClassVar[Country]
    SYR: ClassVar[Country]
    THA: ClassVar[Country]
    TJK: ClassVar[Country]
    TKM: ClassVar[Country]
    TTO: ClassVar[Country]
    TUN: ClassVar[Country]
    TUR: ClassVar[Country]
    TWN: ClassVar[Country]
    TZA: ClassVar[Country]
    UGA: ClassVar[Country]
    UKR: ClassVar[Country]
    URY: ClassVar[Country]
    USA: ClassVar[Country]
    UZB: ClassVar[Country]
    VEN: ClassVar[Country]
    VNM: ClassVar[Country]
    YEM: ClassVar[Country]
    ZAF: ClassVar[Country]
    ZMB: ClassVar[Country]
    ZWE: ClassVar[Country]

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

    AED: ClassVar[Currency]
    AFN: ClassVar[Currency]
    ALL: ClassVar[Currency]
    AMD: ClassVar[Currency]
    AOA: ClassVar[Currency]
    ARS: ClassVar[Currency]
    AUD: ClassVar[Currency]
    AZN: ClassVar[Currency]
    BAM: ClassVar[Currency]
    BDT: ClassVar[Currency]
    BGN: ClassVar[Currency]
    BHD: ClassVar[Currency]
    BND: ClassVar[Currency]
    BRL: ClassVar[Currency]
    CAD: ClassVar[Currency]
    CHF: ClassVar[Currency]
    CLP: ClassVar[Currency]
    CNY: ClassVar[Currency]
    COP: ClassVar[Currency]
    CRC: ClassVar[Currency]
    CZK: ClassVar[Currency]
    DKK: ClassVar[Currency]
    DOP: ClassVar[Currency]
    DZD: ClassVar[Currency]
    EGP: ClassVar[Currency]
    EUR: ClassVar[Currency]
    FJD: ClassVar[Currency]
    GBP: ClassVar[Currency]
    GEL: ClassVar[Currency]
    GHS: ClassVar[Currency]
    GTQ: ClassVar[Currency]
    HKD: ClassVar[Currency]
    HNL: ClassVar[Currency]
    HUF: ClassVar[Currency]
    IDR: ClassVar[Currency]
    ILS: ClassVar[Currency]
    INR: ClassVar[Currency]
    IQD: ClassVar[Currency]
    ISK: ClassVar[Currency]
    JMD: ClassVar[Currency]
    JOD: ClassVar[Currency]
    JPY: ClassVar[Currency]
    KES: ClassVar[Currency]
    KRW: ClassVar[Currency]
    KWD: ClassVar[Currency]
    KYD: ClassVar[Currency]
    KZT: ClassVar[Currency]
    LBP: ClassVar[Currency]
    LKR: ClassVar[Currency]
    LYD: ClassVar[Currency]
    MAD: ClassVar[Currency]
    MDL: ClassVar[Currency]
    MKD: ClassVar[Currency]
    MOP: ClassVar[Currency]
    MUR: ClassVar[Currency]
    MVR: ClassVar[Currency]
    MXN: ClassVar[Currency]
    MYR: ClassVar[Currency]
    MZN: ClassVar[Currency]
    NAD: ClassVar[Currency]
    NGN: ClassVar[Currency]
    NIO: ClassVar[Currency]
    NOK: ClassVar[Currency]
    NPR: ClassVar[Currency]
    NZD: ClassVar[Currency]
    OMR: ClassVar[Currency]
    PEN: ClassVar[Currency]
    PGK: ClassVar[Currency]
    PHP: ClassVar[Currency]
    PKR: ClassVar[Currency]
    PLN: ClassVar[Currency]
    PYG: ClassVar[Currency]
    QAR: ClassVar[Currency]
    RON: ClassVar[Currency]
    RSD: ClassVar[Currency]
    RUB: ClassVar[Currency]
    RWF: ClassVar[Currency]
    SAR: ClassVar[Currency]
    SCR: ClassVar[Currency]
    SEK: ClassVar[Currency]
    SGD: ClassVar[Currency]
    SRD: ClassVar[Currency]
    THB: ClassVar[Currency]
    TND: ClassVar[Currency]
    TRY: ClassVar[Currency]
    TTD: ClassVar[Currency]
    TWD: ClassVar[Currency]
    TZS: ClassVar[Currency]
    UAH: ClassVar[Currency]
    UGX: ClassVar[Currency]
    USD: ClassVar[Currency]
    UYU: ClassVar[Currency]
    UZS: ClassVar[Currency]
    VND: ClassVar[Currency]
    YER: ClassVar[Currency]
    ZAR: ClassVar[Currency]
    ZMW: ClassVar[Currency]

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
    """Summary returned by [`download_bars`] after all tasks finish.

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
    - backtide.data:download_bars
    - backtide.data:fetch_instruments
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

    BVMF: ClassVar[Exchange]
    XADS: ClassVar[Exchange]
    XAMS: ClassVar[Exchange]
    XASE: ClassVar[Exchange]
    XASX: ClassVar[Exchange]
    XATH: ClassVar[Exchange]
    XBKK: ClassVar[Exchange]
    XBOG: ClassVar[Exchange]
    XBOM: ClassVar[Exchange]
    XBRU: ClassVar[Exchange]
    XBUD: ClassVar[Exchange]
    XBUE: ClassVar[Exchange]
    XCAI: ClassVar[Exchange]
    XCOL: ClassVar[Exchange]
    XCSE: ClassVar[Exchange]
    XDFM: ClassVar[Exchange]
    XDHA: ClassVar[Exchange]
    XDUB: ClassVar[Exchange]
    XETR: ClassVar[Exchange]
    XHEL: ClassVar[Exchange]
    XHKG: ClassVar[Exchange]
    XICE: ClassVar[Exchange]
    XIDX: ClassVar[Exchange]
    XIST: ClassVar[Exchange]
    XJPX: ClassVar[Exchange]
    XKAR: ClassVar[Exchange]
    XKLS: ClassVar[Exchange]
    XKRX: ClassVar[Exchange]
    XKUW: ClassVar[Exchange]
    XLIM: ClassVar[Exchange]
    XLIS: ClassVar[Exchange]
    XLIT: ClassVar[Exchange]
    XLON: ClassVar[Exchange]
    XLUX: ClassVar[Exchange]
    XMAD: ClassVar[Exchange]
    XMEX: ClassVar[Exchange]
    XMIL: ClassVar[Exchange]
    XMOS: ClassVar[Exchange]
    XNAS: ClassVar[Exchange]
    XNCM: ClassVar[Exchange]
    XNGS: ClassVar[Exchange]
    XNSE: ClassVar[Exchange]
    XNYS: ClassVar[Exchange]
    XNZE: ClassVar[Exchange]
    XOSL: ClassVar[Exchange]
    XPAR: ClassVar[Exchange]
    XPHS: ClassVar[Exchange]
    XPRA: ClassVar[Exchange]
    XRIS: ClassVar[Exchange]
    XSAU: ClassVar[Exchange]
    XSES: ClassVar[Exchange]
    XSGO: ClassVar[Exchange]
    XSHE: ClassVar[Exchange]
    XSHG: ClassVar[Exchange]
    XSTC: ClassVar[Exchange]
    XSTO: ClassVar[Exchange]
    XSWX: ClassVar[Exchange]
    XTAI: ClassVar[Exchange]
    XTAL: ClassVar[Exchange]
    XTSX: ClassVar[Exchange]
    XWAR: ClassVar[Exchange]
    XWBO: ClassVar[Exchange]

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

    provider : [Provider]
        The data provider that sourced this instrument.

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
    provider: Provider
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

    Crypto: ClassVar[InstrumentType]
    Etf: ClassVar[InstrumentType]
    Forex: ClassVar[InstrumentType]
    Stocks: ClassVar[InstrumentType]

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

    FifteenMinutes: ClassVar[Interval]
    FiveMinutes: ClassVar[Interval]
    FourHours: ClassVar[Interval]
    OneDay: ClassVar[Interval]
    OneHour: ClassVar[Interval]
    OneMinute: ClassVar[Interval]
    OneWeek: ClassVar[Interval]
    ThirtyMinutes: ClassVar[Interval]

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

    Binance: ClassVar[Provider]
    Coinbase: ClassVar[Provider]
    Kraken: ClassVar[Provider]
    Yahoo: ClassVar[Provider]

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

def download_bars(profiles, start=None, end=None, *, verbose=True) -> DownloadResult:
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
    - backtide.storage:query_bars
    - backtide.data:fetch_instruments
    - backtide.data:resolve_profiles

    Examples
    --------
    ```pycon
    from backtide.data import resolve_profiles, download_bars

    profiles = resolve_profiles(["AAPL", "MSFT"], "stocks", "1d")
    result = download_bars(profiles)
    print(result)
    ```

    """

def fetch_instruments(symbols, instrument_type) -> list[Instrument]:
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
    - backtide.data:download_bars
    - backtide.data:list_instruments
    - backtide.data:resolve_profiles

    Examples
    --------
    ```pycon
    from backtide.data import fetch_instruments

    print(fetch_instruments(["AAPL", "MSFT"], "stocks"))
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

    Returns instruments already stored in the database first.  Only when the
    DB holds fewer than `limit` matching rows does it fall back to the network
    provider to fill the gap.  Network results are persisted so that subsequent
    calls can be served entirely from storage.

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
    - backtide.data:download_bars
    - backtide.data:fetch_instruments
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
    - backtide.data:download_bars
    - backtide.data:fetch_instruments
    - backtide.data:list_instruments

    Examples
    --------
    ```pycon
    from backtide.data import resolve_profiles

    print(resolve_profiles(["AAPL", "MSFT"], "stocks", "1d"))
    ```

    """
