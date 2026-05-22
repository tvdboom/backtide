"""Type stubs for `backtide.core.backtest` (auto-generated)."""

__all__ = [
    "CommissionType",
    "ConversionPeriod",
    "CurrencyConversionMode",
    "DataExpConfig",
    "EmptyBarPolicy",
    "EngineExpConfig",
    "EquitySample",
    "ExchangeExpConfig",
    "ExperimentConfig",
    "ExperimentResult",
    "ExperimentStatus",
    "GeneralExpConfig",
    "IndicatorExpConfig",
    "Order",
    "OrderRecord",
    "OrderStatus",
    "OrderType",
    "Portfolio",
    "PortfolioExpConfig",
    "RunResult",
    "State",
    "StrategyExpConfig",
    "Trade",
    "experiment_log",
    "request_abort",
    "run_experiment",
]

from typing import Any, ClassVar

from backtide.core.data import Currency, InstrumentType, Interval

from backtide.sizers import BaseSizer

class CommissionType:
    """How trading commissions are calculated.

    Each variant represents a different fee structure applied to
    every executed order during the simulation.

    See Also
    --------
    - backtide.data:Currency
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:OrderType

    """

    Fixed: ClassVar[CommissionType]
    Percentage: ClassVar[CommissionType]
    PercentagePlusFixed: ClassVar[CommissionType]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __int__(self, /):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    @staticmethod
    def get_default() -> CommissionType:
        """Return the default variant.

        Returns
        -------
        self
            The default variant.

        """
    @staticmethod
    def variants() -> list[CommissionType]:
        """Return all variants.

        Returns
        -------
        list[self]
            All variants of this type.

        """

class ConversionPeriod:
    """The period at which foreign currency balances are converted.

    Used in combination with [`CurrencyConversionMode.EndOfPeriod`][CurrencyConversionMode]
    to specify the frequency of automatic conversions.

    See Also
    --------
    - backtide.data:Currency
    - backtide.backtest:CurrencyConversionMode
    - backtide.backtest:ExchangeExpConfig

    """

    Day: ClassVar[ConversionPeriod]
    Month: ClassVar[ConversionPeriod]
    Week: ClassVar[ConversionPeriod]
    Year: ClassVar[ConversionPeriod]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __int__(self, /):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    @staticmethod
    def get_default() -> ConversionPeriod:
        """Return the default variant.

        Returns
        -------
        self
            The default variant.

        """
    @staticmethod
    def variants() -> list[ConversionPeriod]:
        """Return all variants.

        Returns
        -------
        list[self]
            All variants of this type.

        """

class CurrencyConversionMode:
    """How foreign currency proceeds are converted back to the base currency.

    Determines the timing and conditions under which non-base-currency
    balances are exchanged. The chosen mode affects cash flow timing
    and may influence simulation results when exchange rates fluctuate.

    Attributes
    ----------
    name : str
        The human-readable display name of the variant.

    See Also
    --------
    - backtide.backtest:ConversionPeriod
    - backtide.data:Currency
    - backtide.backtest:ExchangeExpConfig

    """

    name: str

    CustomInterval: ClassVar[CurrencyConversionMode]
    EndOfPeriod: ClassVar[CurrencyConversionMode]
    HoldUntilThreshold: ClassVar[CurrencyConversionMode]
    Immediate: ClassVar[CurrencyConversionMode]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __int__(self, /):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    @staticmethod
    def get_default() -> CurrencyConversionMode:
        """Return the default variant.

        Returns
        -------
        self
            The default variant.

        """
    @staticmethod
    def variants() -> list[CurrencyConversionMode]:
        """Return all variants.

        Returns
        -------
        list[self]
            All variants of this type.

        """

class DataExpConfig:
    """Data settings for an experiment.

    Attributes
    ----------
    instrument_type : str | [InstrumentType], default="stocks"
        The category of financial instrument.

    symbols : list[str], default=[]
        Ticker symbols included in the backtest.

    full_history : bool, default=True
        If `True`, use the maximum available history for every symbol.

    start_date : str | None, default=None
        ISO-8601 start date (YYYY-MM-DD). Ignored when `full_history` is `True`.

    end_date : str | None, default=None
        ISO-8601 end date (YYYY-MM-DD). Ignored when `full_history` is `True`.

    interval : [Interval], default="1d"
        Bar interval.

    See Also
    --------
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:IndicatorExpConfig

    """

    end_date: str | None
    full_history: bool
    instrument_type: str | InstrumentType
    interval: Interval
    start_date: str | None
    symbols: list[str]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def to_dict(self) -> dict:
        """Convert to a dictionary.

        Returns
        -------
        dict
            Self as dict.

        """

class EmptyBarPolicy:
    """How to handle bars with no trading activity.

    Controls what the engine does when a bar has no market data.

    Attributes
    ----------
    name : str
        The human-readable display name of the variant.

    See Also
    --------
    - backtide.data:Bar
    - backtide.data:Instrument
    - backtide.data:Interval

    """

    name: str

    FillWithNaN: ClassVar[EmptyBarPolicy]
    ForwardFill: ClassVar[EmptyBarPolicy]
    Skip: ClassVar[EmptyBarPolicy]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __int__(self, /):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    @staticmethod
    def get_default() -> EmptyBarPolicy:
        """Return the default variant.

        Returns
        -------
        self
            The default variant.

        """
    @staticmethod
    def variants() -> list[EmptyBarPolicy]:
        """Return all variants.

        Returns
        -------
        list[self]
            All variants of this type.

        """

class EngineExpConfig:
    """Engine / simulation settings for an experiment.

    Attributes
    ----------
    warmup_period : int, default=0
        Bars to skip before the strategy starts.

    trade_on_close : bool, default=False
        Fill orders at the close price of the current bar.

    risk_free_rate : float, default=0.0
        Annualised risk-free rate for metrics.

    exclusive_orders : bool, default=False
        Cancel pending orders when a new order is submitted.

    empty_bar_policy : str | [EmptyBarPolicy], default="forward_fill"
        How to handle bars with no data.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig

    """

    empty_bar_policy: str | EmptyBarPolicy
    exclusive_orders: bool
    risk_free_rate: float
    trade_on_close: bool
    warmup_period: int

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def to_dict(self) -> dict:
        """Convert to a dictionary.

        Returns
        -------
        dict
            Self as dict.

        """

class EquitySample:
    """A single equity-curve sample taken once per simulated bar.

    Attributes
    ----------
    timestamp : int
        UTC timestamp in seconds since the Unix epoch.

    equity : float
        Total portfolio value (cash + positions) in the base currency.

    cash : dict[str | Currency, float]
        Cash balance per currency at this bar.

    drawdown : float
        Running drawdown (negative or zero) versus the all-time high
        equity, expressed as a fraction (e.g., -0.12 = -12%).

    See Also
    --------
    - backtide.backtest:ExperimentResult
    - backtide.analysis:plot_pnl
    - backtide.backtest:RunResult

    """

    cash: dict[str | Currency, float]
    drawdown: float
    equity: float
    timestamp: int

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __setstate__(self, state):
        ...
    def __str__(self, /):
        ...

class ExchangeExpConfig:
    """Exchange and execution settings for an experiment.

    Attributes
    ----------
    commission_type : str | [CommissionType], default="percentage"
        Fee structure applied to every executed order.

    commission_pct : float, default=0.1
        Percentage commission per trade.

    commission_fixed : float, default=0.0
        Fixed commission per trade.

    slippage : float, default=0.05
        Simulated market-impact percentage.

    allowed_order_types : list[str | [OrderType]], default=["market"]
        Which order types the engine accepts.

    partial_fills : bool, default=False
        Whether to simulate partial order fills.

    allow_margin : bool, default=False
        Whether margin trading is enabled.

    max_leverage : float, default=2.0
        Maximum leverage ratio.

    initial_margin : float, default=50.0
        Initial margin percentage.

    maintenance_margin : float, default=25.0
        Maintenance margin percentage.

    margin_interest : float, default=0.0
        Annual interest rate on borrowed funds.

    raise_on_margin_limit : bool, default=False
        If `True`, the engine raises an error when an order would breach
        `max_leverage` or when equity falls below `maintenance_margin`.
        If `False`, orders are auto-shrunk or rejected and a warning is
        recorded instead. Does not affect `max_position_size`, which
        always rejects (never aborts) independently.

    allow_short_selling : bool, default=False
        Whether short selling is permitted.

    borrow_rate : float, default=0.0
        Annual borrow cost for short positions.

    raise_on_short_violation : bool, default=False
        If `True`, the engine raises an error when a sell order would
        create or increase a short position while `allow_short_selling`
        is `False`. If `False`, such orders are silently rejected with a
        warning instead of aborting the run.

    max_position_size : int, default=100
        Max allocation to one position (% of portfolio equity).

    conversion_mode : str | [CurrencyConversionMode], default="immediate"
        How foreign-currency proceeds are converted.

    conversion_threshold : float | None, default=None
        Threshold for `HoldUntilThreshold` mode.

    conversion_period : str | [ConversionPeriod] | None, default=None
        Period for `EndOfPeriod` mode.

    conversion_interval : int | None, default=None
        Bar count for `CustomInterval` mode.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:PortfolioExpConfig

    """

    allow_margin: bool
    allow_short_selling: bool
    allowed_order_types: list[str | OrderType]
    borrow_rate: float
    commission_fixed: float
    commission_pct: float
    commission_type: str | CommissionType
    conversion_interval: int | None
    conversion_mode: str | CurrencyConversionMode
    conversion_period: str | ConversionPeriod | None
    conversion_threshold: float | None
    initial_margin: float
    maintenance_margin: float
    margin_interest: float
    max_leverage: float
    max_position_size: int
    partial_fills: bool
    raise_on_margin_limit: bool
    raise_on_short_violation: bool
    slippage: float

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def to_dict(self) -> dict:
        """Convert to a dictionary.

        Returns
        -------
        dict
            Self as dict.

        """

class ExperimentConfig:
    """Complete configuration for a single backtest experiment.

    Enum-valued settings accept both their enum variant and
    plain strings.

    Attributes
    ----------
    general : [GeneralExpConfig]
        Experiment name, tags and description.

    data : [DataExpConfig]
        Instrument type, symbols, date range and interval.

    portfolio : [PortfolioExpConfig]
        Initial cash, base currency and starting positions.

    strategy : [StrategyExpConfig]
        Strategies and benchmark to use in this experiment.

    indicators : [IndicatorExpConfig]
        Indicators to use in this experiment.

    exchange : [ExchangeExpConfig]
        Commission, slippage, order execution, margin and short-selling.

    engine : [EngineExpConfig]
        Warmup, timing and data-handling policies.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:StrategyExpConfig

    """

    data: DataExpConfig
    engine: EngineExpConfig
    exchange: ExchangeExpConfig
    general: GeneralExpConfig
    indicators: IndicatorExpConfig
    portfolio: PortfolioExpConfig
    strategy: StrategyExpConfig

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    @staticmethod
    def from_dict(data) -> ExperimentConfig:
        """Build an `ExperimentConfig` from a (possibly nested) dictionary.

        The dict may use the same nested structure produced by `to_toml`
        (with `general`, `data`, `portfolio`, etc. sections) **or**
        a flat key-value mapping. Missing keys silently fall back to defaults.

        Parameters
        ----------
        data : dict
            Source dictionary.

        Returns
        -------
        self
            The created instance.

        """
    @staticmethod
    def from_toml(text) -> ExperimentConfig:
        """Build an `ExperimentConfig` from a TOML string.

        Parameters
        ----------
        text : str
            TOML document.

        Returns
        -------
        self
            The created instance.

        """
    def to_dict(self) -> dict:
        """Convert the experiment configuration to a nested dictionary.

        Returns
        -------
        dict
            Self as dict.

        """
    def to_toml(self) -> str:
        """Serialise the configuration to a TOML string.

        The output is grouped into `[general]`, `[data]`,
        `[portfolio]`, `[strategy]`, `[indicators]`,
        `[exchange]` and `[engine]` sections.

        Returns
        -------
        str
            TOML representation of the config.

        """

class ExperimentResult:
    """The complete result of a single experiment.

    Attributes
    ----------
    experiment_id : str
        Unique identifier of the experiment.

    name : str
        Human-readable name (mirrors the config).

    tags : list[str]
        Tags assigned to the experiment.

    started_at : int
        UTC timestamp when the run started (in Unix seconds).

    finished_at : int
        UTC timestamp when the run finished (in Unix seconds).

    status : [ExperimentStatus]
        The status with which the experiment ended. Possible values are:

      - **success:** Every strategy succeeded.
      - **partial:** At least one strategy failed, but not all.
      - **error:** All strategies failed or the experiment could not run.

    strategies : list[[RunResult]]
        One result entry per evaluated strategy.

    warnings : list[str]
        Non-fatal warnings emitted during the run.

    See Also
    --------
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:run_experiment
    - backtide.backtest:RunResult

    """

    experiment_id: str
    finished_at: int
    name: str
    started_at: int
    status: ExperimentStatus
    strategies: list[RunResult]
    tags: list[str]
    warnings: list[str]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __setstate__(self, state):
        ...
    def __str__(self, /):
        ...

class ExperimentStatus:
    """The outcome status of a finished experiment.

    See Also
    --------
    - backtide.backtest:ExperimentResult
    - backtide.backtest:run_experiment

    """

    Error: ClassVar[ExperimentStatus]
    Partial: ClassVar[ExperimentStatus]
    Success: ClassVar[ExperimentStatus]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __int__(self, /):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def description(self) -> str:
        """A short human-readable description of this status.

        Returns
        -------
        str
            Description of the variant.

        """
    @staticmethod
    def variants() -> list[ExperimentStatus]:
        """Return all variants.

        Returns
        -------
        list[self]
            All variants of this type.

        """

class GeneralExpConfig:
    """General metadata for an experiment.

    Attributes
    ----------
    name : str, default=""
        A human-readable name to identify this experiment.

    icon : str, default=""
        An emoji icon to identify this experiment visually.

    tags : list[str], default=[]
        Descriptive tags for organizing and filtering experiments.

    description : str, default=""
        Free-text description of the experiment.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:StrategyExpConfig

    """

    description: str
    icon: str
    name: str
    tags: list[str]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def to_dict(self) -> dict:
        """Convert to a dictionary.

        Returns
        -------
        dict
            Self as dict.

        """

class IndicatorExpConfig:
    """Indicator settings for an experiment.

    Indicators are stored by name. Each name refers to a pickled indicator
    object saved in the local indicators directory.

    Attributes
    ----------
    indicators : list[str], default=[]
        Names of the indicators to use in this experiment. Each name must
        match a stored indicator.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:StrategyExpConfig

    """

    indicators: list[str]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def to_dict(self) -> dict:
        """Convert to a dictionary.

        Returns
        -------
        dict
            Self as dict.

        """

class Order:
    """A trading order submitted during the simulation.

    Read more in the [user guide][orders].

    Attributes
    ----------
    id : str
        Unique identifier of the order. Auto-generated if not provided. For
        [`OrderType.Cancel`][OrderType] orders, the `id` field identifies the
        target order that should be canceled. If an order with the same `id`
        already exists in the order book, the duplicate is rejected.

    symbol : str
        The ticker symbol this order targets.

    quantity : int | float | [BaseSizer], default=1
        Signed quantity (positive = buy, negative = sell). Fractional values
        are accepted only for crypto instruments. When a [sizer][sizers] is
        passed, the engine resolves the quantity automatically at order-processing
        time using portfolio equity converted to the asset's quote currency and
        the asset's price.

    order_type : [OrderType]
        The execution semantics (market, limit, stop-loss, etc...). Also accepts
        a string of the form PascalCase (`StopLoss`) or snake_case (`stop_loss`),
        case-insensitively.

    price : float | None
        Primary price for the order. The exact meaning depends on
        `order_type`:

    - `Market` / `Cancel` / `SettlePosition`: ignored.
    - `Limit` / `TakeProfit`: the limit / target price.
    - `StopLoss`: the stop (trigger) price.
    - `StopLossLimit` / `TakeProfitLimit`: the stop (trigger) price. Once hit, the
      order converts to a limit at `limit_price`.
    - `TrailingStop` / `TrailingStopLimit`: the trail amount in price units (positive).
      The engine maintains the running extreme internally.

    limit_price : float | None
        Secondary limit price used by the `StopLossLimit`, `TakeProfitLimit` and
        `TrailingStopLimit` order types. Once the stop component triggers, the order
        converts to a limit order resting at this price. Ignored for all other order
        types.

    See Also
    --------
    - backtide.backtest:OrderType
    - backtide.backtest:Portfolio
    - backtide.backtest:State

    """

    id: str
    limit_price: float | None
    order_type: OrderType
    price: float | None
    quantity: int | float | BaseSizer
    sizer: Any
    symbol: str

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...

class OrderRecord:
    """A record of an order as resolved by the engine.

    Attributes
    ----------
    order : [Order]
        The original order.

    timestamp : int
        The bar timestamp at which the order was processed (in Unix seconds).

    status : [OrderStatus]
        One of: `filled`, `canceled`, `rejected` or `pending`.

    fill_price : float | None
        Average fill price. `None` if not filled.

    reason : str
        Human-readable note (rejection / cancellation reason).

    commission : float
        Commission charged on the fill, in the order's quote currency.
        Zero for non-filled orders.

    pnl : float | None
        Realised profit & loss attributable to this order, in the base currency,
        after commission. Populated only on closing fills (sell that flattens /
        reduces an existing long, or buy-to-cover). `None` for opening fills,
        cancellations and rejections.

    See Also
    --------
    - backtide.backtest:Order
    - backtide.backtest:RunResult
    - backtide.backtest:Trade

    """

    commission: float
    fill_price: float | None
    order: Order
    pnl: float | None
    reason: str
    status: OrderStatus
    timestamp: int

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __setstate__(self, state):
        ...
    def __str__(self, /):
        ...

class OrderStatus:
    """The resolution status of a processed order.

    See Also
    --------
    - backtide.backtest:OrderRecord
    - backtide.backtest:RunResult

    """

    Canceled: ClassVar[OrderStatus]
    Filled: ClassVar[OrderStatus]
    Pending: ClassVar[OrderStatus]
    Rejected: ClassVar[OrderStatus]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __int__(self, /):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def description(self) -> str:
        """A short human-readable description of this status.

        Returns
        -------
        str
            Description of the variant.

        """
    @staticmethod
    def variants() -> list[OrderStatus]:
        """Return all variants.

        Returns
        -------
        list[self]
            All variants of this type.

        """

class OrderType:
    """The type of order that can be submitted to the exchange.

    Defines which execution semantics apply to a trade request.
    The engine validates that only allowed order types (configured
    in the exchange settings) are submitted during the simulation.

    Read more in the [user guide][orders].

    Attributes
    ----------
    name : str
        The human-readable display name of the variant.

    See Also
    --------
    - backtide.backtest:CommissionType
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:Order

    """

    name: str

    Cancel: ClassVar[OrderType]
    Limit: ClassVar[OrderType]
    Market: ClassVar[OrderType]
    SettlePosition: ClassVar[OrderType]
    StopLoss: ClassVar[OrderType]
    StopLossLimit: ClassVar[OrderType]
    TakeProfit: ClassVar[OrderType]
    TakeProfitLimit: ClassVar[OrderType]
    TrailingStop: ClassVar[OrderType]
    TrailingStopLimit: ClassVar[OrderType]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __int__(self, /):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def description(self) -> str:
        """Return a description of the order type.

        Returns
        -------
        str
            A brief explanation of the order's execution semantics.

        """
    @staticmethod
    def get_default() -> OrderType:
        """Return the default variant.

        Returns
        -------
        self
            The default variant.

        """
    @staticmethod
    def variants() -> list[OrderType]:
        """Return all variants.

        Returns
        -------
        list[self]
            All variants of this type.

        """

class Portfolio:
    """A snapshot of the portfolio's holdings at a point in time.

    Cash is represented as a mapping from currency to amount, allowing
    multi-currency portfolios. Positions are a mapping from ticker
    symbol to signed quantity (positive = long, negative = short).

    Attributes
    ----------
    cash : dict[[Currency], float]
        Cash balances keyed by currency. Each value is the amount held
        in that currency.

    positions : dict[str, float]
        Open positions keyed by ticker symbol. Positive values are long
        positions, negative values are short positions. Fractional values
        are supported only for crypto instruments (e.g., 0.0234 BTC).

    orders : list[[Order]]
        Currently open (unfilled) orders.

    See Also
    --------
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:Order
    - backtide.backtest:State

    """

    cash: dict[Currency, float]
    orders: list[Order]
    positions: dict[str, float]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...

class PortfolioExpConfig:
    """Portfolio settings for an experiment.

    Attributes
    ----------
    initial_cash : int, default=10000
        Cash balance at the start of the simulation.

    base_currency : str | [Currency], default="USD"
        ISO 4217 code the portfolio is denominated in.

    starting_positions : dict[str, float], default={}
        Pre-loaded positions `{symbol: quantity}`. Fractional values are
        accepted only for crypto instruments.

    See Also
    --------
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:StrategyExpConfig

    """

    base_currency: str | Currency
    initial_cash: int
    starting_positions: dict[str, float]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def to_dict(self) -> dict:
        """Convert to a dictionary.

        Returns
        -------
        dict
            Self as dict.

        """

class RunResult:
    """Result of running a single strategy as part of an experiment.

    Attributes
    ----------
    strategy_id : str
        Unique identifier for this strategy run.

    strategy_name : str
        The name of the strategy.

    equity_curve : list[[EquitySample]]
        Per-bar equity samples in chronological order.

    trades : list[[Trade]]
        All round-trip trades closed during the run.

    orders : list[[OrderRecord]]
        All orders the engine processed (filled, canceled, rejected).

    metrics : dict[str, float]
        Summary metrics (total_return, sharpe, max_drawdown, ...).

    base_currency : [Currency]
        The portfolio's base currency for this run. Equity, PnL and drawdown
        values stored on the run are denominated in this currency.

    error : str | None
        `None` on success. Otherwise, the first error raised by the strategy
        during the run. Strategies that fail still produce a result row so the
        rest of the experiment isn't lost — the engine simply records the error
        and reports the experiment status as "failed".

    is_benchmark : bool
        Whether this run is the benchmark run for the experiment.

    See Also
    --------
    - backtide.backtest:EquitySample
    - backtide.backtest:ExperimentResult
    - backtide.storage:query_strategy_runs

    """

    base_currency: Currency
    equity_curve: list[EquitySample]
    error: str | None
    is_benchmark: bool
    metrics: dict[str, float]
    orders: list[OrderRecord]
    strategy_id: str
    strategy_name: str
    trades: list[Trade]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __setstate__(self, state):
        ...
    def __str__(self, /):
        ...

class State:
    """The simulation state passed to a strategy's `evaluate` method on every tick.

    Contains metadata about the current position in the simulation: the UTC
    timestamp of the bar being processed, the zero-based bar index, the total
    number of bars in the dataset, and whether the engine is still in the
    warmup phase (where indicators are computed but no orders are placed).

    Attributes
    ----------
    timestamp : int
        UTC timestamp of the current bar in seconds since the Unix epoch.

    bar_index : int
        Zero-based index of the current bar in the dataset.

    total_bars : int
        Total number of bars in the dataset.

    is_warmup : bool
        Whether the engine is currently in the warmup phase. During warmup
        indicators are computed but orders are not executed.

    datetime : datetime.datetime
        The `timestamp` as a timezone-aware datetime. Uses the timezone from
        `config.display.timezone`. Falls back to the system's local timezone
        if none is configured.

    See Also
    --------
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:Order
    - backtide.backtest:Portfolio

    """

    bar_index: int
    datetime: datetime.datetime
    is_warmup: bool
    timestamp: int
    total_bars: int

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...

class StrategyExpConfig:
    """Strategy settings for an experiment.

    Strategies are stored by name. Each name refers to a pickled strategy
    object saved in the local strategies directory.

    Attributes
    ----------
    benchmark : str | None, default=None
        Benchmark identifier. If it matches the name of one of the selected
        strategies it is treated as a strategy benchmark; otherwise it is
        assumed to be a ticker symbol and a passive Buy & Hold strategy is
        injected automatically. If `None`, no benchmark is used.

    strategies : list[str], default=[]
        Names of the strategies to use in this experiment. Each name must
        match a stored strategy.

    See Also
    --------
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:PortfolioExpConfig

    """

    benchmark: str | None
    strategies: list[str]

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def to_dict(self) -> dict:
        """Convert to a dictionary.

        Returns
        -------
        dict
            Self as dict.

        """

class Trade:
    """A single round-trip trade (open + close of a position).

    Attributes
    ----------
    symbol : str
        The traded instrument's symbol.

    quantity : float
        Signed quantity. Positive = long round trip, negative = short.
        Floating-point so fractional units are tracked exactly for crypto.

    entry_ts : int
        Open timestamp (in Unix seconds).

    exit_ts : int
        Close timestamp (in Unix seconds).

    entry_price : float
        Average fill price at entry, in the instrument's quote currency.

    exit_price : float
        Average fill price at exit.

    pnl : float
        Profit and loss in the base currency, after commission.

    See Also
    --------
    - backtide.backtest:Order
    - backtide.backtest:OrderRecord
    - backtide.backtest:RunResult

    """

    entry_price: float
    entry_ts: int
    exit_price: float
    exit_ts: int
    pnl: float
    quantity: float
    symbol: str

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __setstate__(self, state):
        ...
    def __str__(self, /):
        ...

def experiment_log(message, level=...):
    """Write a message to the active experiment's log file.

    This is intended to be called from a custom strategy's `evaluate()`
    method. The message is routed through the `tracing` layer so it
    ends up in the per-experiment `logs.txt` alongside engine events.

    """

def request_abort():
    """Signal the Rust engine to abort the current experiment."""

def run_experiment(config, verbose=True, strategy_overrides=None, indicator_overrides=None):
    """Low-level entry point that runs an already-built experiment
    configuration.

    This is **not** the public API — Python callers should use
    `backtide.backtest.run_experiment`, which handles kwargs
    translation and polymorphic strategies/indicators before
    delegating here.

    """
