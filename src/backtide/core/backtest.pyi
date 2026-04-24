"""Type stubs for `backtide.core.backtest` (auto-generated)."""

__all__ = [
    "AverageDirectionalIndex",
    "AverageTrueRange",
    "BollingerBands",
    "CommissionType",
    "CommodityChannelIndex",
    "ConversionPeriod",
    "CurrencyConversionMode",
    "DataExpConfig",
    "EmptyBarPolicy",
    "EngineExpConfig",
    "ExchangeExpConfig",
    "ExperimentConfig",
    "ExponentialMovingAverage",
    "GeneralExpConfig",
    "IndicatorExpConfig",
    "MovingAverageConvergenceDivergence",
    "OnBalanceVolume",
    "OrderType",
    "PortfolioExpConfig",
    "RelativeStrengthIndex",
    "SimpleMovingAverage",
    "StochasticOscillator",
    "StrategyExpConfig",
    "StrategyType",
    "VolumeWeightedAveragePrice",
    "WeightedMovingAverage",
]

from typing import Any, ClassVar

import numpy as np
import pandas as pd
import polars as pl

from backtide.core.data import Currency, InstrumentType, Interval

class AverageDirectionalIndex:
    """Quantifies trend strength on a scale of 0 to 100, regardless of direction.
    Values above 25 generally indicate a strong trend; below 20, a weak or
    ranging market. Useful for determining whether a market is trending or
    ranging before applying trend-following or mean-reversion strategies.

    Formula:

    $$
    \begin{aligned}
    +DI_t &= 100 \cdot \frac{Smoothed(+DM_t)}{ATR_t} \\\\
    -DI_t &= 100 \cdot \frac{Smoothed(-DM_t)}{ATR_t} \\\\
    DX_t &= 100 \cdot \frac{|+DI_t - (-DI_t)|}{+DI_t + (-DI_t)} \\\\
    ADX_t &= EMA_n(DX_t)
    \end{aligned}
    $$

    Read more on [Wikipedia][wiki-adx].

    Parameters
    ----------
    period : int, default=14
        Look-back window length.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:AverageTrueRange
    backtide.indicators:MovingAverageConvergenceDivergence
    backtide.indicators:RelativeStrengthIndex

    """

    period: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

class AverageTrueRange:
    """Measures market volatility by calculating the average of the true range
    over a period. The true range accounts for gaps between sessions. Useful
    for position sizing, setting stop-loss levels, and comparing volatility
    across instruments.

    Formula:

    $$
    \begin{aligned}
    TR_t &= \max(H_t - L_t,\; |H_t - C_{t-1}|,\; |L_t - C_{t-1}|) \\\\
    ATR_t &= \frac{1}{n} \sum_{i=0}^{n-1} TR_{t-i}
    \end{aligned}
    $$

    Read more on [Wikipedia][wiki-atr].

    Parameters
    ----------
    period : int, default=14
        Look-back window length.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:AverageDirectionalIndex
    backtide.indicators:BollingerBands
    backtide.indicators:SimpleMovingAverage

    """

    period: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

class BollingerBands:
    """Volatility bands placed above and below an n-period SMA. The bands widen
    during high volatility and contract during low volatility. Useful for
    volatility assessment, mean-reversion strategies, and breakout detection
    when price moves outside the bands.

    Formula:

    $$
    \begin{aligned}
    Upper_t &= SMA_t + k \cdot \sigma_t \\\\
    Lower_t &= SMA_t - k \cdot \sigma_t
    \end{aligned}
    $$

    where $\sigma_t$ is the rolling standard deviation over $n$ periods. Read
    more on [Wikipedia][wiki-bb].

    Parameters
    ----------
    period : int, default=20
        Number of bars for the moving average.
    std_dev : float, default=2.0
        Number of standard deviations.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:AverageTrueRange
    backtide.indicators:CommodityChannelIndex
    backtide.indicators:SimpleMovingAverage

    """

    period: Any
    std_dev: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

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

class CommodityChannelIndex:
    """Measures how far the typical price deviates from its statistical mean,
    identifying cyclical trends. Values above +100 suggest overbought
    conditions; below -100, oversold. Useful for identifying cyclical price
    patterns, spotting divergences, and timing entries in commodities and
    equities.

    Formula:

    $$
    \begin{aligned}
    TP_t &= \frac{H_t + L_t + C_t}{3} \\\\
    CCI_t &= \frac{TP_t - SMA_n(TP_t)}{0.015 \cdot MD_t}
    \end{aligned}
    $$

    where $MD_t$ is the mean absolute deviation of $TP$ over $n$ periods. Read
    more on [Wikipedia][wiki-cci].

    Parameters
    ----------
    period : int, default=20
        Look-back window length.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:BollingerBands
    backtide.indicators:RelativeStrengthIndex
    backtide.indicators:StochasticOscillator

    """

    period: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

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
        ISO-8601 start date. Ignored when `full_history` is `True`.

    end_date : str | None, default=None
        ISO-8601 end date.

    interval : str | [Interval], default="1d"
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

    benchmark : str, default=""
        Optional benchmark ticker symbol.

    exclusive_orders : bool, default=False
        Cancel pending orders when a new order is submitted.

    random_seed : int | None, default=None
        Fixed RNG seed for reproducibility.

    empty_bar_policy : str | [EmptyBarPolicy], default="forward_fill"
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

    allow_margin : bool, default=True
        Whether margin trading is enabled.

    max_leverage : float, default=1.0
        Maximum leverage ratio.

    initial_margin : float, default=50.0
        Initial margin percentage.

    maintenance_margin : float, default=25.0
        Maintenance margin percentage.

    margin_interest : float, default=0.0
        Annual interest rate on borrowed funds.

    allow_short_selling : bool, default=True
        Whether short selling is permitted.

    borrow_rate : float, default=0.0
        Annual borrow cost for short positions.

    max_position_size : int, default=100
        Max allocation to one position (% of portfolio).

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

class ExponentialMovingAverage:
    """A weighted moving average that gives exponentially more weight to recent
    prices, making it more responsive to new information than the SMA. Useful
    for faster trend detection, reducing lag in crossover systems, and as a
    building block for other indicators (MACD, ADX).

    Formula:

    $$EMA_t = \alpha \cdot C_t + (1 - \alpha) \cdot EMA_{t-1}$$

    where $\alpha = \frac{2}{n + 1}$. Read more on [Wikipedia][wiki-ema].

    Parameters
    ----------
    period : int, default=14
        Look-back window length.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:MovingAverageConvergenceDivergence
    backtide.indicators:SimpleMovingAverage
    backtide.indicators:WeightedMovingAverage

    """

    period: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

class GeneralExpConfig:
    """General metadata for an experiment.

    Attributes
    ----------
    name : str, default=""
        A human-readable name to identify this experiment.

    tags : list[str], default=[]
        Descriptive tags for organizing and filtering experiments.

    description : str, default=""
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
    - backtide.backtest:EngineExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:PortfolioExpConfig
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

class MovingAverageConvergenceDivergence:
    """A trend-following momentum indicator that shows the relationship between
    two EMAs. The MACD line is the difference between a fast and slow EMA;
    the signal line is an EMA of the MACD line itself. Useful for trend
    direction and momentum, signal line crossovers for entry/exit timing,
    and histogram divergence analysis.

    Formula:

    $$
    \begin{aligned}
    MACD_t &= EMA_{fast}(C_t) - EMA_{slow}(C_t) \\\\
    Signal_t &= EMA_{signal}(MACD_t)
    \end{aligned}
    $$

    Read more on [Wikipedia][wiki-macd].

    Parameters
    ----------
    fast_period : int, default=12
        Fast EMA period.
    slow_period : int, default=26
        Slow EMA period.
    signal_period : int, default=9
        Signal line EMA period.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:AverageDirectionalIndex
    backtide.indicators:ExponentialMovingAverage
    backtide.indicators:RelativeStrengthIndex

    """

    fast_period: Any
    signal_period: Any
    slow_period: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

class OnBalanceVolume:
    """A cumulative volume indicator that adds volume on up-close days and
    subtracts it on down-close days. Rising OBV confirms an uptrend;
    falling OBV confirms a downtrend. Useful for confirming price trends
    with volume and spotting divergences between price and volume momentum.

    Formula:

    $$OBV_t = \begin{cases} OBV_{t-1} + V_t & \text{if } C_t > C_{t-1} \\ OBV_{t-1} - V_t & \text{if } C_t < C_{t-1} \\ OBV_{t-1} & \text{otherwise} \end{cases}$$

    Read more on [Wikipedia][wiki-obv].

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:MovingAverageConvergenceDivergence
    backtide.indicators:RelativeStrengthIndex
    backtide.indicators:VolumeWeightedAveragePrice

    """

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

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

class PortfolioExpConfig:
    """Portfolio settings for an experiment.

    Attributes
    ----------
    initial_cash : int, default=10000
        Cash balance at the start of the simulation.

    base_currency : str | [Currency], default="USD"
        ISO 4217 code the portfolio is denominated in.

    starting_positions : dict[str, int], default={}
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
    initial_cash: int
    starting_positions: dict[str, int]

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

class RelativeStrengthIndex:
    """A momentum oscillator that measures the speed and magnitude of recent
    price changes on a scale of 0 to 100. Values above 70 are typically
    considered overbought; below 30, oversold. Useful for identifying
    overbought/oversold conditions, spotting divergences, and confirming
    trend strength.

    Formula:

    $$RSI = 100 - \frac{100}{1 + RS}$$

    where $RS = \frac{\text{avg gain over } n}{\text{avg loss over } n}$. Read
    more on [Wikipedia][wiki-rsi].

    Parameters
    ----------
    period : int, default=14
        Look-back window length.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:CommodityChannelIndex
    backtide.indicators:MovingAverageConvergenceDivergence
    backtide.indicators:StochasticOscillator

    """

    period: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

class SimpleMovingAverage:
    """The arithmetic mean of the last n closing prices. Used to smooth
    short-term fluctuations and identify the direction of a trend. Useful
    for trend identification, support/resistance levels, and crossover
    strategies (e.g., golden cross / death cross).

    Formula:

    $$SMA_t = \frac{1}{n} \sum_{i=0}^{n-1} C_{t-i}$$

    where $C_t$ is the closing price at time $t$ and $n$ is the period. Read
    more on [Wikipedia][wiki-sma].

    Parameters
    ----------
    period : int, default=14
        Look-back window length.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:BollingerBands
    backtide.indicators:ExponentialMovingAverage
    backtide.indicators:WeightedMovingAverage

    """

    period: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

class StochasticOscillator:
    """Compares the closing price to the high-low range over a period,
    producing a %K line and a smoothed %D signal line. Both oscillate
    between 0 and 100. Useful for overbought/oversold signals, %K/%D
    crossovers for entry/exit timing, and divergence analysis.

    Formula:

    $$
    \begin{aligned}
    \%K_t &= 100 \cdot \frac{C_t - L_n}{H_n - L_n} \\\\
    \%D_t &= SMA_d(\%K_t)
    \end{aligned}
    $$

    where $H_n$ and $L_n$ are the highest high and lowest low over $n$ periods.
    Read more on [Wikipedia][wiki-stoch].

    Parameters
    ----------
    k_period : int, default=14
        %K look-back period.
    d_period : int, default=3
        %D smoothing period.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:CommodityChannelIndex
    backtide.indicators:MovingAverageConvergenceDivergence
    backtide.indicators:RelativeStrengthIndex

    """

    d_period: Any
    k_period: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

class StrategyExpConfig:
    """Strategy settings for an experiment.

    Attributes
    ----------
    predefined_strategies : list[str | [StrategyType]], default=[]
        Built-in strategies to run.

    custom_strategies : list[tuple[str, str]], default=[]
        User-defined strategy code as `(name, code)` tuples.

    See Also
    --------
    - backtide.backtest:DataExpConfig
    - backtide.backtest:EngineExpConfig
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:GeneralExpConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:PortfolioExpConfig

    """

    custom_strategies: list[tuple[str, str]]
    predefined_strategies: list[str | StrategyType]

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
        """Return the description of the strategy.

        Returns
        -------
        str
            A human-readable summary of the strategy's logic.

        """
    @staticmethod
    def get_default() -> StrategyType:
        """Return the default variant.

        Returns
        -------
        self
            The default variant.

        """
    @staticmethod
    def variants() -> list[StrategyType]:
        """Return all variants.

        Returns
        -------
        list[self]
            All variants of this type.

        """

class VolumeWeightedAveragePrice:
    """The cumulative average price weighted by volume. Institutional traders
    use VWAP as a benchmark: buying below VWAP is considered favorable,
    selling above it likewise. Useful as an intraday trading benchmark,
    for assessing execution quality, and as dynamic support/resistance.

    Formula:

    $$VWAP_t = \frac{\sum_{i=1}^{t} TP_i \cdot V_i}{\sum_{i=1}^{t} V_i}$$

    where $TP_i = \frac{H_i + L_i + C_i}{3}$. Read more on [Wikipedia][wiki-vwap].

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:OnBalanceVolume
    backtide.indicators:SimpleMovingAverage
    backtide.indicators:WeightedMovingAverage

    """

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

class WeightedMovingAverage:
    """A moving average where each price is multiplied by a linearly decreasing
    weight, placing more emphasis on recent data than the SMA but with a
    different weighting scheme than the EMA. Useful when you want recent
    prices to matter more without the recursive smoothing of EMA.

    Formula:

    $$WMA_t = \frac{\sum_{i=0}^{n-1} (n - i) \cdot C_{t-i}}{\sum_{i=1}^{n} i}$$

    Read more on [Wikipedia][wiki-wma].

    Parameters
    ----------
    period : int, default=14
        Look-back window length.

    Attributes
    ----------
    acronym : str
        Short ticker-style acronym.
    name : str
        Human-readable indicator name.

    See Also
    --------
    backtide.indicators:ExponentialMovingAverage
    backtide.indicators:MovingAverageConvergenceDivergence
    backtide.indicators:SimpleMovingAverage

    """

    period: Any

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
    def compute(self, data) -> np.ndarray | pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : np.ndarray | pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """
