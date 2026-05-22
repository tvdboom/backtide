"""Type stubs for `backtide.core.indicators` (auto-generated)."""

__all__ = [
    "AverageDirectionalIndex",
    "AverageTrueRange",
    "BollingerBands",
    "CommodityChannelIndex",
    "ExponentialMovingAverage",
    "MovingAverageConvergenceDivergence",
    "OnBalanceVolume",
    "RelativeStrengthIndex",
    "SimpleMovingAverage",
    "StochasticOscillator",
    "VolumeWeightedAveragePrice",
    "WeightedMovingAverage",
    "_indicator_deterministic_name",
]

from typing import Any, ClassVar

import pandas as pd
import polars as pl

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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

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

    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

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

    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
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
    acronym: ClassVar[str]
    name: ClassVar[str]

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
    def compute(self, data) -> pd.Series | pl.Series | pd.DataFrame | pl.DataFrame:
        """Compute the indicator on a dataset.

        Parameters
        ----------
        data : pd.DataFrame | pl.DataFrame
            Historical OHLCV data.

        Returns
        -------
        pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the indicator measures.

        Returns
        -------
        str
            The description.

        """

def _indicator_deterministic_name(indicator):
    """Get the deterministic name for an indicator instance."""
