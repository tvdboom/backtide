//! Order trigger helpers.
//!
//! `resolve_trigger` decides whether an open order fills against the
//! current bar, returning a `TriggerOutcome`:
//!
//!   * `Fill`: The order fills at `raw_px` (before slippage). The caller is
//!     responsible for applying slippage and bookkeeping (cash, positions,
//!     commissions, etc...).
//!   * `Pending`: The order does not fill this bar. Keep it open.
//!   * `Cancel`: The order cannot make sense (e.g., SettlePosition with no
//!     current position). Record as canceled.
//!
//! Stop-into-limit variants mutate `order` in place: when the stop fires we
//! replace `order_type` with `OrderType::Limit` and copy `order.limit_price`
//! into `order.price`, so that on subsequent bars the order rests as a regular
//! limit order.
//!
//! Trailing variants share `trail_state`, keyed by `order.id`, holding
//! `(running_high, running_low)` since the order was placed.

use crate::backtest::models::{Order, OrderId, OrderType, Trade};
use crate::backtest::utils::{is_negligible, is_significant};
use crate::constants::{PositionAmount, Positions};
use crate::data::models::Bar;
use std::collections::HashMap;

#[derive(Debug)]
pub enum TriggerOutcome {
    /// The order fills at `raw_px` (before slippage).
    /// `limit_cap` constrains slippage so the slipped fill never crosses
    /// the resting limit price (used for LimitOrder and its variants).
    Fill {
        raw_px: f64,
        reason: String,
        limit_cap: Option<f64>,
    },

    /// The order does not fill this bar.
    Pending,

    /// The order is invalid against the current state and should be canceled.
    Cancel {
        reason: String,
    },
}

/// Decide whether `order` fills this `bar`.
///
/// May mutate `order` for the stop-into-limit transition, and may mutate
/// `trail_state` for trailing variants.
pub fn resolve_trigger(
    order: &mut Order,
    bar: &Bar,
    positions: &Positions,
    trail_state: &mut HashMap<OrderId, (f64, f64)>,
    trade_on_close: bool,
) -> TriggerOutcome {
    match order.order_type {
        // Cancel is handled before resolve_trigger is called.
        OrderType::Cancel => TriggerOutcome::Cancel {
            reason: "canceled by cancellation order".into(),
        },
        OrderType::Market => TriggerOutcome::Fill {
            raw_px: if trade_on_close {
                bar.close
            } else {
                bar.open
            },
            reason: String::new(),
            limit_cap: None,
        },
        OrderType::SettlePosition => {
            let cur = positions.amount(&order.symbol);
            if is_negligible(cur) {
                return TriggerOutcome::Cancel {
                    reason: "no position to settle".into(),
                };
            }

            // Translate to a market order that flattens the position.
            order.quantity = -cur;
            order.order_type = OrderType::Market;

            TriggerOutcome::Fill {
                raw_px: if trade_on_close {
                    bar.close
                } else {
                    bar.open
                },
                reason: "settle position".into(),
                limit_cap: None,
            }
        },
        OrderType::Limit => match order.price {
            Some(lim) => fill_limit(order.quantity, bar, lim),
            None => TriggerOutcome::Cancel {
                reason: "limit order missing price".into(),
            },
        },
        OrderType::TakeProfit => match order.price {
            // Take-profit is a profit-target limit: same execution
            // semantics as Limit (a buy fills at-or-below, a sell at-or-above).
            Some(target) => fill_limit(order.quantity, bar, target),
            None => TriggerOutcome::Cancel {
                reason: "take-profit missing price".into(),
            },
        },
        OrderType::StopLoss => {
            let stop = match order.price {
                Some(p) => p,
                None => {
                    return TriggerOutcome::Cancel {
                        reason: "stop-loss missing price".into(),
                    }
                },
            };

            if stop_triggered(order.quantity, bar, stop, false) {
                fill_stop(order.quantity, bar, stop)
            } else {
                TriggerOutcome::Pending
            }
        },
        OrderType::StopLossLimit | OrderType::TakeProfitLimit => {
            let stop = match order.price {
                Some(p) => p,
                None => {
                    return TriggerOutcome::Cancel {
                        reason: "stop-limit missing stop price".into(),
                    }
                },
            };

            let is_tp = order.order_type == OrderType::TakeProfitLimit;
            if !stop_triggered(order.quantity, bar, stop, is_tp) {
                return TriggerOutcome::Pending;
            }

            // Convert to a resting Limit at `limit_price` (or stop as fallback).
            let lim = order.limit_price.unwrap_or(stop);
            order.order_type = OrderType::Limit;
            order.price = Some(lim);
            order.limit_price = None;

            // Try to fill same bar; if the limit can't be hit on this bar
            // it will rest and re-evaluate next bar via the new Limit path.
            fill_limit(order.quantity, bar, lim)
        },
        OrderType::TrailingStop | OrderType::TrailingStopLimit => {
            let trail = match order.price {
                Some(p) if p > 0.0 => p,
                _ => {
                    return TriggerOutcome::Cancel {
                        reason: "trailing stop missing/invalid trail amount".into(),
                    }
                },
            };

            // First-bar initialization: seed extremes from this bar.
            let entry = trail_state.entry(order.id).or_insert_with(|| (bar.high, bar.low));
            entry.0 = entry.0.max(bar.high);
            entry.1 = entry.1.min(bar.low);

            let (running_high, running_low) = (entry.0, entry.1);

            // Effective stop: sells trail running_high downward; buys
            // trail running_low upward. `qty == 0` is meaningless here.
            let stop = if order.quantity < 0.0 {
                running_high - trail
            } else if order.quantity > 0.0 {
                running_low + trail
            } else {
                return TriggerOutcome::Cancel {
                    reason: "zero quantity".into(),
                };
            };

            // Re-use the regular stop-trigger / stop-fill helpers.
            if !stop_triggered(order.quantity, bar, stop, false) {
                return TriggerOutcome::Pending;
            }

            if order.order_type == OrderType::TrailingStopLimit {
                let lim = order.limit_price.unwrap_or(stop);
                order.order_type = OrderType::Limit;
                order.price = Some(lim);
                order.limit_price = None;
                fill_limit(order.quantity, bar, lim)
            } else {
                trail_state.remove(&order.id);
                fill_stop(order.quantity, bar, stop)
            }
        },
    }
}

/// Apply slippage to a raw fill price, optionally capping at the limit
/// price so a buy never pays above its limit (and a sell never receives
/// below).
pub fn apply_slippage(raw_px: f64, qty: f64, slippage: f64, limit_cap: Option<f64>) -> f64 {
    // slippage_pct is the fraction (e.g., 0.005 = 0.5%)
    let slipped = if qty >= 0.0 {
        raw_px * (1.0 + slippage / 100.)
    } else {
        raw_px * (1.0 - slippage / 100.)
    };

    match limit_cap {
        Some(cap) if qty > 0.0 => slipped.min(cap),
        Some(cap) if qty < 0.0 => slipped.max(cap),
        _ => slipped,
    }
}

/// Close an open position and return the finalized [`Trade`].
pub fn close_open_trade_sell(
    open_trades: &mut HashMap<String, (i64, f64, f64)>,
    symbol: &str,
    ts: i64,
    abs_qty: f64,
    exit_px: f64,
    commission: f64,
) -> Option<Trade> {
    let (owned_key, (entry_ts, mut q, entry_px)) = open_trades.remove_entry(symbol)?;

    let used = abs_qty.min(q);
    q -= used;

    let trade_symbol = if is_significant(q) {
        let sym = owned_key.clone();
        open_trades.insert(owned_key, (entry_ts, q, entry_px));
        sym
    } else {
        owned_key
    };

    Some(Trade {
        symbol: trade_symbol,
        quantity: used,
        entry_ts,
        exit_ts: ts,
        entry_price: entry_px,
        exit_price: exit_px,
        pnl: (exit_px - entry_px) * used - commission,
    })
}

/// Fill semantics for a Limit (or TakeProfit, identical execution-wise):
///
/// * Buy (qty > 0): fill if price reached the limit *or below*. If the
///   bar opens at-or-below the limit, fill at the open (better than
///   limit). Otherwise, if `bar.low <= lim`, fill at the limit price.
/// * Sell (qty < 0): symmetric — fill at open if open ≥ limit, else at
///   limit if `bar.high >= lim`.
fn fill_limit(qty: f64, bar: &Bar, lim: f64) -> TriggerOutcome {
    if qty > 0.0 {
        if bar.open <= lim {
            TriggerOutcome::Fill {
                raw_px: bar.open,
                reason: "limit (open through)".into(),
                limit_cap: Some(lim),
            }
        } else if bar.low <= lim {
            TriggerOutcome::Fill {
                raw_px: lim,
                reason: "limit hit".into(),
                limit_cap: Some(lim),
            }
        } else {
            TriggerOutcome::Pending
        }
    } else if qty < 0.0 {
        if bar.open >= lim {
            TriggerOutcome::Fill {
                raw_px: bar.open,
                reason: "limit (open through)".into(),
                limit_cap: Some(lim),
            }
        } else if bar.high >= lim {
            TriggerOutcome::Fill {
                raw_px: lim,
                reason: "limit hit".into(),
                limit_cap: Some(lim),
            }
        } else {
            TriggerOutcome::Pending
        }
    } else {
        TriggerOutcome::Cancel {
            reason: "zero quantity".into(),
        }
    }
}

/// Stop trigger predicate.
///
/// * Stop-loss sell (qty < 0, long-protection): triggers when price
///   *falls* to `stop` — `bar.low <= stop` or gap-down (`bar.open <= stop`).
/// * Stop-loss buy  (qty > 0, short-cover): triggers when price *rises*
///   to `stop` — `bar.high >= stop` or gap-up (`bar.open >= stop`).
/// * Take-profit-limit reverses both directions (a sell TP triggers on
///   a price rise, a buy TP on a price drop).
fn stop_triggered(qty: f64, bar: &Bar, stop: f64, is_take_profit: bool) -> bool {
    let down_trigger = (qty < 0.0 && !is_take_profit) || (qty > 0.0 && is_take_profit);
    let up_trigger = (qty > 0.0 && !is_take_profit) || (qty < 0.0 && is_take_profit);
    if down_trigger {
        bar.open <= stop || bar.low <= stop
    } else if up_trigger {
        bar.open >= stop || bar.high >= stop
    } else {
        false
    }
}

/// Stop fill price. Realistic gap handling: if the bar opens past the
/// stop level, the stop fills at the open (worse than the stop) — a
/// gap-down for sell stops, a gap-up for buy stops. Otherwise, the stop
/// fills at exactly the stop level.
fn fill_stop(qty: f64, bar: &Bar, stop: f64) -> TriggerOutcome {
    if qty == 0.0 {
        return TriggerOutcome::Cancel {
            reason: "zero quantity".into(),
        };
    }

    // First check if the stop was actually triggered
    if !stop_triggered(qty, bar, stop, false) {
        return TriggerOutcome::Pending;
    }

    // Stop was triggered, now determine the fill price
    if qty < 0.0 {
        if bar.open <= stop {
            TriggerOutcome::Fill {
                raw_px: bar.open,
                reason: "stop triggered (gap-down)".into(),
                limit_cap: None,
            }
        } else {
            TriggerOutcome::Fill {
                raw_px: stop,
                reason: "stop triggered".into(),
                limit_cap: None,
            }
        }
    } else {
        if bar.open >= stop {
            TriggerOutcome::Fill {
                raw_px: bar.open,
                reason: "stop triggered (gap-up)".into(),
                limit_cap: None,
            }
        } else {
            TriggerOutcome::Fill {
                raw_px: stop,
                reason: "stop triggered".into(),
                limit_cap: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::models::{Order, OrderId, OrderType};
    use crate::data::models::Bar;
    use std::collections::HashMap;

    fn bar(open: f64, high: f64, low: f64, close: f64) -> Bar {
        Bar {
            open_ts: 100,
            close_ts: 200,
            open_ts_exchange: 100,
            open,
            high,
            low,
            close,
            adj_close: close,
            volume: 1000.0,
            n_trades: None,
        }
    }

    fn make_order(order_type: OrderType, qty: f64, price: Option<f64>) -> Order {
        Order {
            id: OrderId::new(),
            symbol: "AAPL".to_owned(),
            quantity: qty,
            order_type,
            price,
            limit_price: None,
            sizer: None,
        }
    }

    fn make_order_with_limit(
        order_type: OrderType,
        qty: f64,
        price: Option<f64>,
        limit_price: Option<f64>,
    ) -> Order {
        Order {
            id: OrderId::new(),
            symbol: "AAPL".to_owned(),
            quantity: qty,
            order_type,
            price,
            limit_price,
            sizer: None,
        }
    }

    fn empty_positions() -> Positions {
        HashMap::new()
    }

    fn positions_with(sym: &str, qty: f64) -> Positions {
        let mut p = HashMap::new();
        p.insert(sym.to_owned(), qty);
        p
    }

    // ── Market Orders ────────────────────────────────────────────────────

    #[test]
    fn market_order_fills_at_open() {
        let mut order = make_order(OrderType::Market, 10.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                limit_cap,
                ..
            } => {
                assert_eq!(raw_px, 100.0);
                assert!(limit_cap.is_none());
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn market_order_fills_at_close_when_trade_on_close() {
        let mut order = make_order(OrderType::Market, 10.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, true) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert_eq!(raw_px, 105.0),
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn market_sell_order_fills_at_open() {
        let mut order = make_order(OrderType::Market, -5.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert_eq!(raw_px, 100.0),
            _ => panic!("expected Fill"),
        }
    }

    // ── Cancel Orders ────────────────────────────────────────────────────

    #[test]
    fn cancel_order_returns_cancel() {
        let mut order = make_order(OrderType::Cancel, 0.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("canceled"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    // ── SettlePosition Orders ────────────────────────────────────────────

    #[test]
    fn settle_position_with_existing_long() {
        let mut order = make_order(OrderType::SettlePosition, 0.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = positions_with("AAPL", 50.0);
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => {
                assert_eq!(raw_px, 100.0);
                assert_eq!(order.quantity, -50.0);
                assert_eq!(order.order_type, OrderType::Market);
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn settle_position_with_existing_short() {
        let mut order = make_order(OrderType::SettlePosition, 0.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = positions_with("AAPL", -30.0);
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => {
                assert_eq!(raw_px, 100.0);
                assert_eq!(order.quantity, 30.0);
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn settle_position_no_position_cancels() {
        let mut order = make_order(OrderType::SettlePosition, 0.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("no position"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn settle_position_trade_on_close() {
        let mut order = make_order(OrderType::SettlePosition, 0.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = positions_with("AAPL", 10.0);
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, true) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert_eq!(raw_px, 105.0),
            _ => panic!("expected Fill"),
        }
    }

    // ── Limit Orders ─────────────────────────────────────────────────────

    #[test]
    fn limit_buy_fills_when_open_below_limit() {
        let mut order = make_order(OrderType::Limit, 10.0, Some(105.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                limit_cap,
                reason,
            } => {
                assert_eq!(raw_px, 100.0);
                assert_eq!(limit_cap, Some(105.0));
                assert!(reason.contains("open through"));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn limit_buy_fills_at_limit_when_low_touches() {
        let mut order = make_order(OrderType::Limit, 10.0, Some(95.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                limit_cap,
                ..
            } => {
                assert_eq!(raw_px, 95.0);
                assert_eq!(limit_cap, Some(95.0));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn limit_buy_pending_when_price_above_limit() {
        let mut order = make_order(OrderType::Limit, 10.0, Some(85.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected Pending"),
        }
    }

    #[test]
    fn limit_sell_fills_when_open_above_limit() {
        let mut order = make_order(OrderType::Limit, -10.0, Some(95.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                limit_cap,
                reason,
            } => {
                assert_eq!(raw_px, 100.0);
                assert_eq!(limit_cap, Some(95.0));
                assert!(reason.contains("open through"));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn limit_sell_fills_at_limit_when_high_reaches() {
        let mut order = make_order(OrderType::Limit, -10.0, Some(108.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                limit_cap,
                ..
            } => {
                assert_eq!(raw_px, 108.0);
                assert_eq!(limit_cap, Some(108.0));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn limit_sell_pending_when_price_below_limit() {
        let mut order = make_order(OrderType::Limit, -10.0, Some(115.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected Pending"),
        }
    }

    #[test]
    fn limit_missing_price_cancels() {
        let mut order = make_order(OrderType::Limit, 10.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("missing price"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn limit_zero_quantity_cancels() {
        let mut order = make_order(OrderType::Limit, 0.0, Some(100.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("zero quantity"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    // ── TakeProfit Orders ────────────────────────────────────────────────

    #[test]
    fn take_profit_buy_fills() {
        let mut order = make_order(OrderType::TakeProfit, 10.0, Some(105.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert_eq!(raw_px, 100.0),
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn take_profit_sell_fills_at_limit() {
        let mut order = make_order(OrderType::TakeProfit, -10.0, Some(108.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert_eq!(raw_px, 108.0),
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn take_profit_missing_price_cancels() {
        let mut order = make_order(OrderType::TakeProfit, 10.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("missing price"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    // ── StopLoss Orders ──────────────────────────────────────────────────

    #[test]
    fn stop_loss_sell_triggers_on_low() {
        // Sell stop at 92: bar.low=90 <= 92 → triggers
        let mut order = make_order(OrderType::StopLoss, -10.0, Some(92.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                limit_cap,
                ..
            } => {
                assert_eq!(raw_px, 92.0);
                assert!(limit_cap.is_none());
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn stop_loss_sell_gap_down_fills_at_open() {
        // Sell stop at 102: bar.open=100 <= 102 → gap-down fill at open
        let mut order = make_order(OrderType::StopLoss, -10.0, Some(102.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                reason,
                ..
            } => {
                assert_eq!(raw_px, 100.0);
                assert!(reason.contains("gap-down"));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn stop_loss_buy_triggers_on_high() {
        // Buy stop at 108: bar.high=110 >= 108 → triggers at stop
        let mut order = make_order(OrderType::StopLoss, 10.0, Some(108.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert_eq!(raw_px, 108.0),
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn stop_loss_buy_gap_up_fills_at_open() {
        // Buy stop at 95: bar.open=100 >= 95 → gap-up fill at open
        let mut order = make_order(OrderType::StopLoss, 10.0, Some(95.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                reason,
                ..
            } => {
                assert_eq!(raw_px, 100.0);
                assert!(reason.contains("gap-up"));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn stop_loss_sell_not_triggered_stays_pending() {
        // Sell stop at 80: bar.low=90 > 80 → not triggered
        let mut order = make_order(OrderType::StopLoss, -10.0, Some(80.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected Pending"),
        }
    }

    #[test]
    fn stop_loss_buy_not_triggered_stays_pending() {
        // Buy stop at 115: bar.high=110 < 115 → not triggered
        let mut order = make_order(OrderType::StopLoss, 10.0, Some(115.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected Pending"),
        }
    }

    #[test]
    fn stop_loss_missing_price_cancels() {
        let mut order = make_order(OrderType::StopLoss, -10.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("missing price"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    // ── StopLossLimit Orders ─────────────────────────────────────────────

    #[test]
    fn stop_loss_limit_sell_triggers_and_converts() {
        // Sell stop-limit: stop at 92, limit at 91. Stop triggers (low=90 <= 92),
        // converts to limit, then limit fills (high >= 91).
        let mut order =
            make_order_with_limit(OrderType::StopLossLimit, -10.0, Some(92.0), Some(91.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                limit_cap,
                ..
            } => {
                assert_eq!(raw_px, 100.0); // open >= limit → fill at open
                assert_eq!(limit_cap, Some(91.0));
                assert_eq!(order.order_type, OrderType::Limit);
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn stop_loss_limit_not_triggered_stays_pending() {
        let mut order =
            make_order_with_limit(OrderType::StopLossLimit, -10.0, Some(80.0), Some(79.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected Pending"),
        }
    }

    #[test]
    fn stop_loss_limit_missing_price_cancels() {
        let mut order = make_order_with_limit(OrderType::StopLossLimit, -10.0, None, Some(91.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("missing stop price"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn stop_loss_limit_uses_stop_as_fallback_limit() {
        // No limit_price → fallback to stop price
        let mut order = make_order_with_limit(OrderType::StopLossLimit, -10.0, Some(92.0), None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                limit_cap,
                ..
            } => {
                assert_eq!(limit_cap, Some(92.0));
                assert_eq!(order.price, Some(92.0));
            },
            _ => panic!("expected Fill"),
        }
    }

    // ── TakeProfitLimit Orders ───────────────────────────────────────────

    #[test]
    fn take_profit_limit_sell_triggers_on_rise() {
        // Sell TP stop at 108, limit at 107. TP is reverse: sell triggers on rise.
        // is_take_profit=true for sell (qty<0): up_trigger.
        // bar.high=110 >= 108 → triggers.
        let mut order =
            make_order_with_limit(OrderType::TakeProfitLimit, -10.0, Some(108.0), Some(107.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                ..
            } => {
                assert_eq!(order.order_type, OrderType::Limit);
                assert_eq!(order.price, Some(107.0));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn take_profit_limit_buy_triggers_on_drop() {
        // Buy TP stop at 92, limit at 93. is_take_profit=true for buy (qty>0): down_trigger.
        // bar.low=90 <= 92 → triggers.
        let mut order =
            make_order_with_limit(OrderType::TakeProfitLimit, 10.0, Some(92.0), Some(93.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                ..
            } => {
                assert_eq!(order.order_type, OrderType::Limit);
                assert_eq!(order.price, Some(93.0));
            },
            _ => panic!("expected Fill"),
        }
    }

    // ── Trailing Stop Orders ─────────────────────────────────────────────

    #[test]
    fn trailing_stop_sell_triggers_when_price_drops_from_high() {
        let mut order = make_order(OrderType::TrailingStop, -10.0, Some(15.0));
        let pos = empty_positions();
        let mut trail = HashMap::new();

        // Bar 1: high=110, low=90. running_high=110, stop=110-15=95.
        // sell stop: bar.low=90 <= 95 → triggers.
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => {
                assert_eq!(raw_px, 95.0);
            },
            _ => panic!("expected Fill"),
        }
        // Trail state should be cleaned up after fill
        assert!(!trail.contains_key(&order.id));
    }

    #[test]
    fn trailing_stop_sell_stays_pending_when_not_triggered() {
        let mut order = make_order(OrderType::TrailingStop, -10.0, Some(25.0));
        let pos = empty_positions();
        let mut trail = HashMap::new();

        // Bar: high=110, low=90. running_high=110, stop=110-25=85.
        // sell stop: bar.low=90 > 85 → not triggered.
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected Pending"),
        }
        assert!(trail.contains_key(&order.id));
    }

    #[test]
    fn trailing_stop_buy_triggers_when_price_rises_from_low() {
        let mut order = make_order(OrderType::TrailingStop, 10.0, Some(15.0));
        let pos = empty_positions();
        let mut trail = HashMap::new();

        // Bar: high=110, low=90. running_low=90, stop=90+15=105.
        // buy stop: bar.high=110 >= 105 → triggers.
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => {
                assert_eq!(raw_px, 105.0);
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn trailing_stop_zero_quantity_cancels() {
        let mut order = make_order(OrderType::TrailingStop, 0.0, Some(10.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("zero quantity"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn trailing_stop_missing_trail_cancels() {
        let mut order = make_order(OrderType::TrailingStop, -10.0, None);
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("missing/invalid"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn trailing_stop_negative_trail_cancels() {
        let mut order = make_order(OrderType::TrailingStop, -10.0, Some(-5.0));
        let b = bar(100.0, 110.0, 90.0, 105.0);
        let pos = empty_positions();
        let mut trail = HashMap::new();
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("missing/invalid"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn trailing_stop_updates_running_extremes_across_bars() {
        let mut order = make_order(OrderType::TrailingStop, -10.0, Some(30.0));
        let pos = empty_positions();
        let mut trail = HashMap::new();

        // Bar 1: high=110, low=90. running_high=110, stop=80. Not triggered (low=90 > 80).
        let b1 = bar(100.0, 110.0, 90.0, 105.0);
        match resolve_trigger(&mut order, &b1, &pos, &mut trail, false) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected Pending"),
        }

        // Bar 2: high=120, low=95. running_high=120, stop=90. Not triggered (low=95 > 90).
        let b2 = bar(112.0, 120.0, 95.0, 115.0);
        match resolve_trigger(&mut order, &b2, &pos, &mut trail, false) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected Pending"),
        }

        let (rh, rl) = trail[&order.id];
        assert_eq!(rh, 120.0);
        assert_eq!(rl, 90.0);

        // Bar 3: high=121, low=85. running_high=121, stop=91. Triggered (low=85 <= 91).
        let b3 = bar(118.0, 121.0, 85.0, 90.0);
        match resolve_trigger(&mut order, &b3, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => {
                assert_eq!(raw_px, 91.0);
            },
            _ => panic!("expected Fill"),
        }
    }

    // ── TrailingStopLimit Orders ─────────────────────────────────────────

    #[test]
    fn trailing_stop_limit_converts_to_limit_on_trigger() {
        let mut order =
            make_order_with_limit(OrderType::TrailingStopLimit, -10.0, Some(15.0), Some(93.0));
        let pos = empty_positions();
        let mut trail = HashMap::new();

        // high=110, low=90. running_high=110, stop=95.
        // sell stop: low=90 <= 95 → triggers.
        // Converts to limit at 93.
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                limit_cap,
                ..
            } => {
                assert_eq!(order.order_type, OrderType::Limit);
                assert_eq!(order.price, Some(93.0));
                assert_eq!(limit_cap, Some(93.0));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn trailing_stop_limit_uses_stop_as_fallback() {
        let mut order =
            make_order_with_limit(OrderType::TrailingStopLimit, -10.0, Some(15.0), None);
        let pos = empty_positions();
        let mut trail = HashMap::new();

        let b = bar(100.0, 110.0, 90.0, 105.0);
        // stop = 110 - 15 = 95. Triggered: low=90 <= 95.
        // No limit_price → fallback to stop=95.
        match resolve_trigger(&mut order, &b, &pos, &mut trail, false) {
            TriggerOutcome::Fill {
                ..
            } => {
                assert_eq!(order.price, Some(95.0));
            },
            _ => panic!("expected Fill"),
        }
    }

    // ── apply_slippage ───────────────────────────────────────────────────

    #[test]
    fn slippage_buy_increases_price() {
        let px = apply_slippage(100.0, 10.0, 0.5, None);
        assert!((px - 100.5).abs() < 1e-10);
    }

    #[test]
    fn slippage_sell_decreases_price() {
        let px = apply_slippage(100.0, -10.0, 0.5, None);
        assert!((px - 99.5).abs() < 1e-10);
    }

    #[test]
    fn slippage_zero_qty_treated_as_buy() {
        let px = apply_slippage(100.0, 0.0, 0.5, None);
        assert!((px - 100.5).abs() < 1e-10);
    }

    #[test]
    fn slippage_buy_capped_at_limit() {
        // Buy slippage would push to 100.5, but limit cap is 100.2.
        let px = apply_slippage(100.0, 10.0, 0.5, Some(100.2));
        assert!((px - 100.2).abs() < 1e-10);
    }

    #[test]
    fn slippage_sell_floored_at_limit() {
        // Sell slippage would drop to 99.5, but limit cap is 99.8.
        let px = apply_slippage(100.0, -10.0, 0.5, Some(99.8));
        assert!((px - 99.8).abs() < 1e-10);
    }

    #[test]
    fn slippage_buy_no_cap_needed() {
        // Slipped price 100.5 < cap 101.0 → no capping.
        let px = apply_slippage(100.0, 10.0, 0.5, Some(101.0));
        assert!((px - 100.5).abs() < 1e-10);
    }

    #[test]
    fn slippage_sell_no_floor_needed() {
        // Slipped price 99.5 > cap 99.0 → no flooring.
        let px = apply_slippage(100.0, -10.0, 0.5, Some(99.0));
        assert!((px - 99.5).abs() < 1e-10);
    }

    #[test]
    fn slippage_zero_slippage() {
        assert_eq!(apply_slippage(100.0, 10.0, 0.0, None), 100.0);
        assert_eq!(apply_slippage(100.0, -10.0, 0.0, None), 100.0);
    }

    // ── close_open_trade_sell ────────────────────────────────────────────

    #[test]
    fn close_full_position() {
        let mut trades = HashMap::new();
        trades.insert("AAPL".to_owned(), (1000_i64, 50.0_f64, 150.0_f64));
        let trade = close_open_trade_sell(&mut trades, "AAPL", 2000, 50.0, 160.0, 5.0).unwrap();
        assert_eq!(trade.symbol, "AAPL");
        assert_eq!(trade.quantity, 50.0);
        assert_eq!(trade.entry_ts, 1000);
        assert_eq!(trade.exit_ts, 2000);
        assert_eq!(trade.entry_price, 150.0);
        assert_eq!(trade.exit_price, 160.0);
        assert!((trade.pnl - ((160.0 - 150.0) * 50.0 - 5.0)).abs() < 1e-10);
        assert!(!trades.contains_key("AAPL"));
    }

    #[test]
    fn close_partial_position() {
        let mut trades = HashMap::new();
        trades.insert("AAPL".to_owned(), (1000, 50.0, 150.0));
        let trade = close_open_trade_sell(&mut trades, "AAPL", 2000, 20.0, 160.0, 2.0).unwrap();
        assert_eq!(trade.quantity, 20.0);
        assert!((trade.pnl - ((160.0 - 150.0) * 20.0 - 2.0)).abs() < 1e-10);
        // Remaining position should still be tracked
        let (ts, remaining_q, px) = trades["AAPL"];
        assert_eq!(ts, 1000);
        assert!((remaining_q - 30.0).abs() < 1e-10);
        assert_eq!(px, 150.0);
    }

    #[test]
    fn close_nonexistent_returns_none() {
        let mut trades = HashMap::new();
        assert!(close_open_trade_sell(&mut trades, "AAPL", 2000, 10.0, 100.0, 0.0).is_none());
    }

    #[test]
    fn close_more_than_available() {
        let mut trades = HashMap::new();
        trades.insert("AAPL".to_owned(), (1000, 10.0, 150.0));
        let trade = close_open_trade_sell(&mut trades, "AAPL", 2000, 50.0, 160.0, 1.0).unwrap();
        // Only 10 available, so used = min(50, 10) = 10
        assert_eq!(trade.quantity, 10.0);
        assert!(!trades.contains_key("AAPL"));
    }

    #[test]
    fn close_with_loss_pnl() {
        let mut trades = HashMap::new();
        trades.insert("AAPL".to_owned(), (1000, 10.0, 150.0));
        let trade = close_open_trade_sell(&mut trades, "AAPL", 2000, 10.0, 140.0, 2.0).unwrap();
        // PnL = (140 - 150) * 10 - 2 = -102
        assert!((trade.pnl - (-102.0)).abs() < 1e-10);
    }

    // ── stop_triggered (via fill_stop) ───────────────────────────────────

    #[test]
    fn fill_stop_zero_qty_cancels() {
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match fill_stop(0.0, &b, 95.0) {
            TriggerOutcome::Cancel {
                reason,
            } => {
                assert!(reason.contains("zero quantity"));
            },
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn fill_stop_sell_at_stop_level() {
        // Sell stop at 92. bar.open=100 > 92, but low=90 <= 92 → fill at stop.
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match fill_stop(-10.0, &b, 92.0) {
            TriggerOutcome::Fill {
                raw_px,
                reason,
                ..
            } => {
                assert_eq!(raw_px, 92.0);
                assert!(reason.contains("stop triggered"));
                assert!(!reason.contains("gap"));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn fill_stop_buy_at_stop_level() {
        // Buy stop at 108. bar.open=100 < 108, but high=110 >= 108 → fill at stop.
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match fill_stop(10.0, &b, 108.0) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert_eq!(raw_px, 108.0),
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn fill_stop_not_triggered_returns_pending() {
        // Sell stop at 80. bar.low=90 > 80 → not triggered.
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match fill_stop(-10.0, &b, 80.0) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected Pending"),
        }
    }

    // ── stop_triggered predicate ─────────────────────────────────────────

    #[test]
    fn stop_triggered_sell_down() {
        let b = bar(100.0, 110.0, 90.0, 105.0);
        assert!(stop_triggered(-10.0, &b, 95.0, false)); // low=90 <= 95
        assert!(!stop_triggered(-10.0, &b, 85.0, false)); // low=90 > 85
    }

    #[test]
    fn stop_triggered_buy_up() {
        let b = bar(100.0, 110.0, 90.0, 105.0);
        assert!(stop_triggered(10.0, &b, 105.0, false)); // high=110 >= 105
        assert!(!stop_triggered(10.0, &b, 115.0, false)); // high=110 < 115
    }

    #[test]
    fn stop_triggered_zero_qty_never_triggers() {
        let b = bar(100.0, 110.0, 90.0, 105.0);
        assert!(!stop_triggered(0.0, &b, 95.0, false));
        assert!(!stop_triggered(0.0, &b, 95.0, true));
    }

    #[test]
    fn stop_triggered_take_profit_sell_triggers_on_rise() {
        let b = bar(100.0, 110.0, 90.0, 105.0);
        // TP sell (qty<0, is_tp=true) → up_trigger: high >= stop
        assert!(stop_triggered(-10.0, &b, 108.0, true));
        assert!(!stop_triggered(-10.0, &b, 115.0, true));
    }

    #[test]
    fn stop_triggered_take_profit_buy_triggers_on_drop() {
        let b = bar(100.0, 110.0, 90.0, 105.0);
        // TP buy (qty>0, is_tp=true) → down_trigger: low <= stop
        assert!(stop_triggered(10.0, &b, 92.0, true));
        assert!(!stop_triggered(10.0, &b, 85.0, true));
    }

    #[test]
    fn stop_triggered_gap_down_open() {
        // Sell stop: bar.open <= stop
        let b = bar(90.0, 110.0, 85.0, 105.0);
        assert!(stop_triggered(-10.0, &b, 95.0, false)); // open=90 <= 95
    }

    #[test]
    fn stop_triggered_gap_up_open() {
        // Buy stop: bar.open >= stop
        let b = bar(110.0, 115.0, 100.0, 112.0);
        assert!(stop_triggered(10.0, &b, 105.0, false)); // open=110 >= 105
    }

    // ── fill_limit (tested through resolve_trigger already, extra coverage) ──

    #[test]
    fn fill_limit_buy_exact_low_at_limit() {
        let b = bar(100.0, 110.0, 95.0, 105.0);
        match fill_limit(10.0, &b, 95.0) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert_eq!(raw_px, 95.0),
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn fill_limit_sell_exact_high_at_limit() {
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match fill_limit(-10.0, &b, 110.0) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert_eq!(raw_px, 110.0),
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn fill_limit_buy_open_exact_at_limit() {
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match fill_limit(10.0, &b, 100.0) {
            TriggerOutcome::Fill {
                raw_px,
                reason,
                ..
            } => {
                assert_eq!(raw_px, 100.0);
                assert!(reason.contains("open through"));
            },
            _ => panic!("expected Fill"),
        }
    }

    #[test]
    fn fill_limit_sell_open_exact_at_limit() {
        let b = bar(100.0, 110.0, 90.0, 105.0);
        match fill_limit(-10.0, &b, 100.0) {
            TriggerOutcome::Fill {
                raw_px,
                reason,
                ..
            } => {
                assert_eq!(raw_px, 100.0);
                assert!(reason.contains("open through"));
            },
            _ => panic!("expected Fill"),
        }
    }
}
