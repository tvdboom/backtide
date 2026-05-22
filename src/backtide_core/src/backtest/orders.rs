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
