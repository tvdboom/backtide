//! Margin & leverage helpers.

use crate::backtest::fx::FxTable;
use crate::backtest::models::ExperimentConfig;
use crate::backtest::utils::is_negligible;
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
                    .unwrap_or_else(|| panic!("Unable to convert currency {ccy} to {base_ccy}"));
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
        if is_negligible(allowed_abs_qty) {
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
        if is_negligible(allowed_abs_qty) {
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

    if !max_abs_qty.is_finite() || is_negligible(max_abs_qty) {
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

/// Post-bar maintenance-margin check.
///
/// Returns `Some(message)` when the account is undercollateralized, `None`
/// otherwise. The caller decides whether to force-liquidate, record a warning,
/// or abort the run.
pub fn check_maintenance_margin(margin: f64, equity_base: f64, gross_base: f64) -> Option<String> {
    if margin <= 0.0 || gross_base <= 0.0 {
        return None;
    }

    // Negative equity is always a margin call.
    if equity_base <= 0.0 {
        return Some(format!(
            "margin call: equity {equity_base:.2} ≤ 0 with gross notional {gross_base:.2}"
        ));
    }

    let ratio = equity_base / gross_base;
    if ratio < margin / 100.0 {
        Some(format!(
            "margin call: equity/notional ratio {:.2}% below maintenance_margin {margin:.2}%",
            ratio * 100.0
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::models::ExchangeExpConfig;
    use crate::data::models::Currency;

    /// Helper to create a minimal ExperimentConfig with custom exchange settings.
    fn cfg_with_exchange(exchange: ExchangeExpConfig) -> ExperimentConfig {
        ExperimentConfig {
            general: Default::default(),
            data: Default::default(),
            portfolio: Default::default(),
            strategy: Default::default(),
            indicators: Default::default(),
            exchange,
            engine: Default::default(),
        }
    }

    fn default_cfg() -> ExperimentConfig {
        cfg_with_exchange(ExchangeExpConfig::default())
    }

    // ── effective_leverage_cap ────────────────────────────────────────────

    #[test]
    fn leverage_cap_margin_disabled() {
        let cfg = default_cfg();
        assert!(!cfg.exchange.allow_margin);
        assert_eq!(effective_leverage_cap(&cfg), 1.0);
    }

    #[test]
    fn leverage_cap_margin_enabled_default() {
        let exchange = ExchangeExpConfig {
            allow_margin: true,
            ..Default::default()
        };
        // default: max_leverage=2.0, initial_margin=50.0 → 100/50=2.0
        // min(2.0, 2.0).max(1.0) = 2.0
        let cfg = cfg_with_exchange(exchange);
        assert_eq!(effective_leverage_cap(&cfg), 2.0);
    }

    #[test]
    fn leverage_cap_initial_margin_more_restrictive() {
        let exchange = ExchangeExpConfig {
            allow_margin: true,
            max_leverage: 10.0,
            initial_margin: 50.0, // 100/50 = 2.0, more restrictive than 10x
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        assert_eq!(effective_leverage_cap(&cfg), 2.0);
    }

    #[test]
    fn leverage_cap_max_leverage_more_restrictive() {
        let exchange = ExchangeExpConfig {
            allow_margin: true,
            max_leverage: 3.0,
            initial_margin: 10.0, // 100/10 = 10.0
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        assert_eq!(effective_leverage_cap(&cfg), 3.0);
    }

    #[test]
    fn leverage_cap_zero_initial_margin_gives_infinity_from_im() {
        let exchange = ExchangeExpConfig {
            allow_margin: true,
            max_leverage: 5.0,
            initial_margin: 0.0,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        assert_eq!(effective_leverage_cap(&cfg), 5.0);
    }

    #[test]
    fn leverage_cap_zero_max_leverage_gives_infinity_from_ml() {
        let exchange = ExchangeExpConfig {
            allow_margin: true,
            max_leverage: 0.0,
            initial_margin: 25.0, // 100/25 = 4.0
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        assert_eq!(effective_leverage_cap(&cfg), 4.0);
    }

    #[test]
    fn leverage_cap_both_zero_gives_infinity() {
        let exchange = ExchangeExpConfig {
            allow_margin: true,
            max_leverage: 0.0,
            initial_margin: 0.0,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        assert!(effective_leverage_cap(&cfg).is_infinite());
    }

    #[test]
    fn leverage_cap_never_below_one() {
        let exchange = ExchangeExpConfig {
            allow_margin: true,
            max_leverage: 0.5,     // below 1
            initial_margin: 200.0, // 100/200 = 0.5
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        assert_eq!(effective_leverage_cap(&cfg), 1.0);
    }

    // ── check_maintenance_margin ─────────────────────────────────────────

    #[test]
    fn maintenance_margin_zero_margin_always_ok() {
        assert!(check_maintenance_margin(0.0, 1000.0, 5000.0).is_none());
    }

    #[test]
    fn maintenance_margin_zero_gross_always_ok() {
        assert!(check_maintenance_margin(25.0, 1000.0, 0.0).is_none());
    }

    #[test]
    fn maintenance_margin_negative_gross_always_ok() {
        assert!(check_maintenance_margin(25.0, 1000.0, -100.0).is_none());
    }

    #[test]
    fn maintenance_margin_negative_equity_margin_call() {
        let msg = check_maintenance_margin(25.0, -100.0, 5000.0).unwrap();
        assert!(msg.contains("margin call"));
        assert!(msg.contains("≤ 0"));
    }

    #[test]
    fn maintenance_margin_zero_equity_margin_call() {
        let msg = check_maintenance_margin(25.0, 0.0, 5000.0).unwrap();
        assert!(msg.contains("margin call"));
    }

    #[test]
    fn maintenance_margin_below_threshold() {
        // equity/gross = 1000/5000 = 20%, margin = 25% → margin call.
        let msg = check_maintenance_margin(25.0, 1000.0, 5000.0).unwrap();
        assert!(msg.contains("margin call"));
        assert!(msg.contains("20.00%"));
    }

    #[test]
    fn maintenance_margin_at_threshold_is_ok() {
        // equity/gross = 2500/10000 = 25%, margin = 25% → no call (not strictly below).
        assert!(check_maintenance_margin(25.0, 2500.0, 10000.0).is_none());
    }

    #[test]
    fn maintenance_margin_above_threshold_is_ok() {
        // equity/gross = 3000/10000 = 30%, margin = 25% → ok.
        assert!(check_maintenance_margin(25.0, 3000.0, 10000.0).is_none());
    }

    #[test]
    fn maintenance_margin_barely_below() {
        // equity/gross = 2499/10000 = 24.99%, margin = 25% → margin call.
        assert!(check_maintenance_margin(25.0, 2499.0, 10000.0).is_some());
    }

    // ── check_order_against_limits ───────────────────────────────────────

    #[test]
    fn limits_zero_qty_passes_through() {
        let cfg = default_cfg();
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", 0.0, 100.0, "USD", "USD", 10000.0, 0.0, 0.0, 0.0, &fx, 0,
        );
        assert_eq!(result.unwrap(), 0.0);
    }

    #[test]
    fn limits_nan_qty_passes_through() {
        let cfg = default_cfg();
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg,
            "AAPL",
            f64::NAN,
            100.0,
            "USD",
            "USD",
            10000.0,
            0.0,
            0.0,
            0.0,
            &fx,
            0,
        );
        assert!(result.unwrap().is_nan());
    }

    #[test]
    fn limits_zero_fill_price_passes_through() {
        let cfg = default_cfg();
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", 10.0, 0.0, "USD", "USD", 10000.0, 0.0, 0.0, 0.0, &fx, 0,
        );
        assert_eq!(result.unwrap(), 10.0);
    }

    #[test]
    fn limits_position_size_cap_default_100_pct() {
        // Default max_position_size=100 → cap = equity * 100% = 10000
        // Buying 10 shares at $100 = $1000 notional, well under cap.
        let cfg = default_cfg();
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", 10.0, 100.0, "USD", "USD", 10000.0, 0.0, 0.0, 0.0, &fx, 0,
        );
        assert_eq!(result.unwrap(), 10.0);
    }

    #[test]
    fn limits_position_size_shrinks_order() {
        // max_position_size=10 → cap = 10000 * 10% = 1000.
        // Order: 20 shares at $100 = $2000. Shrunk to 10 shares.
        let exchange = ExchangeExpConfig {
            max_position_size: 10,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", 20.0, 100.0, "USD", "USD", 10000.0, 0.0, 0.0, 0.0, &fx, 0,
        );
        assert_eq!(result.unwrap(), 10.0);
    }

    #[test]
    fn limits_position_size_rejects_when_at_cap() {
        // max_position_size=10 → cap = 10000 * 10% = 1000.
        // Current position already at 1000. New order rejected.
        let exchange = ExchangeExpConfig {
            max_position_size: 10,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", 5.0, 100.0, "USD", "USD", 10000.0, 0.0, 10.0, 1000.0, &fx, 0,
        );
        assert!(result.is_err());
        let (violation, _reason) = result.unwrap_err();
        assert_eq!(violation, LimitViolation::PositionSize);
    }

    #[test]
    fn limits_leverage_cap_rejects_when_exhausted() {
        // allow_margin=true, max_leverage=2.0. equity=10000, max_gross=20000.
        // Current gross=20000, new order → rejected.
        let exchange = ExchangeExpConfig {
            allow_margin: true,
            max_leverage: 2.0,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", 10.0, 100.0, "USD", "USD", 10000.0, 20000.0, 0.0, 0.0, &fx, 0,
        );
        assert!(result.is_err());
        let (violation, _) = result.unwrap_err();
        assert_eq!(violation, LimitViolation::Margin);
    }

    #[test]
    fn limits_leverage_cap_shrinks_order() {
        // allow_margin=true, max_leverage=2.0. equity=10000, max_gross=20000.
        // Current gross=15000, order 100 shares at $100 = $10000.
        // Only 5000 headroom → shrunk to 50 shares.
        let exchange = ExchangeExpConfig {
            allow_margin: true,
            max_leverage: 2.0,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", 100.0, 100.0, "USD", "USD", 10000.0, 15000.0, 0.0, 0.0, &fx, 0,
        );
        assert_eq!(result.unwrap(), 50.0);
    }

    #[test]
    fn limits_no_margin_cap_at_1x() {
        // allow_margin=false → cap = 1.0. equity=10000, max_gross=10000.
        // Order: 200 shares at $100 = $20000 → shrunk to 100 shares.
        let cfg = default_cfg();
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", 200.0, 100.0, "USD", "USD", 10000.0, 0.0, 0.0, 0.0, &fx, 0,
        );
        assert_eq!(result.unwrap(), 100.0);
    }

    #[test]
    fn limits_negative_equity_rejects() {
        let cfg = default_cfg();
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", 10.0, 100.0, "USD", "USD", -100.0, 0.0, 0.0, 0.0, &fx, 0,
        );
        assert!(result.is_err());
        let (violation, reason) = result.unwrap_err();
        assert_eq!(violation, LimitViolation::Margin);
        assert!(reason.contains("non-positive"));
    }

    #[test]
    fn limits_sell_order_preserves_sign() {
        let cfg = default_cfg();
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", -10.0, 100.0, "USD", "USD", 10000.0, 0.0, 0.0, 0.0, &fx, 0,
        );
        let qty = result.unwrap();
        assert!(qty < 0.0);
        assert_eq!(qty.abs(), 10.0);
    }

    #[test]
    fn limits_opposite_side_order_reduces_position() {
        // Current long 50 shares. Selling 30 → allowed even at cap.
        let exchange = ExchangeExpConfig {
            max_position_size: 10, // cap = 10000 * 10% = 1000
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let fx = FxTable::new("USD");
        let result = check_order_against_limits(
            &cfg, "AAPL", -30.0, 100.0, "USD", "USD", 10000.0, 5000.0, 50.0, 5000.0, &fx, 0,
        );
        // Reducing position: allowed in full (abs_qty=30 <= current_abs_qty=50)
        assert_eq!(result.unwrap(), -30.0);
    }

    #[test]
    fn limits_fx_conversion() {
        // Order in EUR, base is USD. EUR/USD = 1.10.
        let exchange = ExchangeExpConfig {
            max_position_size: 10, // cap = 10000 * 10% = 1000 USD
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let mut fx = FxTable::new("USD");
        fx.add_series("EUR", "USD", vec![(0, 1.10)]);
        // 10 shares at €100 = €1000 = $1100. Cap $1000 → shrunk.
        let result = check_order_against_limits(
            &cfg, "AAPL", 10.0, 100.0, "EUR", "USD", 10000.0, 0.0, 0.0, 0.0, &fx, 0,
        );
        let qty = result.unwrap();
        // $1000 / $110-per-share = ~9.09 shares
        assert!(qty < 10.0);
        assert!(qty > 9.0);
    }

    // ── accrue_margin_costs ──────────────────────────────────────────────

    #[test]
    fn accrue_no_costs_when_rates_zero() {
        let cfg = default_cfg(); // margin_interest=0, borrow_rate=0
        let mut cash: Cash = HashMap::new();
        cash.insert(Currency::USD, 10000.0);
        let positions: Positions = HashMap::new();
        let aligned: HashMap<Symbol, Vec<Option<Bar>>> = HashMap::new();
        let quote_ccy: HashMap<&str, &str> = HashMap::new();
        let fx = FxTable::new("USD");

        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            86400,
        );
        assert_eq!(*cash.get(&Currency::USD).unwrap(), 10000.0);
    }

    #[test]
    fn accrue_no_costs_when_bar_seconds_zero() {
        let exchange = ExchangeExpConfig {
            margin_interest: 5.0,
            borrow_rate: 3.0,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let mut cash: Cash = HashMap::new();
        cash.insert(Currency::USD, -5000.0);
        let positions: Positions = HashMap::new();
        let aligned: HashMap<Symbol, Vec<Option<Bar>>> = HashMap::new();
        let quote_ccy: HashMap<&str, &str> = HashMap::new();
        let fx = FxTable::new("USD");

        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            0,
        );
        assert_eq!(*cash.get(&Currency::USD).unwrap(), -5000.0);
    }

    #[test]
    fn accrue_margin_interest_on_borrowed_cash() {
        let exchange = ExchangeExpConfig {
            margin_interest: 10.0, // 10% annual
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let mut cash: Cash = HashMap::new();
        cash.insert(Currency::USD, -10000.0); // borrowed 10000
        let positions: Positions = HashMap::new();
        let aligned: HashMap<Symbol, Vec<Option<Bar>>> = HashMap::new();
        let quote_ccy: HashMap<&str, &str> = HashMap::new();
        let fx = FxTable::new("USD");
        let bar_seconds = 86400; // 1 day

        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            bar_seconds,
        );

        let expected_cost = 10000.0 * 10.0 / 100.0 * (86400.0 / SECS_PER_YEAR);
        let cash_after = *cash.get(&Currency::USD).unwrap();
        // Cash should be more negative by the cost amount
        assert!((cash_after - (-10000.0 - expected_cost)).abs() < 1e-6);
    }

    #[test]
    fn accrue_borrow_cost_on_short_positions() {
        let exchange = ExchangeExpConfig {
            borrow_rate: 5.0, // 5% annual
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);

        let mut cash: Cash = HashMap::new();
        cash.insert(Currency::USD, 10000.0);

        let mut positions: Positions = HashMap::new();
        positions.insert("AAPL".to_owned(), -100.0); // Short 100 shares

        let test_bar = Bar {
            open_ts: 0,
            close_ts: 86400,
            open_ts_exchange: 0,
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 152.0,
            adj_close: 152.0,
            volume: 1000.0,
            n_trades: None,
        };
        let mut aligned: HashMap<Symbol, Vec<Option<Bar>>> = HashMap::new();
        aligned.insert("AAPL".to_owned(), vec![Some(test_bar)]);

        let mut quote_ccy: HashMap<&str, &str> = HashMap::new();
        quote_ccy.insert("AAPL", "USD");

        let fx = FxTable::new("USD");
        let bar_seconds = 86400;

        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            bar_seconds,
        );

        // Short value = 100 * 152 = 15200
        let expected_cost = 15200.0 * 5.0 / 100.0 * (86400.0 / SECS_PER_YEAR);
        let cash_after = *cash.get(&Currency::USD).unwrap();
        assert!((cash_after - (10000.0 - expected_cost)).abs() < 1e-6);
    }

    #[test]
    fn accrue_positive_cash_no_margin_interest() {
        let exchange = ExchangeExpConfig {
            margin_interest: 10.0,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let mut cash: Cash = HashMap::new();
        cash.insert(Currency::USD, 10000.0); // positive cash
        let positions: Positions = HashMap::new();
        let aligned: HashMap<Symbol, Vec<Option<Bar>>> = HashMap::new();
        let quote_ccy: HashMap<&str, &str> = HashMap::new();
        let fx = FxTable::new("USD");

        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            86400,
        );
        // No borrowed cash → no interest charged
        assert_eq!(*cash.get(&Currency::USD).unwrap(), 10000.0);
    }

    #[test]
    fn accrue_long_positions_no_borrow_cost() {
        let exchange = ExchangeExpConfig {
            borrow_rate: 5.0,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);

        let mut cash: Cash = HashMap::new();
        cash.insert(Currency::USD, 10000.0);

        let mut positions: Positions = HashMap::new();
        positions.insert("AAPL".to_owned(), 100.0); // Long, not short

        let test_bar = Bar {
            open_ts: 0,
            close_ts: 86400,
            open_ts_exchange: 0,
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 152.0,
            adj_close: 152.0,
            volume: 1000.0,
            n_trades: None,
        };
        let mut aligned: HashMap<Symbol, Vec<Option<Bar>>> = HashMap::new();
        aligned.insert("AAPL".to_owned(), vec![Some(test_bar)]);

        let mut quote_ccy: HashMap<&str, &str> = HashMap::new();
        quote_ccy.insert("AAPL", "USD");
        let fx = FxTable::new("USD");

        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            86400,
        );
        assert_eq!(*cash.get(&Currency::USD).unwrap(), 10000.0);
    }

    #[test]
    fn accrue_negative_bar_seconds_returns_early() {
        let exchange = ExchangeExpConfig {
            margin_interest: 10.0,
            ..Default::default()
        };
        let cfg = cfg_with_exchange(exchange);
        let mut cash: Cash = HashMap::new();
        cash.insert(Currency::USD, -5000.0);
        let positions: Positions = HashMap::new();
        let aligned: HashMap<Symbol, Vec<Option<Bar>>> = HashMap::new();
        let quote_ccy: HashMap<&str, &str> = HashMap::new();
        let fx = FxTable::new("USD");

        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            -100,
        );
        assert_eq!(*cash.get(&Currency::USD).unwrap(), -5000.0);
    }

    // ── LimitViolation enum ──────────────────────────────────────────────

    #[test]
    fn limit_violation_debug_and_eq() {
        assert_eq!(LimitViolation::Margin, LimitViolation::Margin);
        assert_eq!(LimitViolation::PositionSize, LimitViolation::PositionSize);
        assert_ne!(LimitViolation::Margin, LimitViolation::PositionSize);
        // Debug trait
        let s = format!("{:?}", LimitViolation::Margin);
        assert_eq!(s, "Margin");
    }
}
