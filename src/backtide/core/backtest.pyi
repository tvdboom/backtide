"""Type stubs for `backtide.core.backtest` (auto-generated)."""

__all__ = [
    "AdaptiveRsi",
    "AlphaRsiPro",
    "AverageDirectionalIndex",
    "AverageTrueRange",
    "BollingerBands",
    "BollingerMeanReversion",
    "BuyAndHold",
    "CommissionType",
    "CommodityChannelIndex",
    "ConversionPeriod",
    "CurrencyConversionMode",
    "DataExpConfig",
    "DoubleTop",
    "EmptyBarPolicy",
    "EngineExpConfig",
    "EquitySample",
    "ExchangeExpConfig",
    "ExperimentConfig",
    "ExperimentResult",
    "ExponentialMovingAverage",
    "GeneralExpConfig",
    "HybridAlphaRsi",
    "IndicatorExpConfig",
    "Macd",
    "Momentum",
    "MovingAverageConvergenceDivergence",
    "MultiBollingerRotation",
    "OnBalanceVolume",
    "Order",
    "OrderRecord",
    "OrderType",
    "Portfolio",
    "PortfolioExpConfig",
    "RelativeStrengthIndex",
    "RiskAverse",
    "Roc",
    "RocRotation",
    "Rsi",
    "Rsrs",
    "RsrsRotation",
    "RunResult",
    "SimpleMovingAverage",
    "SmaCrossover",
    "SmaNaive",
    "State",
    "StochasticOscillator",
    "StrategyExpConfig",
    "Trade",
    "TripleRsiRotation",
    "TurtleTrading",
    "Vcp",
    "VolumeWeightedAveragePrice",
    "WeightedMovingAverage",
    "run_experiment",
]

from typing import Any, ClassVar

import numpy as np
import pandas as pd
import polars as pl

from backtide.core.data import Currency, InstrumentType, Interval

class AdaptiveRsi:
    """Relative Strength Index with a dynamically adaptive look-back period.

    Dynamically adjusts its look-back period based on current market volatility
    and cycle length. In calm, trending markets the period lengthens for smoother
    signals; in volatile or choppy regimes it shortens for faster reaction. Useful
    when a fixed-period RSI produces too many whipsaws or lags behind regime
    changes.

    Parameters
    ----------
    min_period : int, default=8
        Minimum adaptive RSI period.

    max_period : int, default=28
        Maximum adaptive RSI period.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:AlphaRsiPro
    backtide.strategies:HybridAlphaRsi
    backtide.strategies:Rsi

    """

    max_period: Any
    min_period: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class AlphaRsiPro:
    """Advanced Relative Strength Index with adaptive overbought/oversold levels.

    An advanced RSI variant that computes adaptive overbought and oversold
    thresholds based on recent volatility, and adds a trend-bias filter to
    avoid counter-trend entries. In strong uptrends the oversold level is
    raised so buy signals fire earlier; in downtrends the overbought level
    is lowered so sells trigger sooner. Useful for reducing false signals
    in trending markets compared to a plain RSI strategy.

    Parameters
    ----------
    period : int, default=14
        RSI look-back period.

    vol_window : int, default=20
        Window for the volatility-based level adjustment.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:AdaptiveRsi
    backtide.strategies:HybridAlphaRsi
    backtide.strategies:Rsi

    """

    period: Any
    vol_window: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class AverageDirectionalIndex:
    """Average Directional Index (ADX).

    Quantifies trend strength on a scale of 0 to 100, regardless of direction.
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
    """Average True Range (ATR).

    Measures market volatility by calculating the average of the true range
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
    """Bollinger Bands (BB).

    Volatility bands placed above and below an n-period SMA. The bands widen
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

class BollingerMeanReversion:
    """Mean-reversion strategy using Bollinger Band boundaries.

    A mean-reversion strategy that enters long when the price touches or
    crosses below the lower Bollinger Band and exits when it reaches the
    upper band. The assumption is that price will revert to its moving
    average after an extreme excursion. Useful in range-bound or
    mean-reverting markets.

    Parameters
    ----------
    period : int, default=20
        Number of bars for the Bollinger Band moving average.

    std_dev : float, default=2.0
        Number of standard deviations for the band width.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:MultiBollingerRotation
    backtide.strategies:Rsi
    backtide.strategies:SmaCrossover

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class BuyAndHold:
    """Passive baseline that buys once and holds indefinitely.

    The simplest possible strategy: buy on the very first bar and hold the
    position until the end of the simulation. Serves as the baseline
    benchmark against which all other strategies are compared. Equivalent
    to a passive index investment over the backtest window.

    Parameters
    ----------
    symbol : str | None, default=None
        Optional single ticker to buy and hold. When `None`, the strategy
        equal-weights all symbols visible in the experiment.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:Momentum
    backtide.strategies:SmaNaive
    backtide.strategies:TurtleTrading

    """

    symbol: str

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

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
    """Commodity Channel Index (CCI).

    Measures how far the typical price deviates from its statistical mean,
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
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:IndicatorExpConfig

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

class DoubleTop:
    """Chart-pattern breakout triggered by a double-top formation.

    Detects a double-top chart pattern — two consecutive peaks at roughly
    the same price level — and enters long on the subsequent breakout above
    the neckline. Includes a trend filter and volume confirmation to reduce
    false breakouts. Useful for pattern-recognition-based breakout trading.

    Parameters
    ----------
    lookback : int, default=60
        Number of bars to search for the double-top pattern.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:Momentum
    backtide.strategies:TurtleTrading
    backtide.strategies:Vcp

    """

    lookback: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

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

    random_seed : int | None, default=None
        Fixed RNG seed for reproducibility.

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
        equity, expressed as a fraction (e.g. -0.12 = -12 %).

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
    """The complete result of a single experiment run.

    Attributes
    ----------
    experiment_id : str
        Unique identifier of the persisted experiment row.

    name : str
        Human-readable name (mirrors the config).

    tags : list[str]
        Tags assigned to the experiment.

    started_at : int
        UTC timestamp (seconds) when the run started.

    finished_at : int
        UTC timestamp (seconds) when the run finished.

    status : str
        ``"completed"`` if every strategy succeeded, ``"failed"`` otherwise.

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
    status: str
    strategies: list[RunResult]
    tags: list[str]
    warnings: list[str]

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

class ExponentialMovingAverage:
    """Exponential Moving Average (EMA).

    A weighted moving average that gives exponentially more weight to recent
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
    - backtide.backtest:ExperimentConfig
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

class HybridAlphaRsi:
    """Full-featured Relative Strength Index combining adaptive period, levels, and trend filter.

    The most sophisticated RSI variant, combining an adaptive look-back
    period (like [`AdaptiveRsi`]), adaptive overbought/oversold levels
    (like [`AlphaRsiPro`]), and trend confirmation via a moving-average
    filter. Designed to deliver the highest-quality RSI signals across
    different market regimes.

    Parameters
    ----------
    min_period : int, default=8
        Minimum adaptive RSI period.

    max_period : int, default=28
        Maximum adaptive RSI period.

    vol_window : int, default=20
        Window for the volatility-based level adjustment.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:AdaptiveRsi
    backtide.strategies:AlphaRsiPro
    backtide.strategies:Rsi

    """

    max_period: Any
    min_period: Any
    vol_window: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

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

class Macd:
    """Moving Average Convergence Divergence crossover strategy.

    Buys on a MACD golden cross (MACD line crosses above the signal line)
    and sells on a death cross (MACD line crosses below the signal line).
    Captures medium-term trend changes driven by the divergence between
    fast and slow exponential moving averages. Useful for trend-following
    in moderately trending markets.

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
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:Momentum
    backtide.strategies:SmaCrossover
    backtide.strategies:Rsi

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class Momentum:
    """Trend-following strategy driven by short-term price momentum.

    Buys when short-term momentum turns positive (e.g. price rises above
    a recent trough) and sells when the price falls below a trend-filtering
    moving average. A straightforward trend-following approach that aims to
    ride established moves and exit before they reverse.

    Parameters
    ----------
    period : int, default=14
        Look-back period for the momentum calculation.

    ma_period : int, default=50
        Moving average period for the trend filter.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:BuyAndHold
    backtide.strategies:Roc
    backtide.strategies:SmaCrossover

    """

    ma_period: Any
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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class MovingAverageConvergenceDivergence:
    """Moving Average Convergence Divergence (MACD).

    A trend-following momentum indicator that shows the relationship between
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

class MultiBollingerRotation:
    """Multi-asset Bollinger Bands breakout rotation strategy.

    A breakout rotation strategy that periodically ranks all assets by
    how far their price exceeds the upper Bollinger Band and rotates into
    the top K positions. Assets that have broken out above their bands
    are considered to be in strong uptrends. Useful for momentum-driven
    portfolio rotation across a basket of assets.

    Parameters
    ----------
    period : int, default=20
        Bollinger Band moving average period.

    std_dev : float, default=2.0
        Number of standard deviations for the bands.

    top_k : int, default=5
        Number of top-ranked assets to hold.

    rebalance_interval : int, default=20
        Number of bars between rebalancing.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:BollingerMeanReversion
    backtide.strategies:RocRotation
    backtide.strategies:TripleRsiRotation

    """

    period: Any
    rebalance_interval: Any
    std_dev: Any
    top_k: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class OnBalanceVolume:
    """On-Balance Volume (OBV).

    A cumulative volume indicator that adds volume on up-close days and
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

class Order:
    """A trading order submitted during the simulation.

    Attributes
    ----------
    id : str
        Unique identifier of the order. Auto-generated if not provided.
        For [`OrderType.CancelOrder`][OrderType] orders, the `id` field
        identifies the *target* order that should be cancelled.

    symbol : str
        The ticker symbol this order targets.

    order_type : [OrderType]
        The execution semantics (market, limit, stop-loss, etc.).

    quantity : int
        Signed quantity. Positive for buy orders, negative for sell orders.

    price : float | None
        Primary price for the order. The exact meaning depends on
        `order_type`:

        - ``Market`` / ``CancelOrder`` / ``SettlePosition``: ignored.
        - ``Limit`` / ``TakeProfit``: the limit / target price.
        - ``StopLoss``: the stop (trigger) price.
        - ``StopLossLimit`` / ``TakeProfitLimit``: the stop (trigger)
          price; once hit the order converts to a limit at
          ``limit_price``.
        - ``TrailingStop`` / ``TrailingStopLimit``: the trail amount in
          price units (positive). The engine maintains the running
          extreme internally.

    limit_price : float | None
        Secondary limit price used by the ``StopLossLimit``,
        ``TakeProfitLimit`` and ``TrailingStopLimit`` order types.
        Once the stop component triggers, the order converts to a
        limit order resting at this price. Ignored for all other
        order types.

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
    quantity: int
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
        The bar timestamp at which the order was processed.

    status : str
        ``"filled"``, ``"cancelled"``, ``"rejected"`` or ``"pending"``.

    fill_price : float | None
        Average fill price (None if not filled).

    reason : str
        Human-readable note (rejection / cancellation reason).

    commission : float
        Commission charged on the fill, in the order's quote currency.
        Zero for non-filled orders.

    pnl : float | None
        Realised profit & loss attributable to this order, in the base
        currency, after commission. Populated only on closing fills
        (sell that flattens / reduces an existing long, or buy-to-cover);
        `None` for opening fills, cancellations and rejections.

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
    status: str
    timestamp: int

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
    - backtide.backtest:Order

    """

    name: str

    CancelOrder: ClassVar[OrderType]
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

    positions : dict[str, int]
        Open positions keyed by ticker symbol. Positive values are long
        positions, negative values are short positions.

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
    positions: dict[str, int]

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

    starting_positions : dict[str, int], default={}
        Pre-loaded positions `{symbol: quantity}`.

    See Also
    --------
    - backtide.backtest:ExchangeExpConfig
    - backtide.backtest:ExperimentConfig
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
    """Relative Strength Index (RSI).

    A momentum oscillator that measures the speed and magnitude of recent
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

class RiskAverse:
    """Low-volatility breakout strategy for risk-conscious investors.

    Targets low-volatility stocks making new highs on above-average volume.
    Combines a volatility filter (e.g., ATR below a threshold) with a
    breakout condition and volume confirmation to find "quiet" stocks that
    are about to move. Designed for risk-conscious investors who want
    trend exposure with lower drawdowns.

    Parameters
    ----------
    vol_period : int, default=14
        ATR look-back period for the volatility filter.

    breakout_period : int, default=20
        Number of bars for the new-high breakout condition.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:BuyAndHold
    backtide.strategies:TurtleTrading
    backtide.strategies:Vcp

    """

    breakout_period: Any
    vol_period: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class Roc:
    """Rate of Change momentum strategy.

    A simple momentum strategy based on Rate of Change. Buys when the ROC
    over a specified period exceeds an upper threshold (strong upward
    momentum) and sells when ROC falls below a lower threshold. Useful as
    a straightforward momentum filter.

    Parameters
    ----------
    period : int, default=12
        ROC look-back period.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:Momentum
    backtide.strategies:RocRotation
    backtide.strategies:Rsi

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class RocRotation:
    """Multi-asset portfolio rotation ranked by Rate of Change.

    Periodically ranks all assets by their Rate of Change (momentum) over
    a given window and rotates the portfolio into the top K performers.
    A classic relative-momentum rotation approach used to capture the
    strongest trends across a basket of instruments.

    Parameters
    ----------
    period : int, default=12
        ROC look-back period for ranking.

    top_k : int, default=5
        Number of top-ranked assets to hold.

    rebalance_interval : int, default=20
        Number of bars between rebalancing.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:Roc
    backtide.strategies:RsrsRotation
    backtide.strategies:TripleRsiRotation

    """

    period: Any
    rebalance_interval: Any
    top_k: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class Rsi:
    """Relative Strength Index combined with Bollinger Bands for dual confirmation.

    Combines RSI and Bollinger Bands. Enters long when RSI is in oversold
    territory **and** price is at or below the lower Bollinger Band, giving
    a dual confirmation of mean-reversion conditions. Exits when RSI
    returns to neutral or price reaches the upper band. Useful for
    catching bounces with higher conviction than RSI or Bollinger Bands
    alone.

    Parameters
    ----------
    rsi_period : int, default=14
        RSI look-back period.

    bb_period : int, default=20
        Bollinger Band moving average period.

    bb_std : float, default=2.0
        Number of standard deviations for the bands.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:AdaptiveRsi
    backtide.strategies:AlphaRsiPro
    backtide.strategies:BollingerMeanReversion

    """

    bb_period: Any
    bb_std: Any
    rsi_period: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class Rsrs:
    """Resistance Support Relative Strength trend-detection strategy.

    Uses linear regression of high vs. low prices (Resistance Support
    Relative Strength) to detect when support is strengthening. Buys when
    the RSRS indicator signals that the support floor is rising faster
    than resistance, indicating a potential upward breakout. Useful for
    quantitative trend detection based on price structure.

    Parameters
    ----------
    period : int, default=18
        Look-back window for the linear regression.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:Momentum
    backtide.strategies:RsrsRotation
    backtide.strategies:TurtleTrading

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class RsrsRotation:
    """Multi-asset portfolio rotation ranked by Resistance Support Relative Strength.

    Periodically ranks all assets by their RSRS indicator value and
    rotates into those with the strongest support signals. Assets whose
    support floor is rising fastest relative to resistance are considered
    to have the best risk/reward profile. Useful for support-based
    portfolio rotation across a universe of stocks.

    Parameters
    ----------
    period : int, default=18
        RSRS look-back window for ranking.

    top_k : int, default=5
        Number of top-ranked assets to hold.

    rebalance_interval : int, default=20
        Number of bars between rebalancing.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:RocRotation
    backtide.strategies:Rsrs
    backtide.strategies:TripleRsiRotation

    """

    period: Any
    rebalance_interval: Any
    top_k: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class RunResult:
    """Result of running a single strategy as part of an experiment.

    Attributes
    ----------
    strategy_id : str
        Unique identifier for this strategy run.

    strategy_name : str
        The user-facing name of the strategy.

    equity_curve : list[[EquitySample]]
        Per-bar equity samples in chronological order.

    trades : list[[Trade]]
        All round-trip trades closed during the run.

    orders : list[[OrderRecord]]
        All orders the engine processed (filled, canceled, rejected).

    metrics : dict[str, float]
        Summary metrics (total_return, sharpe, max_drawdown, ...).

    base_currency : [Currency]
        The portfolio's base (accounting) currency for this run. Equity,
        PnL and drawdown values stored on the run are denominated in this
        currency. Captured from the `ExperimentConfig` so analysis tools
        don't need to look the experiment config up to label axes.

    error : str | None
        ``None`` on success. Otherwise the first error raised by the
        strategy during the run (e.g. an exception thrown by
        ``evaluate(...)``). Strategies that fail still produce a result
        row so the rest of the experiment isn't lost — the engine simply
        records the error and reports the experiment status as
        ``"failed"``.

    See Also
    --------
    - backtide.backtest:EquitySample
    - backtide.backtest:ExperimentResult
    - backtide.storage:query_strategy_runs

    """

    base_currency: Currency
    equity_curve: list[EquitySample]
    error: str | None
    metrics: dict[str, float]
    orders: list[OrderRecord]
    strategy_id: str
    strategy_name: str
    trades: list[Trade]

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

class SimpleMovingAverage:
    """Simple Moving Average (SMA).

    The arithmetic mean of the last n closing prices. Used to smooth
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

class SmaCrossover:
    """Simple Moving Average crossover strategy using fast and slow periods.

    Generates buy and sell signals based on moving-average crossovers.
    A **golden cross** (fast MA crosses above slow MA) triggers a buy;
    a **death cross** (fast MA crosses below slow MA) triggers a sell.
    More robust than the naive SMA strategy because it requires
    confirmation from two different time horizons.

    Parameters
    ----------
    fast_period : int, default=20
        Fast moving average period.

    slow_period : int, default=50
        Slow moving average period.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:Macd
    backtide.strategies:Momentum
    backtide.strategies:SmaNaive

    """

    fast_period: Any
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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class SmaNaive:
    """Naive single Simple Moving Average trend-following strategy.

    The simplest trend-following strategy: buys when the price is above a
    single moving average and sells when below. No second average or
    additional filter is used, so it reacts quickly but can generate many
    whipsaws in sideways markets. Useful as a baseline trend-following
    strategy.

    Parameters
    ----------
    period : int, default=20
        Moving average period.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:BuyAndHold
    backtide.strategies:Momentum
    backtide.strategies:SmaCrossover

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

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

class StochasticOscillator:
    """Stochastic Oscillator (STOCH).

    Compares the closing price to the high-low range over a period,
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

    Strategies are stored by name. Each name refers to a pickled strategy
    object saved in the local strategies directory.

    Attributes
    ----------
    benchmark : str
        Benchmark ticker symbol used with a passive Buy & Hold experiment as
        a side-car alongside the selected strategies and used to compute alpha.

    strategies : list[str], default=[]
        Names of the strategies to use in this experiment. Each name must
        match a stored strategy.

    See Also
    --------
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:IndicatorExpConfig
    - backtide.backtest:PortfolioExpConfig

    """

    benchmark: str
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

    quantity : int
        Signed quantity. Positive = long round trip, negative = short.

    entry_ts : int
        Open timestamp (seconds since the Unix epoch).

    exit_ts : int
        Close timestamp (seconds since the Unix epoch).

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
    quantity: int
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

class TripleRsiRotation:
    """Multi-timeframe Relative Strength Index portfolio rotation strategy.

    Ranks assets by a composite score derived from long-term, medium-term,
    and short-term RSI values and periodically rotates the portfolio into
    the highest-scoring positions. The triple-time-frame approach helps
    distinguish strong multi-horizon momentum from single-period flukes.
    Useful for momentum rotation with multi-horizon confirmation.

    Parameters
    ----------
    short_period : int, default=5
        Short-term RSI period.

    medium_period : int, default=14
        Medium-term RSI period.

    long_period : int, default=28
        Long-term RSI period.

    top_k : int, default=5
        Number of top-ranked assets to hold.

    rebalance_interval : int, default=20
        Number of bars between rebalancing.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:MultiBollingerRotation
    backtide.strategies:RocRotation
    backtide.strategies:RsrsRotation

    """

    long_period: Any
    medium_period: Any
    rebalance_interval: Any
    short_period: Any
    top_k: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class TurtleTrading:
    """Classic channel-breakout trend-following system with ATR-based position sizing.

    A classic trend-following system inspired by the Turtle Traders. Buys
    on a breakout above the highest high of the last N bars and sells on
    a breakdown below the lowest low of the last M bars. Uses ATR-based
    position sizing to normalise risk across instruments. Useful for
    systematic trend-following with built-in risk management.

    Parameters
    ----------
    entry_period : int, default=20
        Number of bars for the entry breakout (highest high).

    exit_period : int, default=10
        Number of bars for the exit breakdown (lowest low).

    atr_period : int, default=20
        ATR period for position sizing.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:BuyAndHold
    backtide.strategies:Momentum
    backtide.strategies:RiskAverse

    """

    atr_period: Any
    entry_period: Any
    exit_period: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class Vcp:
    """Volatility Contraction Pattern breakout strategy.

    Detects a Volatility Contraction Pattern: a series of progressively
    tighter price consolidations with declining volume. When both price
    range and volume have contracted sufficiently, the strategy enters long
    on a breakout above the consolidation ceiling. Useful for swing trading
    setups where decreasing supply precedes a sharp move.

    Parameters
    ----------
    lookback : int, default=60
        Number of bars to detect the contraction pattern.

    contractions : int, default=3
        Minimum number of contracting ranges required.

    Attributes
    ----------
    name : str
        Human-readable strategy name.

    is_multi_asset : bool
        Whether this is a multi-asset strategy.

    See Also
    --------
    backtide.strategies:DoubleTop
    backtide.strategies:RiskAverse
    backtide.strategies:TurtleTrading

    """

    contractions: Any
    lookback: Any

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
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def required_indicators(self) -> list:
        """Indicators that must be computed up-front for this
        strategy.

        Returns a list of indicator instances, already
        parameterised with this strategy's current settings,
        that the engine will auto-include before the simulation
        starts.

        Returns
        -------
        list
            The required indicator instances.

        """

class VolumeWeightedAveragePrice:
    """Volume-Weighted Average Price (VWAP).

    The cumulative average price weighted by volume. Institutional traders
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
    """Weighted Moving Average (WMA).

    A moving average where each price is multiplied by a linearly decreasing
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

def run_experiment(config, *, verbose=True) -> ExperimentResult:
    """Run a backtest experiment with the provided configuration.

    Performs the full pipeline end-to-end:

    1. Resolves and downloads any missing market data (skipped if already
       present in the local DuckDB cache).
    2. Computes every selected indicator once over the entire dataset, in
       parallel across symbols. Custom (Python) indicators are dispatched
       via PyO3.
    3. Runs every selected strategy fully in parallel — each strategy has
       its own independent portfolio, order book and equity curve.
    4. Persists the aggregated [`ExperimentResult`] (and per-strategy
       artifacts) into the experiment tables in DuckDB.

    Parameters
    ----------
    config : [ExperimentConfig]
        The complete experiment configuration.

    verbose : bool, default=True
        Whether to display a progress bar while running.

    Returns
    -------
    [ExperimentResult]
        The aggregated result of the run.

    See Also
    --------
    - backtide.backtest:ExperimentConfig
    - backtide.backtest:ExperimentResult
    - backtide.storage:query_experiments

    Examples
    --------
    ```pycon
    from backtide.backtest import ExperimentConfig, run_experiment

    cfg = ExperimentConfig()
    result = run_experiment(cfg)
    print(result)
    ```

    """
