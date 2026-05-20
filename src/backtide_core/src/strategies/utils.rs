use crate::backtest::models::{new_order_id, Order, OrderType, Portfolio};
use crate::sizers::{EqualWeight, FixedNotional, FixedQuantity, Sizer};
use crate::config::interface::Config;
use crate::constants::Symbol;
use crate::data::models::Bar;
use crate::errors::{EngineError, EngineResult};
use crate::utils::python::{extract_2d_from_python, extract_bars_from_python, load_pickle};
use itertools::Itertools;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// Load the selected strategies.
///
/// Returns `(name, obj, is_custom)` triples. Names present in `overrides` use
/// the supplied in-memory instance directly. Other names are resolved from the
/// local strategies' directory.
pub fn load_strategies(
    names: &[String],
    overrides: &HashMap<String, Py<PyAny>>,
) -> EngineResult<Vec<(String, Py<PyAny>, bool)>> {
    let cfg = Config::get()?;

    Python::attach(|py| -> PyResult<_> {
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let obj = if let Some(o) = overrides.get(name) {
                o.clone_ref(py)
            } else {
                // Resolve a strategy name to a concrete Python object.
                let path = cfg.data.storage_path.join("strategies").join(format!("{name}.pkl"));
                load_pickle(py, &path)?
            };

            // Determine whether it's a built-in/custom strategy by ther class' module
            let cls = obj.bind(py).get_type();
            let module: String = cls.getattr("__module__")?.extract()?;
            let is_custom = !module.starts_with("backtide.");

            out.push((name.clone(), obj, is_custom));
        }

        Ok(out)
    })
    .map_err(|e: PyErr| EngineError::Io(std::io::Error::other(e.to_string())))
}

// ─────────────────────────────────────────────────────────────────────────────
// Interface utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Extract `(symbol, bars)` pairs from a Python mapping of symbol -> data.
pub fn extract_strategy_data_from_python(
    data: &Bound<'_, PyAny>,
) -> PyResult<Vec<(String, Vec<Bar>)>> {
    let per_symbol =
        data.cast::<PyDict>().map_err(|_| PyValueError::new_err("strategy data must be a dict"))?;

    let mut out = Vec::with_capacity(per_symbol.len());
    for (symbol, data) in per_symbol.iter() {
        out.push((symbol.extract::<String>()?, extract_bars_from_python(&data)?));
    }

    Ok(out)
}

/// Extract Python indicator payloads into the Rust backing map used by [`IndicatorView`].
pub fn extract_indicator_data_from_python(
    indicators: Option<&Bound<'_, PyAny>>,
) -> PyResult<HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>>> {
    let Some(indicators) = indicators else {
        return Ok(HashMap::new());
    };

    let outer = indicators
        .cast::<PyDict>()
        .map_err(|_| PyValueError::new_err("strategy indicators must be a dict"))?;

    let mut out = HashMap::with_capacity(outer.len());
    for (name, per_symbol) in outer.iter() {
        let per_symbol_dict = per_symbol.cast::<PyDict>().map_err(|_| {
            PyValueError::new_err(
                "strategy indicator payload must map each indicator name to a dict of symbols",
            )
        })?;

        let mut by_symbol = HashMap::with_capacity(per_symbol_dict.len());
        for (symbol, value) in per_symbol_dict.iter() {
            by_symbol.insert(symbol.extract::<String>()?, extract_2d_from_python(&value)?);
        }

        out.insert(name.extract::<String>()?, by_symbol);
    }

    Ok(out)
}

/// Cheap, pure-Rust indicator-snapshot view passed to strategies on each
/// bar. The engine pre-computes every auto-injected indicator into a
/// `name -> symbol -> Vec<series>` map.
pub struct IndicatorView<'a> {
    /// Full (name, symbol) computed values over the whole timeline.
    pub data: &'a HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>>,

    /// Bar position the strategy currently sees (index into each output series).
    pub bar_index: u64,
}

impl<'a> IndicatorView<'a> {
    pub fn new(data: &'a HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>>, bar_index: u64) -> Self {
        Self {
            data,
            bar_index,
        }
    }

    /// Last value of the indicator named `name` for `symbol`. Returns one
    /// `f64` per output series (e.g., 3 for Bollinger Bands). Returns `None`
    /// when the indicator hasn't been computed for this symbol.
    pub fn last(&self, name: &str, symbol: &str) -> Option<Vec<f64>> {
        let per_sym = self.data.get(name)?;
        let series = per_sym.get(symbol)?;

        let mut out = Vec::with_capacity(series.len());
        for s in series {
            out.push(*s.get(self.bar_index as usize)?);
        }

        (!out.is_empty()).then_some(out)
    }

    /// Convenience wrapper for single-output indicators. Returns `None`
    /// when the indicator is missing or its current value is non-finite.
    pub fn value(&self, name: &str, symbol: &str) -> Option<f64> {
        self.last(name, symbol).and_then(|v| v.into_iter().next()).filter(|x| x.is_finite())
    }
}

/// Format a value for use in auto-indicator names.
#[inline]
pub fn fmt_arg<T: std::fmt::Debug>(v: T) -> String {
    format!("{:?}", v)
}

/// Build a market buy order whose quantity is determined by `sizer`.
///
/// `capital` is the cash / equity amount the sizer should size against
/// (interpretation depends on the concrete sizer — `EqualWeight` divides
/// it by `n_positions`, `FixedNotional` ignores it, etc.). Returns `None`
/// when the sizer would yield a non-positive quantity.
pub fn buy_with_sizer<S: Sizer>(
    symbol: &str,
    sizer: &S,
    capital: f64,
    price: f64,
) -> Option<Order> {
    if price <= 0.0 {
        return None;
    }
    let qty = sizer.calculate(capital, price, None, None).ok()?;
    if qty <= 0.0 {
        return None;
    }
    Some(Order {
        id: new_order_id(),
        symbol: symbol.to_owned(),
        order_type: OrderType::Market,
        quantity: qty,
        price: None,
        limit_price: None,
        sizer: None,
    })
}

/// Build a market buy order sized to spend `target_cash` (uses the
/// [`FixedNotional`] sizer under the hood). Strategies size their orders
/// against `portfolio.cash`, but the actual fill happens on the *next*
/// bar with slippage and commission applied — so the cost frequently
/// exceeds the requested `target_cash` by a small margin. The engine
/// handles that by auto-shrinking the qty at fill time so equal-weight
/// allocations like `cash / n_symbols` don't lose their last leg to a
/// fractional overshoot.
pub fn buy_order(symbol: &str, target_cash: f64, price: f64) -> Option<Order> {
    if target_cash <= 0.0 {
        return None;
    }
    buy_with_sizer(symbol, &FixedNotional::new(target_cash), 0.0, price)
}

/// Build a market buy order that allocates `capital / n_positions` worth
/// of `price` to one slot of an equal-weight portfolio. Uses the
/// [`EqualWeight`] sizer under the hood.
pub fn buy_equal_weight(
    symbol: &str,
    n_positions: usize,
    capital: f64,
    price: f64,
) -> Option<Order> {
    if n_positions == 0 || capital <= 0.0 {
        return None;
    }
    buy_with_sizer(symbol, &EqualWeight::new(n_positions as u32), capital, price)
}

/// Build a market sell order to flatten an existing long position. Uses
/// the [`FixedQuantity`] sizer under the hood (negated).
pub fn sell_order(symbol: &str, quantity: f64) -> Option<Order> {
    if quantity <= 0.0 {
        return None;
    }
    let sizer = FixedQuantity::new(quantity);
    let qty = sizer.calculate(0.0, 1.0, None, None).ok()?;
    if qty <= 0.0 {
        return None;
    }
    Some(Order {
        id: new_order_id(),
        symbol: symbol.to_owned(),
        order_type: OrderType::Market,
        quantity: -qty,
        price: None,
        limit_price: None,
        sizer: None,
    })
}

/// Estimate cash available in the portfolio (sum of all currency balances).
pub fn portfolio_cash(portfolio: &Portfolio) -> f64 {
    portfolio.cash.values().sum()
}

/// Total portfolio equity: cash + positions marked to their latest close.
pub fn portfolio_equity(portfolio: &Portfolio, bars: &[(String, Vec<Bar>)]) -> f64 {
    let mut equity = portfolio_cash(portfolio);
    for (sym, b) in bars {
        let qty = portfolio.positions.get(sym.as_str()).copied().unwrap_or(0.0);
        if qty.abs() > 1e-12 {
            let last = b.last().map(|bar| bar.close).unwrap_or(0.0);
            equity += qty * last;
        }
    }
    equity
}

/// Generic single-asset signal: place a buy when `signal == true` and
/// the position is flat, place a sell when `signal == false` and we are
/// long. `target_notional` is the desired position size (in cash terms),
/// capped at available cash to avoid over-allocation.
pub fn react_to_signal(
    symbol: &str,
    signal_long: bool,
    last_price: f64,
    portfolio: &Portfolio,
    target_notional: f64,
) -> Vec<Order> {
    let cur = portfolio.positions.get(symbol).copied().unwrap_or(0.0);
    if signal_long && cur <= 0.0 {
        let cash = portfolio_cash(portfolio);
        let alloc = target_notional.min(cash);
        if let Some(o) = buy_order(symbol, alloc, last_price) {
            return vec![o];
        }
    } else if !signal_long && cur > 0.0 {
        if let Some(o) = sell_order(symbol, cur) {
            return vec![o];
        }
    }
    Vec::new()
}

/// Linear regression slope of `series` against the index ``0..len``.
/// Returns `(slope, mean_y)`. Returns `None` when the input is empty
/// or has zero variance on the x axis.
pub fn linreg_slope(series: &[f64]) -> Option<(f64, f64)> {
    let m = series.len() as f64;
    if m < 2.0 {
        return None;
    }
    let mean_x = (m - 1.0) / 2.0;
    let mean_y = series.iter().sum::<f64>() / m;
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, &y) in series.iter().enumerate() {
        let dx = i as f64 - mean_x;
        num += dx * (y - mean_y);
        den += dx * dx;
    }
    if den == 0.0 {
        return None;
    }
    Some((num / den, mean_y))
}

/// Sample standard deviation of `series`. Returns `None` for short inputs.
#[allow(dead_code)]
pub fn stddev(series: &[f64]) -> Option<f64> {
    if series.len() < 2 {
        return None;
    }
    let m = series.len() as f64;
    let mean = series.iter().sum::<f64>() / m;
    let var = series.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (m - 1.0);
    Some(var.sqrt())
}

/// Sample standard deviation of the bar-to-bar returns implied by
/// `prices`. Returns `None` when fewer than two valid returns can be
/// derived (e.g. shorter than 3 bars, all-NaN, or every base price ≤ 0).
/// Used as a lightweight realised-volatility estimate by strategies
/// that adapt their behaviour to the current regime.
pub fn window_return_std(prices: &[f64]) -> Option<f64> {
    let returns: Vec<f64> = prices
        .windows(2)
        .filter_map(|w| {
            if w[0].is_finite() && w[1].is_finite() && w[0] > 0.0 {
                Some(w[1] / w[0] - 1.0)
            } else {
                None
            }
        })
        .collect();
    stddev(&returns)
}

/// Top-K rotation across symbols. Closes positions not in the top, then
/// buys equal-weight into the top `k`.
pub fn rotation_orders(
    scores: &[(String, f64)],
    top_k: usize,
    portfolio: &Portfolio,
    last_prices: &HashMap<String, f64>,
) -> Vec<Order> {
    let sorted: Vec<&(String, f64)> = scores
        .iter()
        .filter(|(_, s)| s.is_finite())
        .sorted_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal))
        .collect();

    let target: HashSet<String> = sorted.iter().take(top_k).map(|(s, _)| s.clone()).collect();

    let mut orders: Vec<Order> = Vec::new();

    // Close positions not in target.
    for (sym, qty) in &portfolio.positions {
        if *qty > 0.0 && !target.contains(sym) {
            if let Some(o) = sell_order(sym, *qty) {
                orders.push(o);
            }
        }
    }

    // Open new positions equal-weight.
    if !target.is_empty() {
        let cash = portfolio_cash(portfolio);
        let n = target.len();
        for sym in &target {
            let cur = portfolio.positions.get(sym).copied().unwrap_or(0.0);
            if cur > 0.0 {
                continue;
            }
            if let Some(px) = last_prices.get(sym).copied() {
                if let Some(o) = buy_equal_weight(sym, n, cash, px) {
                    orders.push(o);
                }
            }
        }
    }

    orders
}
