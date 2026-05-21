//! Margin & leverage helpers.

use crate::backtest::fx::FxTable;
use crate::backtest::models::ExperimentConfig;
use crate::constants::{Cash, Positions, Symbol, MIN_POSITION, SECS_PER_YEAR};
use crate::data::models::{Bar, Currency};
use std::collections::HashMap;

/// Classification of a limit-check rejection.
///
/// Check whether an order satisfies the configured leverage and position
/// limits. Returns either the (possibly shrunk) acceptable quantity, or
/// an error string describing which limit was breached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitViolation {
    /// Per-symbol concentration limit (`max_position_size`).
    PositionSize,

    /// Leverage / margin / equity constraint.
    Margin,
}

/// Accrue per-bar margin interest and short-borrow cost.
///
/// Both rates are annual percentages. The cost is prorated by
/// `bar_seconds / SECS_PER_YEAR`.
///
/// * `margin_interest` is charged on negative base cash (borrowed funds).
/// * `borrow_rate` is charged on the gross value of open short positions.
///
/// Charges are taken from the base-currency cash bucket, which may go
/// further negative (the next maintenance-margin check will surface that).
pub fn accrue_margin_costs(
    cfg: &ExperimentConfig,
    cash: &mut Cash,
    positions: &Positions,
    aligned: &HashMap<Symbol, Vec<Option<Bar>>>,
    bar_index: usize,
    quote_ccy: &HashMap<&str, &str>,
    base_ccy: Currency,
    fx: &FxTable,
    ts: i64,
    bar_seconds: i64,
) {
    if bar_seconds <= 0 {
        return;
    }

    let base_str = base_ccy.to_string();
    let frac = bar_seconds as f64 / SECS_PER_YEAR;

    // Margin interest on borrowed cash (any negative cash bucket, converted to base).
    if cfg.exchange.margin_interest > 0.0 {
        let mut borrowed_base: f64 = 0.;
        for (ccy, amt) in cash.iter() {
            if *amt < 0.0 {
                borrowed_base -= fx
                    .convert(*amt, &ccy.to_string(), &base_str, ts)
                    .expect(format!("Unable to convert currency {ccy} to {base_ccy}").as_str());
            }
        }

        if borrowed_base > 0.0 {
            let cost = borrowed_base * cfg.exchange.margin_interest / 100. * frac;
            *cash.entry(base_ccy).or_insert(0.0) -= cost;
        }
    }

    // Borrow cost on short positions.
    if cfg.exchange.borrow_rate > 0.0 {
        let mut shorts_value_base: f64 = 0.0;
        for (sym, qty) in positions {
            if *qty < 0.0 {
                if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
                    let v = qty.abs() * b.close;

                    let base_str_ref = base_str.as_str();
                    let ccy = quote_ccy.get(sym.as_str()).unwrap_or(&base_str_ref);
                    shorts_value_base += fx.convert(v, ccy, &base_str, ts).unwrap_or(v);
                }
            }
        }

        if shorts_value_base > 0.0 {
            let cost = shorts_value_base * cfg.exchange.borrow_rate / 100. * frac;
            *cash.entry(base_ccy).or_insert(0.0) -= cost;
        }
    }
}

/// Validate an order against the configured position-size and leverage limits.
///
/// This function either:
///
/// * Returns `Ok(accepted_qty)`, where the accepted quantity may be smaller than
///   the requested `qty` when the order would push past a cap.
/// * Returns `Err((violation, reason))` when the existing exposure already
///   exhausts the relevant cap and there is no room at all for the order.
pub fn check_order_against_limits(
    cfg: &ExperimentConfig,
    symbol: &str,
    qty: f64,
    fill_px: f64,
    order_ccy: &str,
    base_ccy: &str,
    equity_base: f64,
    gross_base: f64,
    current_qty: f64,
    current_pos_base: f64,
    fx: &FxTable,
    ts: i64,
) -> Result<f64, (LimitViolation, String)> {
    let abs_qty = qty.abs();
    if abs_qty <= 0.0 || !abs_qty.is_finite() || fill_px <= 0.0 || !fill_px.is_finite() {
        return Ok(qty);
    }
    
    let order_notional_base =
        fx.convert(abs_qty * fill_px, order_ccy, base_ccy, ts).unwrap_or(abs_qty * fill_px);

    let unit_base = order_notional_base / abs_qty;
    if unit_base <= 0.0 || !unit_base.is_finite() {
        return Ok(qty);
    }

    let current_pos_base = current_pos_base.max(0.0);
    let current_abs_qty = current_qty.abs();

    let max_qty_for_final_exposure = |cap_base: f64| -> f64 {
        // Check if they move in the same direction.
        if current_abs_qty <= MIN_POSITION || current_qty.signum() == qty.signum() {
            return ((cap_base - current_pos_base) / unit_base).max(0.0);
        }

        // Opposite-side orders reduce existing exposure first. Always allow
        // the requested size when it only closes/reduces the position, even
        // if the account is currently at/over a cap. If it flips the position,
        // only the post-flip exposure consumes cap room.
        if abs_qty <= current_abs_qty + MIN_POSITION {
            abs_qty
        } else {
            current_abs_qty + (cap_base / unit_base).max(0.0)
        }
    };

    let mut max_abs_qty = abs_qty;

    // Per-symbol exposure (existing notional + new order notional) must not
    // exceed `max_position_size / 100` of equity. When the order would push
    // past the cap, the quantity is shrunk to whatever fits rather than rejected
    // outright. This matches real-broker behavior and prevents an entire
    // equal-weight allocation from being silently dropped.
    let pos_cap_pct = cfg.exchange.max_position_size as f64;
    if pos_cap_pct > 0.0 && equity_base > 0.0 {
        let max_per_pos = equity_base * pos_cap_pct / 100.0;
        let allowed_abs_qty = max_qty_for_final_exposure(max_per_pos);
        if allowed_abs_qty <= MIN_POSITION {
            return Err((
                LimitViolation::PositionSize,
                format!(
                "order would exceed max_position_size ({pos_cap_pct}% of equity) for {symbol}: \
                 position already at limit (current {current_pos_base:.2}, cap {max_per_pos:.2})"
            ),
            ));
        }

        max_abs_qty = max_abs_qty.min(allowed_abs_qty);
    }

    // The total gross notional after this fill (including existing exposure)
    // must not exceed `equity * effective_leverage_cap`.
    let cap = effective_leverage_cap(cfg);
    if equity_base > 0.0 && cap.is_finite() {
        let max_gross = equity_base * cap;
        let other_gross_base = (gross_base - current_pos_base).max(0.0);
        let symbol_cap_base = max_gross - other_gross_base;
        let allowed_abs_qty = max_qty_for_final_exposure(symbol_cap_base);
        if allowed_abs_qty <= MIN_POSITION {
            return Err((
                LimitViolation::Margin,
                format!(
                    "order would exceed max_leverage ({cap:.2}x): gross notional \
                    at limit (current {gross_base:.2}, cap {max_gross:.2})"
                ),
            ));
        }

        max_abs_qty = max_abs_qty.min(allowed_abs_qty);
    } else if equity_base <= 0.0 {
        // Account already wiped out — nothing more can be opened.
        return Err((
            LimitViolation::Margin,
            "equity is non-positive; cannot open new exposure".to_owned(),
        ));
    }

    if !max_abs_qty.is_finite() || max_abs_qty <= MIN_POSITION {
        return Err((
            LimitViolation::Margin,
            format!("no headroom under leverage / position-size limits for {symbol}"),
        ));
    }

    Ok(qty.signum() * max_abs_qty.min(abs_qty))
}

/// Effective leverage cap given the `allow_margin`, `max_leverage` and
/// `initial_margin` settings. When margin is disabled, the cap is 1.0
/// (no borrowing). Otherwise `max_leverage` and `100/initial_margin`
/// are intersected so the more restrictive of the two wins.
pub fn effective_leverage_cap(cfg: &ExperimentConfig) -> f64 {
    if !cfg.exchange.allow_margin {
        return 1.0;
    }
    let im = cfg.exchange.initial_margin;
    let from_im = if im > 0.0 {
        100.0 / im
    } else {
        f64::INFINITY
    };
    let from_ml = if cfg.exchange.max_leverage > 0.0 {
        cfg.exchange.max_leverage
    } else {
        f64::INFINITY
    };
    from_ml.min(from_im).max(1.0)
}

pub fn limit_warning_dedupe_key(symbol: &str, violation: LimitViolation, reason: &str) -> String {
    let bucket = match violation {
        LimitViolation::Margin if reason.contains("gross notional already at limit") => {
            "margin:gross_notional_at_limit"
        },
        LimitViolation::Margin if reason.contains("equity is non-positive") => {
            "margin:equity_non_positive"
        },
        LimitViolation::Margin
            if reason.contains("no headroom under leverage / position-size limits") =>
        {
            "margin:no_headroom"
        },
        LimitViolation::PositionSize if reason.contains("position already at limit") => {
            "position_size:at_limit"
        },
        LimitViolation::PositionSize
            if reason.contains("no headroom under leverage / position-size limits") =>
        {
            "position_size:no_headroom"
        },
        _ => reason,
    };
    format!("{symbol}\0{bucket}")
}

/// Post-bar maintenance-margin check.
///
/// Returns `Some(message)` when `equity / gross_notional < maintenance_margin/100`
/// (i.e. the account is undercollateralised), `None` otherwise. The caller
/// decides whether to force-liquidate, record a warning, or abort the run
/// based on `cfg.exchange.raise_on_margin_limit`.
pub fn check_maintenance_margin(
    cfg: &ExperimentConfig,
    equity_base: f64,
    gross_base: f64,
) -> Option<String> {
    let mm = cfg.exchange.maintenance_margin;
    if mm <= 0.0 || gross_base <= 0.0 {
        return None;
    }
    // Negative equity is always a margin call.
    if equity_base <= 0.0 {
        return Some(format!(
            "margin call: equity {equity_base:.2} ≤ 0 with gross notional {gross_base:.2}"
        ));
    }
    let ratio = equity_base / gross_base;
    if ratio < mm / 100.0 {
        Some(format!(
            "margin call: equity/notional ratio {:.2}% below maintenance_margin {mm:.2}%",
            ratio * 100.0
        ))
    } else {
        None
    }
}
