use std::collections::HashMap;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use crate::backtest::models::order::{new_order_id, Order};
use crate::backtest::models::order_type::OrderType;
use crate::backtest::models::portfolio::Portfolio;
use crate::backtest::sizers::{EqualWeight, FixedNotional, FixedQuantity, Sizer};
use crate::data::models::instrument_type::InstrumentType;

fn extract_close_series_from_python(data: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    if let Ok(col) = data.get_item("close") {
        return col
            .extract::<Vec<f64>>()
            .or_else(|_| col.call_method0("to_numpy")?.extract::<Vec<f64>>());
    }

    let py = data.py();
    let np = py.import("numpy")?;
    let arr = np.call_method1("asarray", (data,))?;
    let ndim: usize = arr.getattr("ndim")?.extract()?;

    match ndim {
        1 => arr.extract::<Vec<f64>>(),
        2 => {
            let shape: Vec<usize> = arr.getattr("shape")?.extract()?;
            if shape.get(1).copied().unwrap_or(0) <= 3 {
                return Err(PyErr::new::<PyValueError, _>(
                    "strategy data array must include a close column at index 3",
                ));
            }

            let kwargs = PyDict::new(py);
            kwargs.set_item("axis", 1)?;
            arr.call_method("take", (3,), Some(&kwargs))?.extract::<Vec<f64>>()
        },
        other => Err(PyErr::new::<PyValueError, _>(format!(
            "strategy data must be 1-D close data or 2-D OHLCV data, got {other}-D"
        ))),
    }
}

fn extract_numeric_series_from_python(data: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    data.extract::<Vec<f64>>()
        .or_else(|_| data.call_method0("to_numpy")?.extract::<Vec<f64>>())
}

/// Extract `(symbol, close_series)` pairs from a Python mapping of symbol -> market data.
pub fn extract_close_from_python(data: &Bound<'_, PyAny>) -> PyResult<Vec<(String, Vec<f64>)>> {
    let per_symbol = data.cast::<PyDict>().map_err(|_| {
        PyErr::new::<PyValueError, _>(
            "strategy data must be a dict[str, np.ndarray | pd.DataFrame | pl.DataFrame]",
        )
    })?;

    let mut out = Vec::with_capacity(per_symbol.len());
    for (symbol, dataset) in per_symbol.iter() {
        out.push((symbol.extract::<String>()?, extract_close_series_from_python(&dataset)?));
    }

    Ok(out)
}

fn extract_indicator_series_from_python(data: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<f64>>> {
    if let Ok(list) = data.cast::<PyList>() {
        let mut out = Vec::with_capacity(list.len());
        for item in list.iter() {
            out.push(extract_numeric_series_from_python(&item)?);
        }
        return Ok(out);
    }

    Ok(vec![extract_numeric_series_from_python(data)?])
}

/// Extract Python indicator payloads into the Rust backing map used by [`IndicatorView`].
pub fn extract_indicators_from_python(
    indicators: Option<&Bound<'_, PyAny>>,
) -> PyResult<HashMap<String, HashMap<String, Vec<Vec<f64>>>>> {
    let Some(indicators) = indicators else {
        return Ok(HashMap::new());
    };

    let outer = indicators.cast::<PyDict>().map_err(|_| {
        PyErr::new::<PyValueError, _>(
            "strategy indicators must be a dict[str, dict[str, np.ndarray | list[np.ndarray]] ]",
        )
    })?;

    let mut out = HashMap::with_capacity(outer.len());
    for (name, per_symbol) in outer.iter() {
        let per_symbol_dict = per_symbol.cast::<PyDict>().map_err(|_| {
            PyErr::new::<PyValueError, _>(
                "strategy indicator payload must map each indicator name to a dict of symbols",
            )
        })?;

        let mut by_symbol = HashMap::with_capacity(per_symbol_dict.len());
        for (symbol, value) in per_symbol_dict.iter() {
            by_symbol.insert(
                symbol.extract::<String>()?,
                extract_indicator_series_from_python(&value)?,
            );
        }

        out.insert(name.extract::<String>()?, by_symbol);
    }

    Ok(out)
}

/// Cheap, pure-Rust indicator-snapshot view passed to strategies on each
/// bar. The engine pre-computes every auto-injected indicator into a
/// `name -> symbol -> Vec<output_series>` map (each output series is
/// dense, indexed by bar). [`IndicatorView::value`] / [`last`] read the
/// value(s) at the current bar without touching Python at all, which is
/// ~100x faster than the previous `arr[-1]` lookup through the GIL.
pub struct IndicatorView<'a> {
    /// Full per-(name, symbol) outputs computed once over the whole timeline.
    pub data: &'a HashMap<String, HashMap<String, Vec<Vec<f64>>>>,

    /// Bar position the strategy currently sees (index into each output series).
    pub bar_index: usize,
}

impl<'a> IndicatorView<'a> {
    pub fn new(
        data: &'a HashMap<String, HashMap<String, Vec<Vec<f64>>>>,
        bar_index: usize,
    ) -> Self {
        Self {
            data,
            bar_index,
        }
    }

    /// Last value(s) of the indicator named `name` for `symbol`. Returns
    /// one `f64` per output series (e.g. 2 for Bollinger Bands' upper /
    /// lower bands; 1 for SMA / RSI / ATR). Returns `None` when the
    /// indicator hasn't been computed for this symbol.
    pub fn last(&self, name: &str, symbol: &str) -> Option<Vec<f64>> {
        let per_sym = self.data.get(name)?;
        let series = per_sym.get(symbol)?;
        let mut out = Vec::with_capacity(series.len());
        for s in series {
            out.push(*s.get(self.bar_index)?);
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

pub fn normalize_builtin_orders_by_instrument_type(
    mut orders: Vec<Order>,
    instrument_types: &HashMap<String, InstrumentType>,
    fallback: InstrumentType,
) -> Vec<Order> {
    orders.retain_mut(|o| {
        if matches!(o.order_type, OrderType::Cancel | OrderType::SettlePosition) {
            return true;
        }
        if !o.quantity.is_finite() || o.quantity == 0.0 {
            return false;
        }
        let instrument_type = instrument_types.get(&o.symbol).copied().unwrap_or(fallback);
        if instrument_type.allows_fractional_quantities() {
            return true;
        }
        let whole_abs = o.quantity.abs().floor();
        if whole_abs <= 0.0 {
            return false;
        }
        o.quantity = whole_abs.copysign(o.quantity);
        true
    });
    orders
}

/// Build a market buy order whose quantity is determined by `sizer`.
///
/// `capital` is the cash / equity amount the sizer should size against
/// (interpretation depends on the concrete sizer — `EqualWeight` divides
/// it by `n_positions`, `FixedNotional` ignores it, etc.). Returns `None`
/// when the sizer would yield a non-positive quantity.
pub fn buy_with_sizer<S: Sizer>(symbol: &str, sizer: &S, capital: f64, price: f64) -> Option<Order> {
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
pub fn buy_equal_weight(symbol: &str, n_positions: usize, capital: f64, price: f64) -> Option<Order> {
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
pub fn portfolio_equity(portfolio: &Portfolio, closes: &[(String, &[f64])]) -> f64 {
    let mut equity = portfolio_cash(portfolio);
    for (sym, c) in closes {
        let qty = portfolio.positions.get(sym.as_str()).copied().unwrap_or(0.0);
        if qty.abs() > 1e-12 {
            let last = *c.last().unwrap_or(&0.0);
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
    use std::collections::HashSet;

    let mut sorted: Vec<&(String, f64)> = scores.iter().filter(|(_, s)| s.is_finite()).collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
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
