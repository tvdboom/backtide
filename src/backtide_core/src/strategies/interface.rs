use pyo3::prelude::*;
use pyo3::types::PyType;
use std::collections::HashMap;

use crate::backtest::models::{new_order_id, Order, OrderType, Portfolio, State};
use crate::sizers::{Sizer, VolatilityScaled};
use crate::data::models::{Bar, InstrumentType};
use crate::indicators::interface::*;
use crate::indicators::utils::indicator_deterministic_name;
use crate::strategies::traits::Strategy;
use crate::strategies::utils::*;

/// Shared pymethods macro for all strategy structs.
///
/// Each built-in strategy carries pure-Rust trading logic. The engine dispatches
/// to that directly via [`BuiltinStrategy`], without going through the Python
/// interpreter on the hot path. This macro therefore only exposes the metadata
/// and factory bits the Python side actually uses.
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
            /// data : dict[str, np.array | pd.DataFrame | pl.DataFrame]
            ///     Keys are the experiment's symbols and values are the historical
            ///     OHLCV data available up to the current bar.
            ///
            /// portfolio : [Portfolio]
            ///     Current portfolio holdings (cash, positions and open orders).
            ///
            /// state : [State]
            ///     Current simulation state.
            ///
            /// indicators : dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None
            ///     The first keys are the indicator names. The second keys are the
            ///     experiment's symbols. The values are the pre-computed indicator
            ///     values. `None` if no indicators were selected.
            ///
            /// Returns
            /// -------
            /// list[[Order]]
            ///     Orders to place this tick.
            #[pyo3(
                signature = (
                    data: "dict[str, np.array | pd.DataFrame | pl.DataFrame]",
                    portfolio: "Portfolio",
                    state: "State",
                    indicators: "dict[str, dict[str, np.array | pd.DataFrame | pl.DataFrame]] | None" = None
                )
            )]
            fn evaluate<'py>(
                &self,
                _py: Python<'py>,
                data: &Bound<'py, PyAny>,
                portfolio: &Portfolio,
                state: &State,
                indicators: Option<Bound<'py, PyAny>>,
            ) -> PyResult<Vec<Order>> {
                let bars = extract_strategy_data_from_python(data)?;

                let indicator_data = extract_indicator_data_from_python(indicators.as_ref())?;
                let indicators = IndicatorView::new(&indicator_data, state.bar_index);

                Ok(self.evaluate_inner(&bars, portfolio, state, &indicators))
            }

            /// Indicators that must be computed up-front for this strategy.
            ///
            /// Returns a list of indicator instances, already parameterized
            /// with this strategy's current settings, that the engine will
            /// auto-include before the backtest starts.
            ///
            /// Returns
            /// -------
            /// list[[BaseIndicator]]
            ///     The required indicator instances.
            fn required_indicators(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
                <$ty as Strategy>::required_indicators_inner(self, py)
            }

            /// Return a debug representation.
            fn __repr__(&self) -> String {
                format!("{}()", <$ty as Strategy>::NAME)
            }
        }
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Strategy structs
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
    #[pyo3(signature = (min_period: "int"=8, max_period: "int"=28))]
    pub fn new(min_period: usize, max_period: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let rsi_short = indicator_deterministic_name("RSI", &[fmt_arg(self.min_period)]);
        let rsi_long = indicator_deterministic_name("RSI", &[fmt_arg(self.max_period)]);
        let n = bars.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            let r_short = match indicators.value(&rsi_short, sym) {
                Some(v) => v,
                None => continue,
            };
            let r_long = match indicators.value(&rsi_long, sym) {
                Some(v) => v,
                None => continue,
            };
            // Need enough history to compute both volatility windows.
            if c.len() <= self.max_period.max(self.min_period) {
                continue;
            }

            // Realised return volatility over short and long windows.
            let short_vol = window_return_std(&c[c.len() - self.min_period..]);
            let long_vol = window_return_std(&c[c.len() - self.max_period..]);

            // Pick the look-back that fits the current regime: short
            // period (more reactive) when recent vol > long-term vol,
            // long period (smoother) otherwise. Falls back to the long
            // RSI when either vol is undefined.
            let r = match (short_vol, long_vol) {
                (Some(sv), Some(lv)) if sv > lv => r_short,
                _ => r_long,
            };

            let last = *c.last().unwrap_or(&0.0);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);
            if r < 30.0 && cur <= 0.0 {
                if let Some(o) = buy_equal_weight(sym, n as usize, portfolio_cash(portfolio), last)
                {
                    orders.push(o);
                }
            } else if r > 70.0 && cur > 0.0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![
            Py::new(py, RelativeStrengthIndex::new(self.min_period))?.into_any(),
            Py::new(py, RelativeStrengthIndex::new(self.max_period))?.into_any(),
        ])
    }
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
    #[pyo3(signature = (period: "int"=14, vol_window: "int"=20))]
    pub fn new(period: usize, vol_window: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let rsi_name = indicator_deterministic_name("RSI", &[fmt_arg(self.period)]);
        let n = bars.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            let r = match indicators.value(&rsi_name, sym) {
                Some(v) => v,
                None => continue,
            };
            if c.len() <= self.vol_window {
                continue;
            }
            let last = *c.last().unwrap_or(&0.0);

            // Trend bias over `vol_window`: shifts both thresholds so buys
            // fire earlier in uptrends and sells fire earlier in downtrends.
            let prev = c[c.len() - 1 - self.vol_window];
            let trend_bias = if prev > 0.0 {
                (last - prev) / prev
            } else {
                0.0
            };
            let shift = (80.0 * trend_bias).clamp(-20.0, 20.0);
            let oversold = (30.0 + shift).clamp(15.0, 50.0);
            let overbought = (70.0 + shift).clamp(50.0, 85.0);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);
            if r < oversold && cur <= 0.0 {
                if let Some(o) = buy_equal_weight(sym, n as usize, portfolio_cash(portfolio), last)
                {
                    orders.push(o);
                }
            } else if r > overbought && cur > 0.0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![Py::new(py, RelativeStrengthIndex::new(self.period))?.into_any()])
    }
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
    #[pyo3(signature = (period: "int"=20, std_dev: "float"=2.0))]
    pub fn new(period: usize, std_dev: f64) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let bb_name =
            indicator_deterministic_name("BB", &[fmt_arg(self.period), fmt_arg(self.std_dev)]);
        let n = bars.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            // BB returns `[upper, middle, lower]`.
            let parts = match indicators.last(&bb_name, sym) {
                Some(v) if v.len() >= 3 => v,
                _ => continue,
            };
            let (upper, lower) = (parts[0], parts[2]);
            if !upper.is_finite() || !lower.is_finite() {
                continue;
            }
            let last = symbol_bars.last().map(|b| b.close).unwrap_or(0.0);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);
            if last < lower && cur <= 0.0 {
                if let Some(o) = buy_equal_weight(sym, n as usize, portfolio_cash(portfolio), last)
                {
                    orders.push(o);
                }
            } else if last > upper && cur > 0.0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![Py::new(py, BollingerBands::new(self.period, self.std_dev))?.into_any()])
    }
}

/// Passive baseline that buys once and holds indefinitely.
///
/// The simplest possible strategy: buy on the very first bar and hold the
/// position until the end of the simulation. Serves as the baseline
/// benchmark against which all other strategies are compared. Equivalent
/// to a passive index investment over the backtest window.
///
/// Parameters
/// ----------
/// symbol : str | None, default=None
///     Optional single ticker to buy and hold. When `None`, the strategy
///     equal-weights all symbols visible in the experiment.
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
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.strategies")]
#[derive(Clone, Debug, Default)]
pub struct BuyAndHold {
    /// Optional single symbol to buy.
    pub symbol: Option<String>,
}

#[pymethods]
impl BuyAndHold {
    #[new]
    #[pyo3(signature = (symbol: "str | None"=None))]
    pub fn new(symbol: Option<String>) -> Self {
        Self {
            symbol,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (Option<String>,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.symbol.clone(),)))
    }
}

impl Strategy for BuyAndHold {
    const NAME: &'static str = "Buy & Hold";
    const DESCRIPTION: &'static str =
        "Buys on the first day and holds to the end. A baseline for performance comparison.";
    const IS_MULTI_ASSET: bool = false;

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        _indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        if bars.is_empty() {
            return Vec::new();
        }

        // If a single symbol is configured, only buy that one once its history
        // has actually started.
        if let Some(target) = &self.symbol {
            // Skip once the target symbol is held *or* still pending fill.
            let has_position = portfolio.positions.get(target).is_some_and(|q| *q > 0.0);
            let has_pending_buy =
                portfolio.orders.iter().any(|o| o.symbol == *target && o.quantity > 0.0);
            if has_position || has_pending_buy {
                return Vec::new();
            }
            let row = match bars.iter().find(|(s, _)| s == target) {
                Some(r) => r,
                None => return Vec::new(),
            };
            let px = match row.1.last() {
                Some(b) if b.close.is_finite() && b.close > 0.0 => b.close,
                _ => return Vec::new(),
            };
            return buy_equal_weight(target, 1, portfolio_cash(portfolio), px)
                .map(|o| vec![o])
                .unwrap_or_default();
        }

        // Equal-weight, staggered entry: enter each symbol as soon as
        // its history starts, dividing *current* cash by the number of
        // symbols still needing entry.
        let mut needs_entry: Vec<(&str, f64)> = Vec::new();
        let mut already_entered = 0usize;
        for (sym, symbol_bars) in bars {
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);
            let pending = portfolio.orders.iter().any(|o| &o.symbol == sym && o.quantity > 0.0);
            if cur > 0.0 || pending {
                already_entered += 1;
                continue;
            }
            let px = match symbol_bars.last() {
                Some(b) if b.close.is_finite() && b.close > 0.0 => b.close,
                _ => continue,
            };
            needs_entry.push((sym.as_str(), px));
        }
        if needs_entry.is_empty() {
            return Vec::new();
        }
        let n_remaining = bars.len().saturating_sub(already_entered).max(1);
        let cash = portfolio_cash(portfolio);
        let mut orders = Vec::new();
        for (sym, px) in needs_entry {
            if let Some(o) = buy_equal_weight(sym, n_remaining, cash, px) {
                orders.push(o);
            }
        }
        orders
    }
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
    #[pyo3(signature = (lookback: "int"=60))]
    pub fn new(lookback: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        _indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let n = bars.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            if c.len() < self.lookback + 2 {
                continue;
            }
            let win = &c[c.len() - self.lookback..];

            // Find local maxima with a simple 3-bar criterion.
            let mut peaks: Vec<(usize, f64)> = Vec::new();
            for i in 1..win.len() - 1 {
                if win[i] > win[i - 1] && win[i] >= win[i + 1] {
                    peaks.push((i, win[i]));
                }
            }
            if peaks.len() < 2 {
                continue;
            }

            let p1 = peaks[peaks.len() - 2];
            let p2 = peaks[peaks.len() - 1];
            if p2.0 <= p1.0 + 1 {
                continue;
            }
            let resistance = p1.1.max(p2.1);
            let neckline = win[p1.0..=p2.0].iter().cloned().fold(f64::INFINITY, f64::min);

            // Two consecutive peaks at "roughly the same level" (within 3%).
            let level_match = resistance > 0.0 && (p1.1 - p2.1).abs() / resistance < 0.03;

            let last = *c.last().unwrap_or(&0.0);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);

            if level_match && last > resistance && cur <= 0.0 {
                if let Some(o) = buy_equal_weight(sym, n as usize, portfolio_cash(portfolio), last)
                {
                    orders.push(o);
                }
            } else if cur > 0.0 && last < neckline {
                // Pattern invalidated: bail out.
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }
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
    #[pyo3(signature = (min_period: "int"=8, max_period: "int"=28, vol_window: "int"=20))]
    pub fn new(min_period: usize, max_period: usize, vol_window: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let rsi_short = indicator_deterministic_name("RSI", &[fmt_arg(self.min_period)]);
        let rsi_long = indicator_deterministic_name("RSI", &[fmt_arg(self.max_period)]);
        let n = bars.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            let r_s = match indicators.value(&rsi_short, sym) {
                Some(v) => v,
                None => continue,
            };
            let r_l = match indicators.value(&rsi_long, sym) {
                Some(v) => v,
                None => continue,
            };
            if c.len() <= self.vol_window {
                continue;
            }
            let last = *c.last().unwrap_or(&0.0);

            // Adaptive period proxy: blend short and long RSIs.
            let r = 0.5 * (r_s + r_l);

            // Adaptive thresholds shifted by the trend bias over `vol_window`.
            let prev = c[c.len() - 1 - self.vol_window];
            let trend_bias = if prev > 0.0 {
                (last - prev) / prev
            } else {
                0.0
            };
            let shift = (80.0 * trend_bias).clamp(-20.0, 20.0);
            let oversold = (30.0 + shift).clamp(15.0, 50.0);
            let overbought = (70.0 + shift).clamp(50.0, 85.0);

            // Enter on a pullback inside the adaptive band instead of only at
            // deep oversold extremes, which are rare in persistent uptrends.
            let entry_level = oversold + 0.25 * (overbought - oversold);

            // Trend confirmation: only go long while price is above its
            // ``vol_window``-bar mean (a lightweight MA filter).
            let recent = &c[c.len() - self.vol_window..];
            let mean = recent.iter().sum::<f64>() / recent.len() as f64;
            let in_uptrend = last > mean;

            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);
            if r < entry_level && in_uptrend && cur <= 0.0 {
                if let Some(o) = buy_equal_weight(sym, n as usize, portfolio_cash(portfolio), last)
                {
                    orders.push(o);
                }
            } else if (r > overbought || !in_uptrend) && cur > 0.0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![
            Py::new(py, RelativeStrengthIndex::new(self.min_period))?.into_any(),
            Py::new(py, RelativeStrengthIndex::new(self.max_period))?.into_any(),
        ])
    }
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
    #[pyo3(signature = (fast_period: "int"=12, slow_period: "int"=26, signal_period: "int"=9))]
    pub fn new(fast_period: usize, slow_period: usize, signal_period: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let macd_name = indicator_deterministic_name(
            "MACD",
            &[fmt_arg(self.fast_period), fmt_arg(self.slow_period), fmt_arg(self.signal_period)],
        );
        let n = bars.len().max(1) as f64;
        let target = portfolio_equity(portfolio, bars) / n;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let parts = match indicators.last(&macd_name, sym) {
                Some(v) if v.len() >= 2 => v,
                _ => continue,
            };
            let (macd, signal) = (parts[0], parts[1]);
            if !macd.is_finite() || !signal.is_finite() {
                continue;
            }
            let last = symbol_bars.last().map(|b| b.close).unwrap_or(0.0);
            orders.extend(react_to_signal(sym, macd > signal, last, portfolio, target));
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![Py::new(
            py,
            MovingAverageConvergenceDivergence::new(
                self.fast_period,
                self.slow_period,
                self.signal_period,
            ),
        )?
        .into_any()])
    }
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
    #[pyo3(signature = (period: "int"=14, ma_period: "int"=50))]
    pub fn new(period: usize, ma_period: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        // Trend filter uses the auto-included ``SMA(ma_period)``; momentum
        // confirmation is a pure price comparison (no indicator required).
        let sma_name = indicator_deterministic_name("SMA", &[fmt_arg(self.ma_period)]);
        let n = bars.len().max(1) as f64;
        let target = portfolio_equity(portfolio, bars) / n;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            if c.len() <= self.period {
                continue;
            }
            let ma = match indicators.value(&sma_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let last = *c.last().unwrap_or(&0.0);
            let prev = c[c.len() - 1 - self.period];
            let positive_momentum = prev > 0.0 && last > prev;
            orders.extend(react_to_signal(
                sym,
                positive_momentum && last > ma,
                last,
                portfolio,
                target,
            ));
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![Py::new(py, SimpleMovingAverage::new(self.ma_period))?.into_any()])
    }
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
    #[pyo3(signature = (period: "int"=20, std_dev: "float"=2.0, top_k: "int"=5, rebalance_interval: "int"=20))]
    pub fn new(period: usize, std_dev: f64, top_k: usize, rebalance_interval: usize) -> Self {
        Self {
            period,
            std_dev,
            top_k,
            rebalance_interval,
        }
    }

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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        if !state.bar_index.is_multiple_of(self.rebalance_interval as u64) {
            return Vec::new();
        }
        let bb_name =
            indicator_deterministic_name("BB", &[fmt_arg(self.period), fmt_arg(self.std_dev)]);
        let scores: Vec<(String, f64)> = bars
            .iter()
            .map(|(s, symbol_bars)| {
                let last = symbol_bars.last().map(|b| b.close).unwrap_or(0.0);
                // BB returns `[upper, middle, lower]`. Rank by distance of
                // the latest close above the upper band — the further price
                // has broken out above the volatility envelope, the higher
                // the score.
                let score = match indicators.last(&bb_name, s) {
                    Some(v) if v.len() >= 3 && v[0].is_finite() && v[2].is_finite() => last - v[0],
                    _ => f64::NAN,
                };
                (s.clone(), score)
            })
            .collect();
        let last_prices: HashMap<String, f64> = bars
            .iter()
            .map(|(s, symbol_bars)| (s.clone(), symbol_bars.last().map(|b| b.close).unwrap_or(0.0)))
            .collect();
        rotation_orders(&scores, self.top_k, portfolio, &last_prices)
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![Py::new(py, BollingerBands::new(self.period, self.std_dev))?.into_any()])
    }
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
    #[pyo3(signature = (vol_period: "int"=14, breakout_period: "int"=20))]
    pub fn new(vol_period: usize, breakout_period: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let atr_name = indicator_deterministic_name("ATR", &[fmt_arg(self.vol_period)]);
        let n = bars.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            if c.len() <= self.breakout_period {
                continue;
            }
            let last = *c.last().unwrap_or(&0.0);
            if last <= 0.0 {
                continue;
            }

            // Donchian-style breakout window (excluding the current bar).
            let win = &c[c.len() - 1 - self.breakout_period..c.len() - 1];
            let high = win.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let low = win.iter().cloned().fold(f64::INFINITY, f64::min);

            // Low-volatility filter via auto-included ATR (ATR / price < 4%).
            let atr = match indicators.value(&atr_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let low_vol = atr.is_finite() && atr / last < 0.04;

            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);
            if low_vol && last >= high && cur <= 0.0 {
                if let Some(o) = buy_equal_weight(sym, n as usize, portfolio_cash(portfolio), last)
                {
                    orders.push(o);
                }
            } else if cur > 0.0 && last <= low {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![Py::new(py, AverageTrueRange::new(self.vol_period))?.into_any()])
    }
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
    #[pyo3(signature = (period: "int"=12))]
    pub fn new(period: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        _indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        // ROC is just a price-change ratio; not an indicator we precompute.
        let n = bars.len().max(1) as f64;
        let target = portfolio_equity(portfolio, bars) / n;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            if c.len() <= self.period {
                continue;
            }
            let prev = c[c.len() - 1 - self.period];
            if prev <= 0.0 {
                continue;
            }
            let last = *c.last().unwrap_or(&0.0);
            let roc = (last - prev) / prev * 100.0;
            orders.extend(react_to_signal(sym, roc > 5.0, last, portfolio, target));
        }
        orders
    }
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
    #[pyo3(signature = (period: "int"=12, top_k: "int"=5, rebalance_interval: "int"=20))]
    pub fn new(period: usize, top_k: usize, rebalance_interval: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        state: &State,
        _indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        if !state.bar_index.is_multiple_of(self.rebalance_interval as u64) {
            return Vec::new();
        }
        let period = self.period;
        let scores: Vec<(String, f64)> = bars
            .iter()
            .map(|(s, symbol_bars)| {
                let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
                let score = if c.len() > period {
                    let prev = c[c.len() - 1 - period];
                    if prev > 0.0 {
                        (*c.last().unwrap_or(&0.0) - prev) / prev * 100.0
                    } else {
                        f64::NAN
                    }
                } else {
                    f64::NAN
                };
                (s.clone(), score)
            })
            .collect();
        let last_prices: HashMap<String, f64> = bars
            .iter()
            .map(|(s, symbol_bars)| (s.clone(), symbol_bars.last().map(|b| b.close).unwrap_or(0.0)))
            .collect();
        rotation_orders(&scores, self.top_k, portfolio, &last_prices)
    }
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
    #[pyo3(signature = (rsi_period: "int"=14, bb_period: "int"=20, bb_std: "float"=2.0))]
    pub fn new(rsi_period: usize, bb_period: usize, bb_std: f64) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let rsi_name = indicator_deterministic_name("RSI", &[fmt_arg(self.rsi_period)]);
        let bb_name =
            indicator_deterministic_name("BB", &[fmt_arg(self.bb_period), fmt_arg(self.bb_std)]);
        let n = bars.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let r = match indicators.value(&rsi_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let bb = match indicators.last(&bb_name, sym) {
                Some(v) if v.len() >= 3 && v[0].is_finite() && v[2].is_finite() => v,
                _ => continue,
            };
            // BB returns `[upper, middle, lower]`.
            let (upper, lower) = (bb[0], bb[2]);
            let last = symbol_bars.last().map(|b| b.close).unwrap_or(0.0);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);
            // Dual confirmation: oversold RSI + price at/below lower band.
            if r < 30.0 && last <= lower && cur <= 0.0 {
                let cash = portfolio_cash(portfolio);
                if let Some(o) = buy_equal_weight(sym, n as usize, cash, last) {
                    orders.push(o);
                }
            } else if (r > 70.0 || last >= upper) && cur > 0.0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![
            Py::new(py, RelativeStrengthIndex::new(self.rsi_period))?.into_any(),
            Py::new(py, BollingerBands::new(self.bb_period, self.bb_std))?.into_any(),
        ])
    }
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
    #[pyo3(signature = (period: "int"=18))]
    pub fn new(period: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        _indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let n = bars.len().max(1) as f64;
        let target = portfolio_equity(portfolio, bars) / n;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            if c.len() < self.period {
                continue;
            }
            let win = &c[c.len() - self.period..];
            let (slope, mean_y) = match linreg_slope(win) {
                Some(v) => v,
                None => continue,
            };
            if mean_y <= 0.0 {
                continue;
            }
            // Normalised slope: rise per bar relative to mean price.
            let strength = slope / mean_y;
            let last = *c.last().unwrap_or(&0.0);
            // ``> 0`` would whipsaw constantly; require a sustained ascent.
            let signal_long = strength > 0.001 && last > mean_y;
            orders.extend(react_to_signal(sym, signal_long, last, portfolio, target));
        }
        orders
    }
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
    #[pyo3(signature = (period: "int"=18, top_k: "int"=5, rebalance_interval: "int"=20))]
    pub fn new(period: usize, top_k: usize, rebalance_interval: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        state: &State,
        _indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        if !state.bar_index.is_multiple_of(self.rebalance_interval as u64) {
            return Vec::new();
        }
        // Rank by the normalised regression slope of recent closes (RSRS proxy).
        let period = self.period;
        let scores: Vec<(String, f64)> = bars
            .iter()
            .map(|(s, symbol_bars)| {
                let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
                let score = if c.len() >= period {
                    let win = &c[c.len() - period..];
                    match linreg_slope(win) {
                        Some((slope, mean_y)) if mean_y > 0.0 => slope / mean_y,
                        _ => f64::NAN,
                    }
                } else {
                    f64::NAN
                };
                (s.clone(), score)
            })
            .collect();
        let last_prices: HashMap<String, f64> = bars
            .iter()
            .map(|(s, symbol_bars)| (s.clone(), symbol_bars.last().map(|b| b.close).unwrap_or(0.0)))
            .collect();
        rotation_orders(&scores, self.top_k, portfolio, &last_prices)
    }
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
    #[pyo3(signature = (fast_period: "int"=20, slow_period: "int"=50))]
    pub fn new(fast_period: usize, slow_period: usize) -> Self {
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
    const NAME: &'static str = "Crossover SMA";
    const DESCRIPTION: &'static str =
        "Buys on a golden cross (fast MA over slow MA), sells on a death cross.";
    const IS_MULTI_ASSET: bool = false;

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let fast_name = indicator_deterministic_name("SMA", &[fmt_arg(self.fast_period)]);
        let slow_name = indicator_deterministic_name("SMA", &[fmt_arg(self.slow_period)]);
        let n = bars.len().max(1) as f64;
        let target = portfolio_equity(portfolio, bars) / n;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let fast = match indicators.value(&fast_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let slow = match indicators.value(&slow_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let last = symbol_bars.last().map(|b| b.close).unwrap_or(0.0);
            orders.extend(react_to_signal(sym, fast > slow, last, portfolio, target));
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![
            Py::new(py, SimpleMovingAverage::new(self.fast_period))?.into_any(),
            Py::new(py, SimpleMovingAverage::new(self.slow_period))?.into_any(),
        ])
    }
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
    #[pyo3(signature = (period: "int"=20))]
    pub fn new(period: usize) -> Self {
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
    const NAME: &'static str = "Naive SMA";
    const DESCRIPTION: &'static str =
        "Buys when price is above a moving average, sells when below.";
    const IS_MULTI_ASSET: bool = false;

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let name = indicator_deterministic_name("SMA", &[fmt_arg(self.period)]);
        let n = bars.len().max(1) as f64;
        let target = portfolio_equity(portfolio, bars) / n;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let last = match symbol_bars.last() {
                Some(b) => b.close,
                None => continue,
            };
            let ma = match indicators.value(&name, sym) {
                Some(v) => v,
                None => continue,
            };
            orders.extend(react_to_signal(sym, last > ma, last, portfolio, target));
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![Py::new(py, SimpleMovingAverage::new(self.period))?.into_any()])
    }
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
    #[pyo3(signature = (short_period: "int"=5, medium_period: "int"=14, long_period: "int"=28, top_k: "int"=5, rebalance_interval: "int"=20))]
    pub fn new(
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        if !state.bar_index.is_multiple_of(self.rebalance_interval as u64) {
            return Vec::new();
        }
        let short_name = indicator_deterministic_name("RSI", &[fmt_arg(self.short_period)]);
        let medium_name = indicator_deterministic_name("RSI", &[fmt_arg(self.medium_period)]);
        let long_name = indicator_deterministic_name("RSI", &[fmt_arg(self.long_period)]);
        let scores: Vec<(String, f64)> = bars
            .iter()
            .map(|(s, _symbol_bars)| {
                let r1 = indicators.value(&short_name, s);
                let r2 = indicators.value(&medium_name, s);
                let r3 = indicators.value(&long_name, s);
                let score = match (r1, r2, r3) {
                    (Some(a), Some(b), Some(c)) => (a + b + c) / 3.0,
                    _ => f64::NAN,
                };
                (s.clone(), score)
            })
            .collect();
        let last_prices: HashMap<String, f64> = bars
            .iter()
            .map(|(s, symbol_bars)| (s.clone(), symbol_bars.last().map(|b| b.close).unwrap_or(0.0)))
            .collect();
        rotation_orders(&scores, self.top_k, portfolio, &last_prices)
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![
            Py::new(py, RelativeStrengthIndex::new(self.short_period))?.into_any(),
            Py::new(py, RelativeStrengthIndex::new(self.medium_period))?.into_any(),
            Py::new(py, RelativeStrengthIndex::new(self.long_period))?.into_any(),
        ])
    }
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
    #[pyo3(signature = (entry_period: "int"=20, exit_period: "int"=10, atr_period: "int"=20))]
    pub fn new(entry_period: usize, exit_period: usize, atr_period: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        // Donchian channel breakouts are pure price extremes – no indicator
        // computation required for the signal itself.
        let atr_name = indicator_deterministic_name("ATR", &[fmt_arg(self.atr_period)]);
        let equity = portfolio_equity(portfolio, bars);
        let cash = portfolio_cash(portfolio);
        let risk_pct = 0.01_f64;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            if c.len() < self.entry_period.max(self.exit_period) {
                continue;
            }
            let last = *c.last().unwrap_or(&0.0);
            if last <= 0.0 {
                continue;
            }
            let entry_high =
                c[c.len() - self.entry_period..].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let exit_low =
                c[c.len() - self.exit_period..].iter().cloned().fold(f64::INFINITY, f64::min);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);
            if last >= entry_high && cur <= 0.0 {
                // ATR-based "N" sizing via the VolatilityScaled sizer —
                // 1 % of equity per unit of ATR, capped by available cash.
                let atr = match indicators.value(&atr_name, sym) {
                    Some(v) if v > 0.0 => v,
                    _ => continue,
                };
                let sizer = VolatilityScaled::new(risk_pct);
                let target_qty = match sizer.calculate(equity, last, None, Some(atr)) {
                    Ok(q) if q > 0.0 => q,
                    _ => continue,
                };
                // Never spend more than the cash on hand.
                let max_affordable = cash / last;
                let qty = target_qty.min(max_affordable);
                if qty > 0.0 {
                    orders.push(Order {
                        id: new_order_id(),
                        symbol: sym.to_owned(),
                        order_type: OrderType::Market,
                        quantity: qty,
                        price: None,
                        limit_price: None,
                        sizer: None,
                    });
                }
            } else if last <= exit_low && cur > 0.0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }

    fn required_indicators_inner(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(vec![Py::new(py, AverageTrueRange::new(self.atr_period))?.into_any()])
    }
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
    #[pyo3(signature = (lookback: "int"=60, contractions: "int"=3))]
    pub fn new(lookback: usize, contractions: usize) -> Self {
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

    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        _state: &State,
        _indicators: &IndicatorView<'_>,
    ) -> Vec<Order> {
        let n = bars.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, symbol_bars) in bars {
            let c: Vec<f64> = symbol_bars.iter().map(|b| b.close).collect();
            if self.contractions < 2 || c.len() < self.lookback {
                continue;
            }
            let win = &c[c.len() - self.lookback..];
            let seg_len = win.len() / self.contractions;
            if seg_len < 2 {
                continue;
            }

            // Walk through `contractions` consecutive segments and verify that
            // each segment's range is strictly tighter than the previous one.
            let mut prev_range = f64::INFINITY;
            let mut ok = true;
            for k in 0..self.contractions {
                let seg = &win[k * seg_len..(k + 1) * seg_len];
                let hi = seg.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let lo = seg.iter().cloned().fold(f64::INFINITY, f64::min);
                let r = hi - lo;
                if !r.is_finite() || r >= prev_range {
                    ok = false;
                    break;
                }
                prev_range = r;
            }
            // Breakout must clear prior resistance; exclude the latest bar
            // so `last > ceiling` is achievable.
            let ceiling = win[..win.len() - 1].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            if !ok || !ceiling.is_finite() {
                continue;
            }

            let last = *c.last().unwrap_or(&0.0);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);

            if last > ceiling && cur <= 0.0 {
                if let Some(o) = buy_equal_weight(sym, n as usize, portfolio_cash(portfolio), last)
                {
                    orders.push(o);
                }
            } else if cur > 0.0 && last < ceiling * 0.92 {
                // Stop-out 8% below the breakout ceiling.
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }
}

// Apply shared pymethods
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

// ─────────────────────────────────────────────────────────────────────────────
// Built-in strategy dispatch
// ─────────────────────────────────────────────────────────────────────────────

/// Owned copy of any built-in strategy.
#[derive(Clone, Debug)]
pub enum BuiltinStrategy {
    AdaptiveRsi(AdaptiveRsi),
    AlphaRsiPro(AlphaRsiPro),
    BollingerMeanReversion(BollingerMeanReversion),
    BuyAndHold(BuyAndHold),
    DoubleTop(DoubleTop),
    HybridAlphaRsi(HybridAlphaRsi),
    Macd(Macd),
    Momentum(Momentum),
    MultiBollingerRotation(MultiBollingerRotation),
    RiskAverse(RiskAverse),
    Roc(Roc),
    RocRotation(RocRotation),
    Rsi(Rsi),
    Rsrs(Rsrs),
    RsrsRotation(RsrsRotation),
    SmaCrossover(SmaCrossover),
    SmaNaive(SmaNaive),
    TripleRsiRotation(TripleRsiRotation),
    TurtleTrading(TurtleTrading),
    Vcp(Vcp),
}

impl BuiltinStrategy {
    /// Try to clone an owned [`BuiltinStrategy`] out of a Python object.
    /// Returns `None` for custom (Python-defined) strategies.
    pub fn try_from_py(py: Python<'_>, obj: &Py<PyAny>) -> Option<Self> {
        let bound = obj.bind(py);
        macro_rules! try_dispatch {
            ($($variant:ident => $t:ty),* $(,)?) => {
                $(
                    if let Ok(b) = bound.cast::<$t>() {
                        return Some(BuiltinStrategy::$variant(b.borrow().clone()));
                    }
                )*
            };
        }

        try_dispatch!(
            AdaptiveRsi => AdaptiveRsi,
            AlphaRsiPro => AlphaRsiPro,
            BollingerMeanReversion => BollingerMeanReversion,
            BuyAndHold => BuyAndHold,
            DoubleTop => DoubleTop,
            HybridAlphaRsi => HybridAlphaRsi,
            Macd => Macd,
            Momentum => Momentum,
            MultiBollingerRotation => MultiBollingerRotation,
            RiskAverse => RiskAverse,
            Roc => Roc,
            RocRotation => RocRotation,
            Rsi => Rsi,
            Rsrs => Rsrs,
            RsrsRotation => RsrsRotation,
            SmaCrossover => SmaCrossover,
            SmaNaive => SmaNaive,
            TripleRsiRotation => TripleRsiRotation,
            TurtleTrading => TurtleTrading,
            Vcp => Vcp,
        );
        None
    }

    /// Pure-Rust dispatch to the underlying [`Strategy::evaluate_inner`].
    pub fn evaluate(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        state: &State,
        indicators: &IndicatorView<'_>,
        it_map: &HashMap<String, InstrumentType>,
    ) -> Vec<Order> {
        macro_rules! delegate {
            ($($variant:ident),* $(,)?) => {
                match self {
                    $(BuiltinStrategy::$variant(s) => s.evaluate_inner(bars, portfolio, state, indicators),)*
                }
            };
        }

        let mut orders = delegate!(
            AdaptiveRsi,
            AlphaRsiPro,
            BollingerMeanReversion,
            BuyAndHold,
            DoubleTop,
            HybridAlphaRsi,
            Macd,
            Momentum,
            MultiBollingerRotation,
            RiskAverse,
            Roc,
            RocRotation,
            Rsi,
            Rsrs,
            RsrsRotation,
            SmaCrossover,
            SmaNaive,
            TripleRsiRotation,
            TurtleTrading,
            Vcp,
        );

        // Return rounded quantities for instruments that don't allow fractional shares.
        orders.retain_mut(|o| {
            let it = it_map.get(&o.symbol).unwrap();

            if !it.allows_fractional_quantities() && o.quantity.fract() != 0. {
                let whole_abs = o.quantity.abs().floor();
                if whole_abs <= 0.0 {
                    return false; // Exclude orders with qty=0
                }

                o.quantity = whole_abs.copysign(o.quantity);
            }

            true
        });

        orders
    }
}
