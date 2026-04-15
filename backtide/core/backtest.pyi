"""Type stubs for `backtide.core.backtest` (auto-generated)."""

__all__ = [
    "CodeSnippet",
    "CommissionType",
    "ConversionPeriod",
    "CurrencyConversionMode",
    "DataExpConfig",
    "EmptyBarPolicy",
    "EngineExpConfig",
    "ExchangeExpConfig",
    "ExperimentConfig",
    "GeneralExpConfig",
    "IndicatorExpConfig",
    "IndicatorType",
    "OrderType",
    "PortfolioExpConfig",
    "StrategyExpConfig",
    "StrategyType",
]

from typing import ClassVar

from backtide.core.data import Currency, InstrumentType, Interval

class CodeSnippet:
    """A named snippet of custom Python code (strategy or indicator).

    Attributes
    ----------
    name : str
        Human-readable label for the snippet.

    code : str
        Python source code.

    See Also
    --------
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:StrategyExpConfig

    """

    code: str
    name: str

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
    def get_default() -> CommissionType: ...
    @staticmethod
    def variants() -> list[CommissionType]: ...

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
    def get_default() -> ConversionPeriod: ...
    @staticmethod
    def variants() -> list[ConversionPeriod]: ...

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
    def get_default() -> CurrencyConversionMode: ...
    @staticmethod
    def variants() -> list[CurrencyConversionMode]: ...

class DataExpConfig:
    """Data settings for an experiment.

    Attributes
    ----------
    instrument_type : str | [InstrumentType]
        The category of financial instrument.

    symbols : list[str]
        Ticker symbols included in the backtest.

    full_history : bool
        If `True`, use the maximum available history for every symbol.

    start_date : str | None
        ISO-8601 start date. Ignored when `full_history` is `True`.

    end_date : str | None
        ISO-8601 end date.

    interval : str | [Interval]
        Bar interval.

    See Also
    --------
    - backtide.backtest:EngineExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:PortfolioExpConfig
    - backtide.backtest:StrategyExpConfig

    """

    end_date: str | None
    full_history: bool
    instrument_type: str | InstrumentType
    interval: str | Interval
    start_date: str | None
    symbols: list[str]

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
    - backtide.data:Interval

    """

    name: str

    FillWithNaN: ClassVar[EmptyBarPolicy]
    ForwardFill: ClassVar[EmptyBarPolicy]
    Skip: ClassVar[EmptyBarPolicy]

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
    def get_default() -> EmptyBarPolicy: ...
    @staticmethod
    def variants() -> list[EmptyBarPolicy]: ...

class EngineExpConfig:
    """Engine / simulation settings for an experiment.

    Attributes
    ----------
    warmup_period : int
        Bars to skip before the strategy starts.

    trade_on_close : bool
        Fill orders at the close price of the current bar.

    risk_free_rate : float
        Annualised risk-free rate for metrics.

    benchmark : str
        Optional benchmark ticker symbol.

    exclusive_orders : bool
        Cancel pending orders when a new order is submitted.

    random_seed : int | None
        Fixed RNG seed for reproducibility.

    empty_bar_policy : str | [EmptyBarPolicy]
        How to handle bars with no data.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:PortfolioExpConfig
    - backtide.backtest:StrategyExpConfig

    """

    benchmark: str
    empty_bar_policy: str | EmptyBarPolicy
    exclusive_orders: bool
    random_seed: int | None
    risk_free_rate: float
    trade_on_close: bool
    warmup_period: int

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

class ExchangeExpConfig:
    """Exchange and execution settings for an experiment.

    Attributes
    ----------
    commission_type : str | [CommissionType]
        Fee structure applied to every executed order.

    commission_pct : float
        Percentage commission per trade.

    commission_fixed : float
        Fixed commission per trade.

    slippage : float
        Simulated market-impact percentage.

    allowed_order_types : list[str | [OrderType]]
        Which order types the engine accepts.

    partial_fills : bool
        Whether to simulate partial order fills.

    allow_margin : bool
        Whether margin trading is enabled.

    max_leverage : float
        Maximum leverage ratio.

    initial_margin : float
        Initial margin percentage.

    maintenance_margin : float
        Maintenance margin percentage.

    margin_interest : float
        Annual interest rate on borrowed funds.

    allow_short_selling : bool
        Whether short selling is permitted.

    borrow_rate : float
        Annual borrow cost for short positions.

    max_position_size : int
        Max allocation to one position (% of portfolio).

    conversion_mode : str | [CurrencyConversionMode]
        How foreign-currency proceeds are converted.

    conversion_threshold : float | None
        Threshold for `HoldUntilThreshold` mode.

    conversion_period : str | [ConversionPeriod] | None
        Period for `EndOfPeriod` mode.

    conversion_interval : int | None
        Bar count for `CustomInterval` mode.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:EngineExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:PortfolioExpConfig
    - backtide.backtest:StrategyExpConfig

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
    slippage: float

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
        Predefined and custom strategies.

    indicators : [IndicatorExpConfig]
        Built-in and custom indicators.

    exchange : [ExchangeExpConfig]
        Commission, slippage, order execution, margin and short-selling.

    engine : [EngineExpConfig]
        Warmup, timing, benchmark and data-handling policies.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:EngineExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:PortfolioExpConfig
    - backtide.backtest:StrategyExpConfig

    """

    data: DataExpConfig
    engine: EngineExpConfig
    exchange: ExchangeExpConfig
    general: GeneralExpConfig
    indicators: IndicatorExpConfig
    portfolio: PortfolioExpConfig
    strategy: StrategyExpConfig

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
    @staticmethod
    def from_dict(data) -> ExperimentConfig: ...
    @staticmethod
    def from_toml(text) -> ExperimentConfig: ...
    def to_dict(self) -> dict: ...
    def to_toml(self) -> str: ...

class GeneralExpConfig:
    """General metadata for an experiment.

    Attributes
    ----------
    name : str
        A human-readable name to identify this experiment.

    tags : list[str]
        Descriptive tags for organising and filtering experiments.

    description : str
        Free-text description of the experiment.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:EngineExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:PortfolioExpConfig
    - backtide.backtest:StrategyExpConfig

    """

    description: str
    name: str
    tags: list[str]

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

class IndicatorExpConfig:
    """Indicator settings for an experiment.

    Attributes
    ----------
    builtin_indicators : list[str | [IndicatorType]]
        Built-in indicators to compute.

    custom_indicators : list[[CodeSnippet]]
        User-defined indicator code snippets.

    See Also
    --------
    - backtide.backtest:CodeSnippet
    - backtide.backtest:DataExpConfig
    - backtide.backtest:EngineExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:PortfolioExpConfig
    - backtide.backtest:StrategyExpConfig

    """

    builtin_indicators: list[str | IndicatorType]
    custom_indicators: list[CodeSnippet]

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

class IndicatorType:
    """Built-in technical indicator type.

    Indicators are mathematical functions applied to price and volume
    data that quantify trends, momentum, volatility and other market
    characteristics.

    Attributes
    ----------
    name : str
        The human-readable name of the indicator.

    See Also
    --------
    - backtide.data:Bar
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:StrategyType

    """

    name: str

    ADX: ClassVar[IndicatorType]
    ATR: ClassVar[IndicatorType]
    BB: ClassVar[IndicatorType]
    CCI: ClassVar[IndicatorType]
    EMA: ClassVar[IndicatorType]
    MACD: ClassVar[IndicatorType]
    OBV: ClassVar[IndicatorType]
    RSI: ClassVar[IndicatorType]
    SMA: ClassVar[IndicatorType]
    STOCH: ClassVar[IndicatorType]
    VWAP: ClassVar[IndicatorType]
    WMA: ClassVar[IndicatorType]

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
    def description(self) -> str: ...
    @staticmethod
    def get_default() -> IndicatorType: ...
    @staticmethod
    def variants() -> list[IndicatorType]: ...

class OrderType:
    """The type of order that can be submitted to the exchange.

    Defines which execution semantics apply to a trade request.
    The engine validates that only allowed order types (configured
    in the exchange settings) are submitted during the simulation.

    Attributes
    ----------
    name : str
        The human-readable display name of the variant.

    See Also
    --------
    - backtide.backtest:CommissionType
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:StrategyType

    """

    name: str

    Limit: ClassVar[OrderType]
    Market: ClassVar[OrderType]
    SettlePosition: ClassVar[OrderType]
    StopLoss: ClassVar[OrderType]
    StopLossLimit: ClassVar[OrderType]
    TakeProfit: ClassVar[OrderType]
    TakeProfitLimit: ClassVar[OrderType]
    TrailingStop: ClassVar[OrderType]
    TrailingStopLimit: ClassVar[OrderType]

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
    def description(self) -> str: ...
    @staticmethod
    def get_default() -> OrderType: ...
    @staticmethod
    def variants() -> list[OrderType]: ...

class PortfolioExpConfig:
    """Portfolio settings for an experiment.

    Attributes
    ----------
    initial_cash : float
        Cash balance at the start of the simulation.

    base_currency : str | [Currency]
        ISO 4217 code the portfolio is denominated in.

    starting_positions : dict[str, int]
        Pre-loaded positions `{symbol: quantity}`.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:EngineExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:StrategyExpConfig

    """

    base_currency: str | Currency
    initial_cash: float
    starting_positions: dict[str, int]

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

class StrategyExpConfig:
    """Strategy settings for an experiment.

    Attributes
    ----------
    predefined_strategies : list[str | [StrategyType]]
        Built-in strategies to run.

    custom_strategies : list[CodeSnippet]
        User-defined strategy code snippets.

    See Also
    --------
    - backtide.backtest:CodeSnippet
    - backtide.backtest:DataExpConfig
    - backtide.backtest:EngineExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:PortfolioExpConfig

    """

    custom_strategies: list[CodeSnippet]
    predefined_strategies: list[str | StrategyType]

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

class StrategyType:
    """A predefined (built-in) strategy type.

    Each variant represents a complete trading strategy shipped with
    backtide. Predefined strategies can be selected alongside custom
    user-defined strategies for performance comparison.

    Attributes
    ----------
    name : str
        The human-readable display name of the strategy.

    is_rotation : bool
        Whether this is a portfolio rotation (multi-asset) strategy.

    See Also
    --------
    - backtide.backtest:IndicatorType
    - backtide.backtest:OrderType
    - backtide.backtest:StrategyExpConfig

    """

    is_rotation: bool
    name: str

    AdaptiveRsi: ClassVar[StrategyType]
    AlphaRsiPro: ClassVar[StrategyType]
    BollingerBands: ClassVar[StrategyType]
    BuyAndHold: ClassVar[StrategyType]
    DoubleTop: ClassVar[StrategyType]
    HybridAlphaRsi: ClassVar[StrategyType]
    Macd: ClassVar[StrategyType]
    Momentum: ClassVar[StrategyType]
    MultiBbRotation: ClassVar[StrategyType]
    RiskAverse: ClassVar[StrategyType]
    Roc: ClassVar[StrategyType]
    RocRotation: ClassVar[StrategyType]
    Rsi: ClassVar[StrategyType]
    Rsrs: ClassVar[StrategyType]
    RsrsRotation: ClassVar[StrategyType]
    SmaCrossover: ClassVar[StrategyType]
    SmaNaive: ClassVar[StrategyType]
    TripleRsiRotation: ClassVar[StrategyType]
    TurtleTrading: ClassVar[StrategyType]
    Vcp: ClassVar[StrategyType]

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
    def description(self) -> str: ...
    @staticmethod
    def get_default() -> StrategyType: ...
    @staticmethod
    def variants() -> list[StrategyType]: ...
