"""Type stubs for `backtide.core.config` (auto-generated)."""

__all__ = [
    "Config",
    "DataConfig",
    "DataFrameLibrary",
    "DisplayConfig",
    "GeneralConfig",
    "LogLevel",
    "TriangulationStrategy",
    "get_config",
    "load_config",
    "set_config",
]

from typing import ClassVar

from backtide.core.data import Currency, InstrumentType, Provider

class Config:
    """Backtide configuration.

    Read more in the [user guide][configuration].

    Attributes
    ----------
    general : [GeneralConfig]
        Portfolio-wide settings.

    data : [DataConfig]
        Settings that control how market data is fetched and stored.

    display : [DisplayConfig]
        Settings that control how values are presented in the application's
        frontend.

    See Also
    --------
    - backtide.config:get_config
    - backtide.config:load_config
    - backtide.config:set_config

    """

    data: DataConfig
    display: DisplayConfig
    general: GeneralConfig

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    def to_dict(self) -> dict: ...

class DataConfig:
    """Configuration for data parameters.

    The data parameters control how and where market data is fetched and
    stored. Read more in the [user guide][configuration].

    Attributes
    ----------
    storage_path : str, default=".backtide"
        File-system path to the location to store the database and cache.

    providers : dict[[InstrumentType], [Provider]]
        Which data provider to use for each instrument type. When constructing,
        it defaults to: `{"stocks": "yahoo", "etf": "yahoo", "forex": "yahoo",
        "crypto": "binance"}`.

    dataframe_library : [DataFrameLibrary], default="pandas"
        Which DataFrame library to use for tabular data exchanged with user
        code (e.g., strategy function parameters, storage query results,
        indicator inputs/outputs). Choose from: "pandas", "polars", "numpy".

    See Also
    --------
    - backtide.config:get_config
    - backtide.config:load_config
    - backtide.config:set_config

    """

    dataframe_library: DataFrameLibrary
    providers: dict[InstrumentType, Provider]
    storage_path: str

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    def to_dict(self) -> dict: ...

class DataFrameLibrary:
    """DataFrame library used for returning tabular data.

    Controls which DataFrame library is used when storage functions return
    tabular data. Read more in the [user guide][configuration].

    Attributes
    ----------
    class_name : str
        Return the Python class name.

    """

    class_name: str

    Numpy: ClassVar[DataFrameLibrary]
    Pandas: ClassVar[DataFrameLibrary]
    Polars: ClassVar[DataFrameLibrary]

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
    def variants() -> list[DataFrameLibrary]: ...

class DisplayConfig:
    """Configuration for display parameters.

    The display parameters control how values are presented in the UI
    application. Read more in the [user guide][configuration].

    Attributes
    ----------
    date_format : str, default="YYYY-MM-DD"
        Format in which to display dates in [momentjs] style. Valid formats include
        `YYYY/MM/DD`, `DD/MM/YYYY`, or `MM/DD/YYYY` and can also use a period (.) or
        hyphen (-) as separators.

    time_format : str, default="HH:MM"
        Format in which to display timestamps in [momentjs] style. Valid formats
        include `HH:MM:SS` (include seconds), `hh:mm a` (show am/pm).

    timezone : str or None, default=None
        IANA timezone name. `None` to use the system's local timezone.

    logokit_api_key : str or None, default=None
        API key for the [logokit] website, which is used to fetch images for instruments.
        If `None`, no images are loaded.

    address : str | None, default=None
        The address where the streamlit server will listen for client and browser
        connections. Use this if you want to bind the server to a specific
        address. If set, the server will only be available from this address,
        and not from any aliases (like localhost).

    port : int, default=8501
        TCP port the server listens on.

    See Also
    --------
    - backtide.config:get_config
    - backtide.config:load_config
    - backtide.config:set_config

    """

    address: str | None
    date_format: str
    logokit_api_key: str | None
    port: int
    time_format: str
    timezone: str | None

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    def datetime_format(self) -> str: ...
    def to_dict(self) -> dict: ...

class GeneralConfig:
    """Portfolio-wide settings.

    Attributes
    ----------
    base_currency : [Currency], default="USD"
        ISO 4217 currency code that all prices are normalized to.

    triangulation_strategy : [TriangulationStrategy], default="direct"
        With which approach to convert currencies to `base_currency`. Read more
        in the [user guide][currency-conversion].

    triangulation_fiat : [Currency], default="USD"
        The fiat currency used as an intermediate between a fiat currency and
        `base_currency`. This method is chosen when no direct conversion path exists
        or when this method has longer history and `triangulation_strategy="earliest"`
        For example, if converting `PLN -> THB` and no `PLN-THB` pair is available, the
        engine will route through this currency as `PLN` -> `triangulation_fiat` -> `THB`.
        The chosen currency is expected to have pairs with all the currencies the
        project works with.

    triangulation_crypto : str, default="USDT"
        The cryptocurrency used as an intermediate when no direct conversion
        path exists between a crypto and `base_currency`. For example, to calculate
        the value of `BTC`, the engine will route `BTC` -> `triangulation_crypto` ->
        `triangulation_crypto_pegged` -> `base_currency`. The selected crypto is
        expected to be a stablecoin pegged to the `triangulation_crypto_pegged`
        fiat currency.

    triangulation_crypto_pegged : str, default="USD"
        The fiat currency to which `triangulation_crypto` is pegged, for the
        purposes of bridging between the crypto and fiat conversion graphs. When
        a conversion path crosses the crypto/fiat boundary,
        the engine treats `triangulation_crypto`/`triangulation_crypto_pegged`
        as the crossing pair at parity 1:1.

    log_level : [LogLevel], default="warn"
        Minimum tracing log level. Choose from: "error", "warn", "info", "debug".

    See Also
    --------
    - backtide.config:get_config
    - backtide.config:load_config
    - backtide.config:set_config

    """

    base_currency: Currency
    log_level: LogLevel
    triangulation_crypto: str
    triangulation_crypto_pegged: str
    triangulation_fiat: Currency
    triangulation_strategy: TriangulationStrategy

    def __eq__(self, value, /): ...
    def __ge__(self, value, /): ...
    def __getstate__(self, /): ...
    def __gt__(self, value, /): ...
    def __init__(self, /, *args, **kwargs): ...
    def __le__(self, value, /): ...
    def __lt__(self, value, /): ...
    def __ne__(self, value, /): ...
    def __new__(cls, *args, **kwargs): ...
    def __repr__(self, /): ...
    def __str__(self, /): ...
    def to_dict(self) -> dict: ...

class LogLevel:
    """Tracing logging level.

    The minimum logging level that are displayed, i.e., if `log_level="info"`,
    all `info`, `warn` and `error` messages are displayed.

    """

    Debug: ClassVar[LogLevel]
    Error: ClassVar[LogLevel]
    Info: ClassVar[LogLevel]
    Trace: ClassVar[LogLevel]
    Warn: ClassVar[LogLevel]

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
    def variants() -> list[LogLevel]: ...

class TriangulationStrategy:
    """Strategy with which to triangulate currencies.

    With which approach to convert currencies to the `base_currency`. Read
    more in the [user guide][currency-conversion].

    """

    Direct: ClassVar[TriangulationStrategy]
    Earliest: ClassVar[TriangulationStrategy]

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
    def variants() -> list[TriangulationStrategy]: ...

def get_config() -> Config:
    """Get a copy of the current global configuration.

    Use this function to alter the configuration programmatically before
    updating the current config with [`set_config`]. Read more in the
    [user guide][configuration].

    Returns
    -------
    [Config]
        The current configuration.

    See Also
    --------
    - backtide.config:load_config
    - backtide.config:set_config

    Examples
    --------
    ```pycon
    from pprint import pprint
    from backtide.config import get_config

    # Load and display the current configuration
    cfg = get_config()
    pprint(cfg.to_dict())
    ```

    """

def load_config(path) -> Config:
    """Load a backtide configuration from a file.

    Use this function to update a configuration programmatically before updating
    the current config with [`set_config`]. The accepted file formats are: `toml`,
    `yaml`, `yml`, `json`. Read more in the [user guide][configuration].

    Parameters
    ----------
    path: str
        Location of the config file to load.

    Returns
    -------
    [Config]
        The loaded configuration.

    See Also
    --------
    - backtide.config:get_config
    - backtide.config:set_config

    Examples
    --------
    ```pycon
    from backtide.config import load_config, set_config

    # Use the configuration from a custom file location
    set_config(load_config("path/to/config.toml")) # norun
    ```

    """

def set_config(config):
    """Set the global configuration.

    The configuration can only be set before it's used anywhere, so call this
    function at the start of the process. If the configuration is already used
    by any backtide functionality, an exception is raised. Read more in the
    [user guide][configuration].

    Parameters
    ----------
    config: [Config]
        Configuration to set.

    See Also
    --------
    - backtide.config:get_config
    - backtide.config:load_config

    Examples
    --------
    ```pycon
    from backtide.config import get_config, set_config

    # Load the current configuration and change a value
    cfg = get_config()
    cfg.general.base_currency = "USD"

    # Update backtide's configuration
    set_config(cfg)  # norun

    cfg = get_config()
    print(cfg.general.base_currency)
    ```

    """
