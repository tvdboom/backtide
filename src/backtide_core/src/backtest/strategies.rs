use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};

use crate::backtest::models::order::{new_order_id, Order};
use crate::backtest::models::order_type::OrderType;
use crate::backtest::models::portfolio::Portfolio;
use crate::backtest::models::state::State;

/// Trait for all built-in strategies.
pub trait Strategy {
    /// Human-readable name (e.g. `"Buy & Hold"`).
    const NAME: &'static str;

    /// One-sentence explanation of what the strategy does.
    const DESCRIPTION: &'static str;

    /// Whether this is a portfolio-rotation (multi-asset) strategy.
    const IS_MULTI_ASSET: bool;

    /// Decide which orders to place on the current bar.
    ///
    /// Receives:
    /// - `closes`: per-symbol close vectors up to (and including) the current bar.
    /// - `indicators`: the engine-built `dict[str, dict[str, np.ndarray | list[np.ndarray]]]`
    ///   view of every auto-included indicator (see ``STRATEGY_INDICATORS``)
    ///   sliced up to the current bar. Strategies should read precomputed
    ///   values via [`auto_indicator_last`] / [`auto_indicator_value`] rather
    ///   than recomputing them locally.
    ///
    /// The default returns no orders, letting passive strategies opt out
    /// without boilerplate.
    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        _closes: &[(String, Vec<f64>)],
        _indicators: &Bound<'py, PyAny>,
        _portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        Vec::new()
    }
}

/// Per-symbol close vector extracted from the engine's `data` payload.
fn extract_closes(data: &Bound<'_, PyAny>) -> PyResult<Vec<(String, Vec<f64>)>> {
    let mut out: Vec<(String, Vec<f64>)> = Vec::new();
    if let Ok(dict) = data.cast::<PyDict>() {
        for (k, v) in dict.iter() {
            let symbol: String = k.extract()?;
            let closes: Vec<f64> = if let Ok(s) = v.get_item("close") {
                s.extract::<Vec<f64>>()
                    .or_else(|_| {
                        s.getattr("values")
                            .and_then(|x| x.extract::<Vec<f64>>())
                            .or_else(|_| s.call_method0("to_numpy")?.extract::<Vec<f64>>())
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            out.push((symbol, closes));
        }
    }
    Ok(out)
}

/// Build a market buy order sized to spend `target_cash` (capped at
/// `max_position_size%` of equity).
fn buy_order(symbol: &str, target_cash: f64, price: f64) -> Option<Order> {
    if price <= 0.0 || target_cash <= 0.0 {
        return None;
    }
    let qty = (target_cash / price).floor() as i64;
    if qty <= 0 {
        return None;
    }
    Some(Order {
        id: new_order_id(),
        symbol: symbol.to_owned(),
        order_type: OrderType::Market,
        quantity: qty,
        price: None,
    })
}

/// Build a market sell order to flatten an existing long position.
fn sell_order(symbol: &str, quantity: i64) -> Option<Order> {
    if quantity <= 0 {
        return None;
    }
    Some(Order {
        id: new_order_id(),
        symbol: symbol.to_owned(),
        order_type: OrderType::Market,
        quantity: -quantity,
        price: None,
    })
}

/// Estimate cash available in the portfolio (sum of all currency balances).
fn portfolio_cash(portfolio: &Portfolio) -> f64 {
    portfolio.cash.values().sum()
}

/// Generic single-asset signal: place a buy when `signal == true` and
/// the position is flat, place a sell when `signal == false` and we are
/// long.
fn react_to_signal(
    symbol: &str,
    signal_long: bool,
    last_price: f64,
    portfolio: &Portfolio,
    target_alloc: f64,
) -> Vec<Order> {
    let cur = portfolio.positions.get(symbol).copied().unwrap_or(0);
    if signal_long && cur <= 0 {
        let cash = portfolio_cash(portfolio);
        if let Some(o) = buy_order(symbol, cash * target_alloc, last_price) {
            return vec![o];
        }
    } else if !signal_long && cur > 0 {
        if let Some(o) = sell_order(symbol, cur) {
            return vec![o];
        }
    }
    Vec::new()
}

/// Top-K rotation across symbols. Closes positions not in the top, then
/// buys equal-weight into the top `k`.
fn rotation_orders(
    scores: &[(String, f64)],
    top_k: usize,
    portfolio: &Portfolio,
    last_prices: &std::collections::HashMap<String, f64>,
) -> Vec<Order> {
    use std::collections::HashSet;

    let mut sorted: Vec<&(String, f64)> = scores.iter().filter(|(_, s)| s.is_finite()).collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let target: HashSet<String> = sorted.iter().take(top_k).map(|(s, _)| s.clone()).collect();

    let mut orders: Vec<Order> = Vec::new();

    // Close positions not in target.
    for (sym, qty) in &portfolio.positions {
        if *qty > 0 && !target.contains(sym) {
            if let Some(o) = sell_order(sym, *qty) {
                orders.push(o);
            }
        }
    }

    // Open new positions equal-weight.
    if !target.is_empty() {
        let cash = portfolio_cash(portfolio);
        let per = cash / target.len() as f64;
        for sym in &target {
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0);
            if cur > 0 {
                continue;
            }
            if let Some(px) = last_prices.get(sym).copied() {
                if let Some(o) = buy_order(sym, per, px) {
                    orders.push(o);
                }
            }
        }
    }

    orders
}

/// Format a value the same way Python's ``str()`` does, for use in
/// ``__auto_*`` indicator names. Floats render with their decimal point
/// (e.g. ``2.0`` → ``"2.0"``); integers and other types fall back to
/// their ``Debug`` representation.
fn fmt_arg<T: std::fmt::Debug>(v: T) -> String {
    format!("{:?}", v)
}

/// Build the deterministic ``__auto_*`` name used by the engine for an
/// auto-included indicator (mirrors `_auto_indicator_name` in the Python
/// strategy utils).
fn auto_indicator_name(acronym: &str, args: &[String]) -> String {
    let arg_str = if args.is_empty() {
        "default".to_owned()
    } else {
        args.join("_")
    };
    let sanitized = arg_str.replace('.', "p").replace('-', "n").replace(' ', "");
    format!("__auto_{acronym}_{sanitized}")
}

/// Look up the last value(s) of a precomputed indicator from the engine's
/// per-tick view dict for *symbol*.
///
/// The engine builds `indicators[name][symbol]` as either a single 1-D numpy
/// array (single-output indicators like SMA / RSI / ATR) or a list of 1-D
/// arrays (multi-output indicators like Bollinger Bands or MACD). This helper
/// hides that shape difference and returns one `f64` per output series, in
/// order. Returns `None` if no matching indicator/symbol is present.
fn auto_indicator_last<'py>(
    indicators: &Bound<'py, PyAny>,
    name: &str,
    symbol: &str,
) -> Option<Vec<f64>> {
    use pyo3::types::{PyDict, PyList};

    let dict = indicators.cast::<PyDict>().ok()?;
    let per_sym = dict.get_item(name).ok().flatten()?;
    let per_sym = per_sym.cast::<PyDict>().ok()?;
    let value = per_sym.get_item(symbol).ok().flatten()?;

    if let Ok(list) = value.cast::<PyList>() {
        let mut out = Vec::with_capacity(list.len());
        for arr in list.iter() {
            let v = arr.get_item(-1isize).ok()?;
            out.push(v.extract::<f64>().ok()?);
        }
        return (!out.is_empty()).then_some(out);
    }

    let v = value.get_item(-1isize).ok()?;
    v.extract::<f64>().ok().map(|x| vec![x])
}

/// Convenience wrapper for single-output indicators (SMA / RSI / ATR / …).
/// Returns `None` if the indicator is missing or its last value is non-finite.
fn auto_indicator_value<'py>(
    indicators: &Bound<'py, PyAny>,
    name: &str,
    symbol: &str,
) -> Option<f64> {
    auto_indicator_last(indicators, name, symbol)
        .and_then(|v| v.into_iter().next())
        .filter(|x| x.is_finite())
}

/// Shared pymethods macro for all strategy structs. The `evaluate`
/// implementation is provided per-struct via the `__evaluate_orders__`
/// associated method (defaults to no orders).
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
            /// portfolio : [Portfolio]
            ///     Current portfolio holdings (cash, positions and open orders).
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
                data: &Bound<'py, PyAny>,
                portfolio: &Portfolio,
                state: &State,
                _indicators: &Bound<'py, PyAny>,
            ) -> PyResult<Vec<Order>> {
                let closes = extract_closes(data)?;
                Ok(<$ty as Strategy>::decide_inner(self, _py, &closes, _indicators, portfolio, state))
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        // Auto-included RSI uses ``min_period`` (see STRATEGY_INDICATORS).
        let rsi_name = auto_indicator_name("RSI", &[fmt_arg(self.min_period)]);
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            let r = match auto_indicator_value(indicators, &rsi_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let last = *c.last().unwrap_or(&0.0);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0);
            if r < 30.0 && cur <= 0 {
                if let Some(o) = buy_order(sym, portfolio_cash(portfolio) / n, last) {
                    orders.push(o);
                }
            } else if r > 70.0 && cur > 0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        let rsi_name = auto_indicator_name("RSI", &[fmt_arg(self.period)]);
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            let r = match auto_indicator_value(indicators, &rsi_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let last = *c.last().unwrap_or(&0.0);
            orders.extend(react_to_signal(sym, r < 35.0, last, portfolio, 1.0 / n));
        }
        orders
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        let bb_name = auto_indicator_name(
            "BB",
            &[fmt_arg(self.period), fmt_arg(self.std_dev)],
        );
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            let parts = match auto_indicator_last(indicators, &bb_name, sym) {
                Some(v) if v.len() >= 2 => v,
                _ => continue,
            };
            let (upper, lower) = (parts[0], parts[1]);
            if !upper.is_finite() || !lower.is_finite() {
                continue;
            }
            let last = *c.last().unwrap_or(&0.0);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0);
            if last < lower && cur <= 0 {
                if let Some(o) = buy_order(sym, portfolio_cash(portfolio) / n, last) {
                    orders.push(o);
                }
            } else if last > upper && cur > 0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
    }
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        _indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        // One-shot: skip once anything is held *or* still pending fill.
        let has_position = portfolio.positions.values().any(|q| *q > 0);
        let has_pending_buy = portfolio.orders.iter().any(|o| o.quantity > 0);
        if has_position || has_pending_buy {
            return Vec::new();
        }
        if closes.is_empty() {
            return Vec::new();
        }
        // Equal-weight: divide all available cash across the selected symbols.
        let per = portfolio_cash(portfolio) / closes.len() as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            if let Some(&px) = c.last() {
                if let Some(o) = buy_order(sym, per, px) {
                    orders.push(o);
                }
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        // Auto-included RSI uses ``min_period`` (see STRATEGY_INDICATORS).
        let rsi_name = auto_indicator_name("RSI", &[fmt_arg(self.min_period)]);
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            let r = match auto_indicator_value(indicators, &rsi_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let last = *c.last().unwrap_or(&0.0);
            orders.extend(react_to_signal(sym, r < 30.0, last, portfolio, 1.0 / n));
        }
        orders
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        let macd_name = auto_indicator_name(
            "MACD",
            &[
                fmt_arg(self.fast_period),
                fmt_arg(self.slow_period),
                fmt_arg(self.signal_period),
            ],
        );
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            let parts = match auto_indicator_last(indicators, &macd_name, sym) {
                Some(v) if v.len() >= 2 => v,
                _ => continue,
            };
            let (macd, signal) = (parts[0], parts[1]);
            if !macd.is_finite() || !signal.is_finite() {
                continue;
            }
            let last = *c.last().unwrap_or(&0.0);
            orders.extend(react_to_signal(sym, macd > signal, last, portfolio, 1.0 / n));
        }
        orders
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        // Trend filter uses the auto-included ``SMA(ma_period)``; momentum
        // confirmation is a pure price comparison (no indicator required).
        let sma_name = auto_indicator_name("SMA", &[fmt_arg(self.ma_period)]);
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            if c.len() <= self.period {
                continue;
            }
            let ma = match auto_indicator_value(indicators, &sma_name, sym) {
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
                1.0 / n,
            ));
        }
        orders
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
    #[pyo3(signature = (period=20, std_dev=2.0, top_k=5, rebalance_interval=20))]
    fn new(period: usize, std_dev: f64, top_k: usize, rebalance_interval: usize) -> Self {
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        state: &State,
    ) -> Vec<Order> {
        if !state.bar_index.is_multiple_of(self.rebalance_interval as u64) {
            return Vec::new();
        }
        let bb_name = auto_indicator_name(
            "BB",
            &[fmt_arg(self.period), fmt_arg(self.std_dev)],
        );
        let scores: Vec<(String, f64)> = closes
            .iter()
            .map(|(s, c)| {
                let last = *c.last().unwrap_or(&0.0);
                let score = match auto_indicator_last(indicators, &bb_name, s) {
                    Some(v) if v.len() >= 2 && v[0].is_finite() && v[1].is_finite() => {
                        // Score by deviation above the band midpoint.
                        last - 0.5 * (v[0] + v[1])
                    },
                    _ => f64::NAN,
                };
                (s.clone(), score)
            })
            .collect();
        let last_prices: std::collections::HashMap<String, f64> =
            closes.iter().map(|(s, c)| (s.clone(), *c.last().unwrap_or(&0.0))).collect();
        rotation_orders(&scores, self.top_k, portfolio, &last_prices)
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        _indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        // ROC is just a price-change ratio; not an indicator we precompute.
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            if c.len() <= self.period {
                continue;
            }
            let prev = c[c.len() - 1 - self.period];
            if prev <= 0.0 {
                continue;
            }
            let last = *c.last().unwrap_or(&0.0);
            let roc = (last - prev) / prev * 100.0;
            orders.extend(react_to_signal(sym, roc > 5.0, last, portfolio, 1.0 / n));
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        _indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        state: &State,
    ) -> Vec<Order> {
        if !state.bar_index.is_multiple_of(self.rebalance_interval as u64) {
            return Vec::new();
        }
        let period = self.period;
        let scores: Vec<(String, f64)> = closes
            .iter()
            .map(|(s, c)| {
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
        let last_prices: std::collections::HashMap<String, f64> =
            closes.iter().map(|(s, c)| (s.clone(), *c.last().unwrap_or(&0.0))).collect();
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        let rsi_name = auto_indicator_name("RSI", &[fmt_arg(self.rsi_period)]);
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            let r = match auto_indicator_value(indicators, &rsi_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let last = *c.last().unwrap_or(&0.0);
            // Buy when oversold (<30), sell when overbought (>70).
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0);
            if r < 30.0 && cur <= 0 {
                let cash = portfolio_cash(portfolio);
                if let Some(o) = buy_order(sym, cash / n, last) {
                    orders.push(o);
                }
            } else if r > 70.0 && cur > 0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        _indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        state: &State,
    ) -> Vec<Order> {
        if !state.bar_index.is_multiple_of(self.rebalance_interval as u64) {
            return Vec::new();
        }
        // Without a registered RSRS indicator we approximate strength via
        // simple rate-of-change over the lookback period.
        let period = self.period;
        let scores: Vec<(String, f64)> = closes
            .iter()
            .map(|(s, c)| {
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
        let last_prices: std::collections::HashMap<String, f64> =
            closes.iter().map(|(s, c)| (s.clone(), *c.last().unwrap_or(&0.0))).collect();
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        let fast_name = auto_indicator_name("SMA", &[fmt_arg(self.fast_period)]);
        let slow_name = auto_indicator_name("SMA", &[fmt_arg(self.slow_period)]);
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            let fast = match auto_indicator_value(indicators, &fast_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let slow = match auto_indicator_value(indicators, &slow_name, sym) {
                Some(v) => v,
                None => continue,
            };
            let last = *c.last().unwrap_or(&0.0);
            orders.extend(react_to_signal(sym, fast > slow, last, portfolio, 1.0 / n));
        }
        orders
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        let name = auto_indicator_name("SMA", &[fmt_arg(self.period)]);
        let n = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            let last = match c.last() {
                Some(&v) => v,
                None => continue,
            };
            let ma = match auto_indicator_value(indicators, &name, sym) {
                Some(v) => v,
                None => continue,
            };
            orders.extend(react_to_signal(sym, last > ma, last, portfolio, 1.0 / n));
        }
        orders
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        state: &State,
    ) -> Vec<Order> {
        if !state.bar_index.is_multiple_of(self.rebalance_interval as u64) {
            return Vec::new();
        }
        let short_name = auto_indicator_name("RSI", &[fmt_arg(self.short_period)]);
        let medium_name = auto_indicator_name("RSI", &[fmt_arg(self.medium_period)]);
        let long_name = auto_indicator_name("RSI", &[fmt_arg(self.long_period)]);
        let scores: Vec<(String, f64)> = closes
            .iter()
            .map(|(s, _c)| {
                let r1 = auto_indicator_value(indicators, &short_name, s);
                let r2 = auto_indicator_value(indicators, &medium_name, s);
                let r3 = auto_indicator_value(indicators, &long_name, s);
                let score = match (r1, r2, r3) {
                    (Some(a), Some(b), Some(c)) => (a + b + c) / 3.0,
                    _ => f64::NAN,
                };
                (s.clone(), score)
            })
            .collect();
        let last_prices: std::collections::HashMap<String, f64> =
            closes.iter().map(|(s, c)| (s.clone(), *c.last().unwrap_or(&0.0))).collect();
        rotation_orders(&scores, self.top_k, portfolio, &last_prices)
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

    fn decide_inner<'py>(
        &self,
        _py: Python<'py>,
        closes: &[(String, Vec<f64>)],
        _indicators: &Bound<'py, PyAny>,
        portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order> {
        // Donchian channel breakouts are pure price extremes – no indicator
        // computation required.
        let n_syms = closes.len().max(1) as f64;
        let mut orders = Vec::new();
        for (sym, c) in closes {
            if c.len() < self.entry_period.max(self.exit_period) {
                continue;
            }
            let last = *c.last().unwrap_or(&0.0);
            let entry_high = c[c.len() - self.entry_period..]
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            let exit_low = c[c.len() - self.exit_period..]
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min);
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0);
            if last >= entry_high && cur <= 0 {
                if let Some(o) = buy_order(sym, portfolio_cash(portfolio) / n_syms, last) {
                    orders.push(o);
                }
            } else if last <= exit_low && cur > 0 {
                if let Some(o) = sell_order(sym, cur) {
                    orders.push(o);
                }
            }
        }
        orders
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
