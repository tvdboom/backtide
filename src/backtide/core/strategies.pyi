"""Type stubs for `backtide.core.strategies` (auto-generated)."""

__all__ = [
    "AdaptiveRsi",
    "AlphaRsiPro",
    "BollingerMeanReversion",
    "BuyAndHold",
    "DoubleTop",
    "HybridAlphaRsi",
    "Macd",
    "Momentum",
    "MultiBollingerRotation",
    "RiskAverse",
    "Roc",
    "RocRotation",
    "Rsi",
    "Rsrs",
    "RsrsRotation",
    "SmaCrossover",
    "SmaNaive",
    "TripleRsiRotation",
    "TurtleTrading",
    "Vcp",
]

from typing import Any, ClassVar

import numpy as np
import pandas as pd
import polars as pl

from backtide.core.backtest import Order

from backtide.indicators import BaseIndicator

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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
            The required indicator instances.

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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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

    symbol: Any
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
            The required indicator instances.

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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
            The required indicator instances.

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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
            The required indicator instances.

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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
            The required indicator instances.

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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
            The required indicator instances.

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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
            The required indicator instances.

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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
            The required indicator instances.

        """

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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
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
    is_multi_asset: ClassVar[bool]
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
    @classmethod
    def description(cls) -> str:
        """Short explanation of what the strategy does.

        Returns
        -------
        str
            The description.

        """
    def evaluate(self, data, portfolio, state, indicators=None) -> list[Order]:
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            Keys are the experiment's symbols and values are the historical
            OHLCV data available up to the current bar.

        portfolio : [Portfolio]
            Current portfolio holdings (cash, positions and open orders).

        state : [State]
            Current simulation state.

        indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            The first keys are the indicator names. The second keys are the
            experiment's symbols. The values are the pre-computed indicator
            values. `None` if no indicators were selected.

        Returns
        -------
        list[[Order]]
            Orders to place this tick.

        """
    def required_indicators(self) -> list[BaseIndicator]:
        """Indicators that must be computed up-front for this strategy.

        Returns a list of indicator instances, already parameterized
        with this strategy's current settings, that the engine will
        auto-include before the backtest starts.

        Returns
        -------
        list[[BaseIndicator]]
            The required indicator instances.

        """
