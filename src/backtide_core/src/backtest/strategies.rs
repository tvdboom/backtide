use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::backtest::models::order::Order;
use crate::backtest::models::state::State;

/// Trait for all built-in strategies.
pub trait Strategy {
    /// Human-readable name (e.g. `"Buy & Hold"`).
    const NAME: &'static str;

    /// One-sentence explanation of what the strategy does.
    const DESCRIPTION: &'static str;

    /// Whether this is a portfolio-rotation (multi-asset) strategy.
    const IS_MULTI_ASSET: bool;
}

/// Shared pymethods macro for all strategy structs.
///
/// The struct must already have a `#[pymethods]` block with `new` and `__reduce__`.
/// This macro adds `name`, `description`, `is_multi_asset`, `evaluate`, `__repr__`.
macro_rules! strategy_pymethods {
    ($ty:ident) => {
        #[pymethods]
        impl $ty {
            /// Human-readable name.
            #[classattr]
            fn name() -> &'static str {
                <$ty as Strategy>::NAME
            }

            /// Short explanation of what the strategy does.
            ///
            /// Returns
            /// -------
            /// str
            ///     The description.
            #[classmethod]
            fn description(_cls: &Bound<'_, PyType>) -> &'static str {
                <$ty as Strategy>::DESCRIPTION
            }

            /// Whether this is a portfolio-rotation (multi-asset) strategy.
            #[classattr]
            fn is_multi_asset() -> bool {
                <$ty as Strategy>::IS_MULTI_ASSET
            }

            /// Evaluate the strategy and return orders.
            ///
            /// Parameters
            /// ----------
            /// data : np.array | pd.DataFrame | pl.DataFrame
            ///     Historical OHLCV data available up to the current bar.
            ///
            /// state : [State]
            ///     Current simulation state.
            ///
            /// indicators : np.array | pd.DataFrame | pl.DataFrame | None
            ///     Indicators calculated on the historical data. None if no
            ///     indicators were selected.
            ///
            /// Returns
            /// -------
            /// list[[Order]]
            ///     The orders to place this tick.
            fn evaluate<'py>(
                &self,
                _py: Python<'py>,
                _data: &Bound<'py, PyAny>,
                _state: &State,
                _indicators: &Bound<'py, PyAny>,
            ) -> PyResult<Vec<Order>> {
                // TODO: Implement strategy evaluation logic
                Ok(vec![])
            }

            /// Return a debug representation.
            fn __repr__(&self) -> String {
                format!("{}()", <$ty as Strategy>::NAME)
            }
        }
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Strategy structs (alphabetical order)
// ─────────────────────────────────────────────────────────────────────────────

/// Relative Strength Index with a dynamically adaptive look-back period.
///
/// Dynamically adjusts its look-back period based on current market volatility
/// and cycle length. In calm, trending markets the period lengthens for smoother
/// signals; in volatile or choppy regimes it shortens for faster reaction. Useful
/// when a fixed-period RSI produces too many whipsaws or lags behind regime
/// changes.
///
/// Parameters
/// ----------
/// min_period : int, default=8
///     Minimum adaptive RSI period.
///
/// max_period : int, default=28
///     Maximum adaptive RSI period.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:AlphaRsiPro
/// backtide.strategies:HybridAlphaRsi
/// backtide.strategies:Rsi
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct AdaptiveRsi {
    /// Minimum adaptive RSI period.
    min_period: usize,

    /// Maximum adaptive RSI period.
    max_period: usize,
}

#[pymethods]
impl AdaptiveRsi {
    #[new]
    #[pyo3(signature = (min_period=8, max_period=28))]
    fn new(min_period: usize, max_period: usize) -> Self {
        Self {
            min_period,
            max_period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.min_period, self.max_period)))
    }
}

impl Strategy for AdaptiveRsi {
    const NAME: &'static str = "Adaptive RSI";
    const DESCRIPTION: &'static str =
        "RSI with dynamic period that adapts to market volatility and cycles.";
    const IS_MULTI_ASSET: bool = false;
}

/// Advanced Relative Strength Index with adaptive overbought/oversold levels.
///
/// An advanced RSI variant that computes adaptive overbought and oversold
/// thresholds based on recent volatility, and adds a trend-bias filter to
/// avoid counter-trend entries. In strong uptrends the oversold level is
/// raised so buy signals fire earlier; in downtrends the overbought level
/// is lowered so sells trigger sooner. Useful for reducing false signals
/// in trending markets compared to a plain RSI strategy.
///
/// Parameters
/// ----------
/// period : int, default=14
///     RSI look-back period.
///
/// vol_window : int, default=20
///     Window for the volatility-based level adjustment.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:AdaptiveRsi
/// backtide.strategies:HybridAlphaRsi
/// backtide.strategies:Rsi
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct AlphaRsiPro {
    /// RSI look-back period.
    period: usize,

    /// Window for the volatility-based level adjustment.
    vol_window: usize,
}

#[pymethods]
impl AlphaRsiPro {
    #[new]
    #[pyo3(signature = (period=14, vol_window=20))]
    fn new(period: usize, vol_window: usize) -> Self {
        Self {
            period,
            vol_window,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period, self.vol_window)))
    }
}

impl Strategy for AlphaRsiPro {
    const NAME: &'static str = "AlphaRSI Pro";
    const DESCRIPTION: &'static str = "Advanced RSI with adaptive overbought/oversold levels based on volatility and trend bias filtering.";
    const IS_MULTI_ASSET: bool = false;
}

/// Mean-reversion strategy using Bollinger Band boundaries.
///
/// A mean-reversion strategy that enters long when the price touches or
/// crosses below the lower Bollinger Band and exits when it reaches the
/// upper band. The assumption is that price will revert to its moving
/// average after an extreme excursion. Useful in range-bound or
/// mean-reverting markets.
///
/// Parameters
/// ----------
/// period : int, default=20
///     Number of bars for the Bollinger Band moving average.
///
/// std_dev : float, default=2.0
///     Number of standard deviations for the band width.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:MultiBollingerRotation
/// backtide.strategies:Rsi
/// backtide.strategies:SmaCrossover
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
#[allow(clippy::upper_case_acronyms)]
pub struct BollingerMeanReversion {
    /// Number of bars for the moving average.
    period: usize,

    /// Number of standard deviations for the band width.
    std_dev: f64,
}

#[pymethods]
impl BollingerMeanReversion {
    #[new]
    #[pyo3(signature = (period=20, std_dev=2.0))]
    fn new(period: usize, std_dev: f64) -> Self {
        Self {
            period,
            std_dev,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, f64))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period, self.std_dev)))
    }
}

impl Strategy for BollingerMeanReversion {
    const NAME: &'static str = "BB Mean Reversion";
    const DESCRIPTION: &'static str =
        "A mean-reversion strategy that buys at the lower band and sells at the upper band.";
    const IS_MULTI_ASSET: bool = false;
}

/// Passive baseline that buys once and holds indefinitely.
///
/// The simplest possible strategy: buy on the very first bar and hold the
/// position until the end of the simulation. Serves as the baseline
/// benchmark against which all other strategies are compared. Equivalent
/// to a passive index investment over the backtest window.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:Momentum
/// backtide.strategies:SmaNaive
/// backtide.strategies:TurtleTrading
#[pyclass(skip_from_py_object, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct BuyAndHold;

#[pymethods]
impl BuyAndHold {
    #[new]
    fn new() -> Self {
        Self
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, ())> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, ()))
    }
}

impl Strategy for BuyAndHold {
    const NAME: &'static str = "Buy & Hold";
    const DESCRIPTION: &'static str =
        "Buys on the first day and holds to the end. A baseline for performance comparison.";
    const IS_MULTI_ASSET: bool = false;
}

/// Chart-pattern breakout triggered by a double-top formation.
///
/// Detects a double-top chart pattern — two consecutive peaks at roughly
/// the same price level — and enters long on the subsequent breakout above
/// the neckline. Includes a trend filter and volume confirmation to reduce
/// false breakouts. Useful for pattern-recognition-based breakout trading.
///
/// Parameters
/// ----------
/// lookback : int, default=60
///     Number of bars to search for the double-top pattern.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:Momentum
/// backtide.strategies:TurtleTrading
/// backtide.strategies:Vcp
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct DoubleTop {
    /// Number of bars to search for the double-top pattern.
    lookback: usize,
}

#[pymethods]
impl DoubleTop {
    #[new]
    #[pyo3(signature = (lookback=60))]
    fn new(lookback: usize) -> Self {
        Self {
            lookback,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.lookback,)))
    }
}

impl Strategy for DoubleTop {
    const NAME: &'static str = "Double Top";
    const DESCRIPTION: &'static str =
        "Buys on a breakout after a double top pattern, with trend and volume confirmation.";
    const IS_MULTI_ASSET: bool = false;
}

/// Full-featured Relative Strength Index combining adaptive period, levels, and trend filter.
///
/// The most sophisticated RSI variant, combining an adaptive look-back
/// period (like [`AdaptiveRsi`]), adaptive overbought/oversold levels
/// (like [`AlphaRsiPro`]), and trend confirmation via a moving-average
/// filter. Designed to deliver the highest-quality RSI signals across
/// different market regimes.
///
/// Parameters
/// ----------
/// min_period : int, default=8
///     Minimum adaptive RSI period.
///
/// max_period : int, default=28
///     Maximum adaptive RSI period.
///
/// vol_window : int, default=20
///     Window for the volatility-based level adjustment.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:AdaptiveRsi
/// backtide.strategies:AlphaRsiPro
/// backtide.strategies:Rsi
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct HybridAlphaRsi {
    /// Minimum adaptive RSI period.
    min_period: usize,

    /// Maximum adaptive RSI period.
    max_period: usize,

    /// Window for the volatility-based level adjustment.
    vol_window: usize,
}

#[pymethods]
impl HybridAlphaRsi {
    #[new]
    #[pyo3(signature = (min_period=8, max_period=28, vol_window=20))]
    fn new(min_period: usize, max_period: usize, vol_window: usize) -> Self {
        Self {
            min_period,
            max_period,
            vol_window,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (usize, usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.min_period, self.max_period, self.vol_window)))
    }
}

impl Strategy for HybridAlphaRsi {
    const NAME: &'static str = "Hybrid AlphaRSI";
    const DESCRIPTION: &'static str = "Most sophisticated RSI variant combining adaptive period, adaptive levels, and trend confirmation.";
    const IS_MULTI_ASSET: bool = false;
}

/// Moving Average Convergence Divergence crossover strategy.
///
/// Buys on a MACD golden cross (MACD line crosses above the signal line)
/// and sells on a death cross (MACD line crosses below the signal line).
/// Captures medium-term trend changes driven by the divergence between
/// fast and slow exponential moving averages. Useful for trend-following
/// in moderately trending markets.
///
/// Parameters
/// ----------
/// fast_period : int, default=12
///     Fast EMA period.
///
/// slow_period : int, default=26
///     Slow EMA period.
///
/// signal_period : int, default=9
///     Signal line EMA period.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:Momentum
/// backtide.strategies:SmaCrossover
/// backtide.strategies:Rsi
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct Macd {
    /// Fast EMA period.
    fast_period: usize,

    /// Slow EMA period.
    slow_period: usize,

    /// Signal line EMA period.
    signal_period: usize,
}

#[pymethods]
impl Macd {
    #[new]
    #[pyo3(signature = (fast_period=12, slow_period=26, signal_period=9))]
    fn new(fast_period: usize, slow_period: usize, signal_period: usize) -> Self {
        Self {
            fast_period,
            slow_period,
            signal_period,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (usize, usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.fast_period, self.slow_period, self.signal_period)))
    }
}

impl Strategy for Macd {
    const NAME: &'static str = "MACD";
    const DESCRIPTION: &'static str = "Buys on a MACD golden cross and sells on a death cross.";
    const IS_MULTI_ASSET: bool = false;
}

/// Trend-following strategy driven by short-term price momentum.
///
/// Buys when short-term momentum turns positive (e.g. price rises above
/// a recent trough) and sells when the price falls below a trend-filtering
/// moving average. A straightforward trend-following approach that aims to
/// ride established moves and exit before they reverse.
///
/// Parameters
/// ----------
/// period : int, default=14
///     Look-back period for the momentum calculation.
///
/// ma_period : int, default=50
///     Moving average period for the trend filter.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:BuyAndHold
/// backtide.strategies:Roc
/// backtide.strategies:SmaCrossover
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct Momentum {
    /// Look-back period for the momentum calculation.
    period: usize,

    /// Moving average period for the trend filter.
    ma_period: usize,
}

#[pymethods]
impl Momentum {
    #[new]
    #[pyo3(signature = (period=14, ma_period=50))]
    fn new(period: usize, ma_period: usize) -> Self {
        Self {
            period,
            ma_period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period, self.ma_period)))
    }
}

impl Strategy for Momentum {
    const NAME: &'static str = "Momentum";
    const DESCRIPTION: &'static str =
        "Buys when momentum turns positive, sells when price falls below a trend-filtering MA.";
    const IS_MULTI_ASSET: bool = false;
}

/// Multi-asset Bollinger Bands breakout rotation strategy.
///
/// A breakout rotation strategy that periodically ranks all assets by
/// how far their price exceeds the upper Bollinger Band and rotates into
/// the top K positions. Assets that have broken out above their bands
/// are considered to be in strong uptrends. Useful for momentum-driven
/// portfolio rotation across a basket of assets.
///
/// Parameters
/// ----------
/// period : int, default=20
///     Bollinger Band moving average period.
///
/// std_dev : float, default=2.0
///     Number of standard deviations for the bands.
///
/// top_k : int, default=5
///     Number of top-ranked assets to hold.
///
/// rebalance_interval : int, default=20
///     Number of bars between rebalancing.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:BollingerMeanReversion
/// backtide.strategies:RocRotation
/// backtide.strategies:TripleRsiRotation
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct MultiBollingerRotation {
    /// Bollinger Band moving average period.
    period: usize,

    /// Number of standard deviations for the bands.
    std_dev: f64,

    /// Number of top-ranked assets to hold.
    top_k: usize,

    /// Number of bars between rebalancing.
    rebalance_interval: usize,
}

#[pymethods]
impl MultiBollingerRotation {
    #[new]
    #[pyo3(signature = (period=20, std_dev=2.0, top_k=5, rebalance_interval=20))]
    fn new(period: usize, std_dev: f64, top_k: usize, rebalance_interval: usize) -> Self {
        Self {
            period,
            std_dev,
            top_k,
            rebalance_interval,
        }
    }

    #[allow(clippy::type_complexity)]
    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (usize, f64, usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period, self.std_dev, self.top_k, self.rebalance_interval)))
    }
}

impl Strategy for MultiBollingerRotation {
    const NAME: &'static str = "Multi BB Rotation";
    const DESCRIPTION: &'static str =
        "A breakout rotation strategy that buys stocks crossing above their upper Bollinger Band.";
    const IS_MULTI_ASSET: bool = true;
}

/// Low-volatility breakout strategy for risk-conscious investors.
///
/// Targets low-volatility stocks making new highs on above-average volume.
/// Combines a volatility filter (e.g., ATR below a threshold) with a
/// breakout condition and volume confirmation to find "quiet" stocks that
/// are about to move. Designed for risk-conscious investors who want
/// trend exposure with lower drawdowns.
///
/// Parameters
/// ----------
/// vol_period : int, default=14
///     ATR look-back period for the volatility filter.
///
/// breakout_period : int, default=20
///     Number of bars for the new-high breakout condition.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:BuyAndHold
/// backtide.strategies:TurtleTrading
/// backtide.strategies:Vcp
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct RiskAverse {
    /// ATR look-back period for the volatility filter.
    vol_period: usize,

    /// Number of bars for the new-high breakout condition.
    breakout_period: usize,
}

#[pymethods]
impl RiskAverse {
    #[new]
    #[pyo3(signature = (vol_period=14, breakout_period=20))]
    fn new(vol_period: usize, breakout_period: usize) -> Self {
        Self {
            vol_period,
            breakout_period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.vol_period, self.breakout_period)))
    }
}

impl Strategy for RiskAverse {
    const NAME: &'static str = "Risk Averse";
    const DESCRIPTION: &'static str = "Buys low-volatility stocks making new highs on high volume.";
    const IS_MULTI_ASSET: bool = false;
}

/// Rate of Change momentum strategy.
///
/// A simple momentum strategy based on Rate of Change. Buys when the ROC
/// over a specified period exceeds an upper threshold (strong upward
/// momentum) and sells when ROC falls below a lower threshold. Useful as
/// a straightforward momentum filter.
///
/// Parameters
/// ----------
/// period : int, default=12
///     ROC look-back period.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:Momentum
/// backtide.strategies:RocRotation
/// backtide.strategies:Rsi
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct Roc {
    /// ROC look-back period.
    period: usize,
}

#[pymethods]
impl Roc {
    #[new]
    #[pyo3(signature = (period=12))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Strategy for Roc {
    const NAME: &'static str = "ROC";
    const DESCRIPTION: &'static str =
        "A simple momentum strategy that buys on a high Rate of Change and sells on a low one.";
    const IS_MULTI_ASSET: bool = false;
}

/// Multi-asset portfolio rotation ranked by Rate of Change.
///
/// Periodically ranks all assets by their Rate of Change (momentum) over
/// a given window and rotates the portfolio into the top K performers.
/// A classic relative-momentum rotation approach used to capture the
/// strongest trends across a basket of instruments.
///
/// Parameters
/// ----------
/// period : int, default=12
///     ROC look-back period for ranking.
///
/// top_k : int, default=5
///     Number of top-ranked assets to hold.
///
/// rebalance_interval : int, default=20
///     Number of bars between rebalancing.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:Roc
/// backtide.strategies:RsrsRotation
/// backtide.strategies:TripleRsiRotation
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct RocRotation {
    /// ROC look-back period for ranking.
    period: usize,

    /// Number of top-ranked assets to hold.
    top_k: usize,

    /// Number of bars between rebalancing.
    rebalance_interval: usize,
}

#[pymethods]
impl RocRotation {
    #[new]
    #[pyo3(signature = (period=12, top_k=5, rebalance_interval=20))]
    fn new(period: usize, top_k: usize, rebalance_interval: usize) -> Self {
        Self {
            period,
            top_k,
            rebalance_interval,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (usize, usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period, self.top_k, self.rebalance_interval)))
    }
}

impl Strategy for RocRotation {
    const NAME: &'static str = "ROC Rotation";
    const DESCRIPTION: &'static str =
        "Periodically rotates into the top K stocks with the highest Rate of Change (momentum).";
    const IS_MULTI_ASSET: bool = true;
}

/// Relative Strength Index combined with Bollinger Bands for dual confirmation.
///
/// Combines RSI and Bollinger Bands. Enters long when RSI is in oversold
/// territory **and** price is at or below the lower Bollinger Band, giving
/// a dual confirmation of mean-reversion conditions. Exits when RSI
/// returns to neutral or price reaches the upper band. Useful for
/// catching bounces with higher conviction than RSI or Bollinger Bands
/// alone.
///
/// Parameters
/// ----------
/// rsi_period : int, default=14
///     RSI look-back period.
///
/// bb_period : int, default=20
///     Bollinger Band moving average period.
///
/// bb_std : float, default=2.0
///     Number of standard deviations for the bands.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:AdaptiveRsi
/// backtide.strategies:AlphaRsiPro
/// backtide.strategies:BollingerMeanReversion
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct Rsi {
    /// RSI look-back period.
    rsi_period: usize,

    /// Bollinger Band moving average period.
    bb_period: usize,

    /// Number of standard deviations for the bands.
    bb_std: f64,
}

#[pymethods]
impl Rsi {
    #[new]
    #[pyo3(signature = (rsi_period=14, bb_period=20, bb_std=2.0))]
    fn new(rsi_period: usize, bb_period: usize, bb_std: f64) -> Self {
        Self {
            rsi_period,
            bb_period,
            bb_std,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (usize, usize, f64))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.rsi_period, self.bb_period, self.bb_std)))
    }
}

impl Strategy for Rsi {
    const NAME: &'static str = "RSI";
    const DESCRIPTION: &'static str = "Combines RSI and Bollinger Bands. Buys when RSI is oversold and price is below the lower band.";
    const IS_MULTI_ASSET: bool = false;
}

/// Resistance Support Relative Strength trend-detection strategy.
///
/// Uses linear regression of high vs. low prices (Resistance Support
/// Relative Strength) to detect when support is strengthening. Buys when
/// the RSRS indicator signals that the support floor is rising faster
/// than resistance, indicating a potential upward breakout. Useful for
/// quantitative trend detection based on price structure.
///
/// Parameters
/// ----------
/// period : int, default=18
///     Look-back window for the linear regression.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:Momentum
/// backtide.strategies:RsrsRotation
/// backtide.strategies:TurtleTrading
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct Rsrs {
    /// Look-back window for the linear regression.
    period: usize,
}

#[pymethods]
impl Rsrs {
    #[new]
    #[pyo3(signature = (period=18))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Strategy for Rsrs {
    const NAME: &'static str = "RSRS";
    const DESCRIPTION: &'static str =
        "Uses linear regression of high/low prices to buy on signals of strengthening support.";
    const IS_MULTI_ASSET: bool = false;
}

/// Multi-asset portfolio rotation ranked by Resistance Support Relative Strength.
///
/// Periodically ranks all assets by their RSRS indicator value and
/// rotates into those with the strongest support signals. Assets whose
/// support floor is rising fastest relative to resistance are considered
/// to have the best risk/reward profile. Useful for support-based
/// portfolio rotation across a universe of stocks.
///
/// Parameters
/// ----------
/// period : int, default=18
///     RSRS look-back window for ranking.
///
/// top_k : int, default=5
///     Number of top-ranked assets to hold.
///
/// rebalance_interval : int, default=20
///     Number of bars between rebalancing.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:RocRotation
/// backtide.strategies:Rsrs
/// backtide.strategies:TripleRsiRotation
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct RsrsRotation {
    /// RSRS look-back window for ranking.
    period: usize,

    /// Number of top-ranked assets to hold.
    top_k: usize,

    /// Number of bars between rebalancing.
    rebalance_interval: usize,
}

#[pymethods]
impl RsrsRotation {
    #[new]
    #[pyo3(signature = (period=18, top_k=5, rebalance_interval=20))]
    fn new(period: usize, top_k: usize, rebalance_interval: usize) -> Self {
        Self {
            period,
            top_k,
            rebalance_interval,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (usize, usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period, self.top_k, self.rebalance_interval)))
    }
}

impl Strategy for RsrsRotation {
    const NAME: &'static str = "RSRS Rotation";
    const DESCRIPTION: &'static str =
        "Periodically rotates into stocks with high RSRS indicator values (strong support).";
    const IS_MULTI_ASSET: bool = true;
}

/// Simple Moving Average crossover strategy using fast and slow periods.
///
/// Generates buy and sell signals based on moving-average crossovers.
/// A **golden cross** (fast MA crosses above slow MA) triggers a buy;
/// a **death cross** (fast MA crosses below slow MA) triggers a sell.
/// More robust than the naive SMA strategy because it requires
/// confirmation from two different time horizons.
///
/// Parameters
/// ----------
/// fast_period : int, default=20
///     Fast moving average period.
///
/// slow_period : int, default=50
///     Slow moving average period.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:Macd
/// backtide.strategies:Momentum
/// backtide.strategies:SmaNaive
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct SmaCrossover {
    /// Fast moving average period.
    fast_period: usize,

    /// Slow moving average period.
    slow_period: usize,
}

#[pymethods]
impl SmaCrossover {
    #[new]
    #[pyo3(signature = (fast_period=20, slow_period=50))]
    fn new(fast_period: usize, slow_period: usize) -> Self {
        Self {
            fast_period,
            slow_period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.fast_period, self.slow_period)))
    }
}

impl Strategy for SmaCrossover {
    const NAME: &'static str = "SMA (Crossover)";
    const DESCRIPTION: &'static str =
        "Buys on a golden cross (fast MA over slow MA), sells on a death cross.";
    const IS_MULTI_ASSET: bool = false;
}

/// Naive single Simple Moving Average trend-following strategy.
///
/// The simplest trend-following strategy: buys when the price is above a
/// single moving average and sells when below. No second average or
/// additional filter is used, so it reacts quickly but can generate many
/// whipsaws in sideways markets. Useful as a baseline trend-following
/// strategy.
///
/// Parameters
/// ----------
/// period : int, default=20
///     Moving average period.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:BuyAndHold
/// backtide.strategies:Momentum
/// backtide.strategies:SmaCrossover
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct SmaNaive {
    /// Moving average period.
    period: usize,
}

#[pymethods]
impl SmaNaive {
    #[new]
    #[pyo3(signature = (period=20))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Strategy for SmaNaive {
    const NAME: &'static str = "SMA (Naive)";
    const DESCRIPTION: &'static str =
        "Buys when price is above a moving average, sells when below.";
    const IS_MULTI_ASSET: bool = false;
}

/// Multi-timeframe Relative Strength Index portfolio rotation strategy.
///
/// Ranks assets by a composite score derived from long-term, medium-term,
/// and short-term RSI values and periodically rotates the portfolio into
/// the highest-scoring positions. The triple-time-frame approach helps
/// distinguish strong multi-horizon momentum from single-period flukes.
/// Useful for momentum rotation with multi-horizon confirmation.
///
/// Parameters
/// ----------
/// short_period : int, default=5
///     Short-term RSI period.
///
/// medium_period : int, default=14
///     Medium-term RSI period.
///
/// long_period : int, default=28
///     Long-term RSI period.
///
/// top_k : int, default=5
///     Number of top-ranked assets to hold.
///
/// rebalance_interval : int, default=20
///     Number of bars between rebalancing.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:MultiBollingerRotation
/// backtide.strategies:RocRotation
/// backtide.strategies:RsrsRotation
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct TripleRsiRotation {
    /// Short-term RSI period.
    short_period: usize,

    /// Medium-term RSI period.
    medium_period: usize,

    /// Long-term RSI period.
    long_period: usize,

    /// Number of top-ranked assets to hold.
    top_k: usize,

    /// Number of bars between rebalancing.
    rebalance_interval: usize,
}

#[pymethods]
impl TripleRsiRotation {
    #[new]
    #[pyo3(signature = (short_period=5, medium_period=14, long_period=28, top_k=5, rebalance_interval=20))]
    fn new(
        short_period: usize,
        medium_period: usize,
        long_period: usize,
        top_k: usize,
        rebalance_interval: usize,
    ) -> Self {
        Self {
            short_period,
            medium_period,
            long_period,
            top_k,
            rebalance_interval,
        }
    }

    #[allow(clippy::type_complexity)]
    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (usize, usize, usize, usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((
            cls,
            (
                self.short_period,
                self.medium_period,
                self.long_period,
                self.top_k,
                self.rebalance_interval,
            ),
        ))
    }
}

impl Strategy for TripleRsiRotation {
    const NAME: &'static str = "Triple RSI Rotation";
    const DESCRIPTION: &'static str =
        "Rotates stocks based on a combination of long, medium, and short-term RSI signals.";
    const IS_MULTI_ASSET: bool = true;
}

/// Classic channel-breakout trend-following system with ATR-based position sizing.
///
/// A classic trend-following system inspired by the Turtle Traders. Buys
/// on a breakout above the highest high of the last N bars and sells on
/// a breakdown below the lowest low of the last M bars. Uses ATR-based
/// position sizing to normalise risk across instruments. Useful for
/// systematic trend-following with built-in risk management.
///
/// Parameters
/// ----------
/// entry_period : int, default=20
///     Number of bars for the entry breakout (highest high).
///
/// exit_period : int, default=10
///     Number of bars for the exit breakdown (lowest low).
///
/// atr_period : int, default=20
///     ATR period for position sizing.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:BuyAndHold
/// backtide.strategies:Momentum
/// backtide.strategies:RiskAverse
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct TurtleTrading {
    /// Number of bars for the entry breakout (highest high).
    entry_period: usize,

    /// Number of bars for the exit breakdown (lowest low).
    exit_period: usize,

    /// ATR period for position sizing.
    atr_period: usize,
}

#[pymethods]
impl TurtleTrading {
    #[new]
    #[pyo3(signature = (entry_period=20, exit_period=10, atr_period=20))]
    fn new(entry_period: usize, exit_period: usize, atr_period: usize) -> Self {
        Self {
            entry_period,
            exit_period,
            atr_period,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (usize, usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.entry_period, self.exit_period, self.atr_period)))
    }
}

impl Strategy for TurtleTrading {
    const NAME: &'static str = "Turtle Trading";
    const DESCRIPTION: &'static str = "A classic trend-following strategy that buys on breakouts and sells on breakdowns, using ATR for position sizing.";
    const IS_MULTI_ASSET: bool = false;
}

/// Volatility Contraction Pattern breakout strategy.
///
/// Detects a Volatility Contraction Pattern: a series of progressively
/// tighter price consolidations with declining volume. When both price
/// range and volume have contracted sufficiently, the strategy enters long
/// on a breakout above the consolidation ceiling. Useful for swing trading
/// setups where decreasing supply precedes a sharp move.
///
/// Parameters
/// ----------
/// lookback : int, default=60
///     Number of bars to detect the contraction pattern.
///
/// contractions : int, default=3
///     Minimum number of contracting ranges required.
///
/// Attributes
/// ----------
/// name : str
///     Human-readable strategy name.
///
/// is_multi_asset : bool
///     Whether this is a multi-asset strategy.
///
/// See Also
/// --------
/// backtide.strategies:DoubleTop
/// backtide.strategies:RiskAverse
/// backtide.strategies:TurtleTrading
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug)]
pub struct Vcp {
    /// Number of bars to detect the contraction pattern.
    lookback: usize,

    /// Minimum number of contracting ranges required.
    contractions: usize,
}

#[pymethods]
impl Vcp {
    #[new]
    #[pyo3(signature = (lookback=60, contractions=3))]
    fn new(lookback: usize, contractions: usize) -> Self {
        Self {
            lookback,
            contractions,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.lookback, self.contractions)))
    }
}

impl Strategy for Vcp {
    const NAME: &'static str = "VCP";
    const DESCRIPTION: &'static str = "Buys on breakouts after price and volume volatility have contracted (Volatility Contraction Pattern).";
    const IS_MULTI_ASSET: bool = false;
}

// Apply shared pymethods (alphabetical)
strategy_pymethods!(AdaptiveRsi);
strategy_pymethods!(AlphaRsiPro);
strategy_pymethods!(BollingerMeanReversion);
strategy_pymethods!(BuyAndHold);
strategy_pymethods!(DoubleTop);
strategy_pymethods!(HybridAlphaRsi);
strategy_pymethods!(Macd);
strategy_pymethods!(Momentum);
strategy_pymethods!(MultiBollingerRotation);
strategy_pymethods!(RiskAverse);
strategy_pymethods!(Roc);
strategy_pymethods!(RocRotation);
strategy_pymethods!(Rsi);
strategy_pymethods!(Rsrs);
strategy_pymethods!(RsrsRotation);
strategy_pymethods!(SmaCrossover);
strategy_pymethods!(SmaNaive);
strategy_pymethods!(TripleRsiRotation);
strategy_pymethods!(TurtleTrading);
strategy_pymethods!(Vcp);
