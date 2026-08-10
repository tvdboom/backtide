//! Deterministic paper-broker execution and portfolio accounting.

use crate::backtest::models::{Order, OrderId, OrderStatus, OrderType, Portfolio, SizerSlot};
use crate::backtest::orders::{apply_slippage, resolve_trigger, TriggerOutcome};
use crate::backtest::utils::{is_negligible, is_significant};
use crate::constants::{CashAmount, PositionAmount};
use crate::live::models::{
    MarketUpdate, PaperFill, PaperTradingConfig, PaperTradingSnapshot, PaperTradingUpdate,
};
use pyo3::prelude::*;
use std::collections::{HashMap, HashSet};

/// Stateful, deterministic paper broker.
///
/// The broker has no network or wall-clock dependencies. Callers provide
/// market updates and orders, making the same implementation usable by live
/// WebSocket feeds, replay tests, and benchmarks.
#[derive(Debug)]
pub struct PaperBroker {
    config: PaperTradingConfig,
    portfolio: Portfolio,
    latest_prices: HashMap<String, f64>,
    average_cost: HashMap<String, f64>,
    trail_state: HashMap<OrderId, (f64, f64)>,
    known_order_ids: HashSet<OrderId>,
    last_seen_open_ts: HashMap<String, u64>,
    last_processed_final_ts: HashMap<String, u64>,
    realized_pnl: f64,
    processed_bars: u64,
}

impl PaperBroker {
    /// Create a broker from validated paper-trading configuration.
    pub fn new(config: PaperTradingConfig) -> Result<Self, String> {
        validate_config(&config)?;
        let mut portfolio = Portfolio::default();
        portfolio.cash.clear();
        portfolio.cash.insert(config.base_currency, config.initial_cash);

        Ok(Self {
            config,
            portfolio,
            latest_prices: HashMap::new(),
            average_cost: HashMap::new(),
            trail_state: HashMap::new(),
            known_order_ids: HashSet::new(),
            last_seen_open_ts: HashMap::new(),
            last_processed_final_ts: HashMap::new(),
            realized_pnl: 0.0,
            processed_bars: 0,
        })
    }

    /// Read the current portfolio without cloning it.
    pub fn portfolio(&self) -> &Portfolio {
        &self.portfolio
    }

    /// Process one market update and any orders produced from that update.
    ///
    /// Resting orders are matched first. Newly submitted market orders for the
    /// update's symbol fill against its latest close; all other orders rest
    /// until a matching symbol update arrives.
    pub fn process(&mut self, market: MarketUpdate, orders: Vec<Order>) -> PaperTradingUpdate {
        let (mut fills, should_process) = self.begin_update(&market);
        let orders_submitted = orders.len();
        if should_process {
            self.submit_orders(orders, &market, &mut fills);
        }

        PaperTradingUpdate {
            market,
            fills,
            snapshot: self.snapshot(),
            orders_submitted,
            processed: should_process,
        }
    }

    pub(crate) fn begin_update(&mut self, market: &MarketUpdate) -> (Vec<PaperFill>, bool) {
        let mut fills = Vec::new();
        let structurally_valid = market.is_valid_bar();

        let is_stale = self
            .last_seen_open_ts
            .get(&market.symbol)
            .is_some_and(|timestamp| market.open_ts < *timestamp);
        let valid_market = structurally_valid && !is_stale;
        if valid_market {
            self.latest_prices.insert(market.symbol.clone(), market.close);
            self.last_seen_open_ts.insert(market.symbol.clone(), market.open_ts);
        }

        let already_processed_final = market.is_final
            && self
                .last_processed_final_ts
                .get(&market.symbol)
                .is_some_and(|timestamp| market.open_ts <= *timestamp);
        let should_process = valid_market
            && !already_processed_final
            && (market.is_final || self.config.trade_on_partial);
        if should_process {
            let bar = market.bar();
            self.processed_bars += 1;
            self.match_resting_orders(&market.symbol, &bar, &mut fills);
            if market.is_final {
                self.last_processed_final_ts.insert(market.symbol.clone(), market.open_ts);
            }
        }

        (fills, should_process)
    }

    pub(crate) fn submit_orders(
        &mut self,
        orders: Vec<Order>,
        market: &MarketUpdate,
        fills: &mut Vec<PaperFill>,
    ) {
        for order in orders {
            self.submit_order(order, market, fills);
        }
    }

    /// Return a cloned mark-to-market account snapshot.
    pub fn snapshot(&self) -> PaperTradingSnapshot {
        let cash = self.portfolio.cash.amount(&self.config.base_currency);
        let mut market_value = 0.0;
        let mut unrealized_pnl = 0.0;

        for (symbol, quantity) in &self.portfolio.positions {
            let Some(price) = self.latest_prices.get(symbol) else {
                continue;
            };
            market_value += quantity * price;

            if let Some(cost) = self.average_cost.get(symbol) {
                unrealized_pnl += if *quantity >= 0.0 {
                    (price - cost) * quantity
                } else {
                    (cost - price) * quantity.abs()
                };
            }
        }

        PaperTradingSnapshot {
            portfolio: self.portfolio.clone(),
            latest_prices: self.latest_prices.clone(),
            equity: cash + market_value,
            realized_pnl: self.realized_pnl,
            unrealized_pnl,
            processed_bars: self.processed_bars,
        }
    }

    fn match_resting_orders(
        &mut self,
        symbol: &str,
        bar: &crate::data::models::Bar,
        fills: &mut Vec<PaperFill>,
    ) {
        let mut still_open = Vec::with_capacity(self.portfolio.orders.len());
        let open_orders = std::mem::take(&mut self.portfolio.orders);

        for mut order in open_orders {
            if order.symbol != symbol {
                still_open.push(order);
                continue;
            }

            match resolve_trigger(
                &mut order,
                bar,
                &self.portfolio.positions,
                &mut self.trail_state,
                false,
            ) {
                TriggerOutcome::Fill {
                    raw_px,
                    reason,
                    limit_cap,
                } => fills.push(self.execute(order, bar.open_ts as i64, raw_px, limit_cap, reason)),
                TriggerOutcome::Pending => still_open.push(order),
                TriggerOutcome::Cancel {
                    reason,
                } => {
                    self.trail_state.remove(&order.id);
                    fills.push(canceled_fill(order, bar.open_ts as i64, reason));
                },
            }
        }

        self.portfolio.orders = still_open;
    }

    fn submit_order(&mut self, order: Order, market: &MarketUpdate, fills: &mut Vec<PaperFill>) {
        if order.order_type == OrderType::Cancel {
            if let Some(index) = self.portfolio.orders.iter().position(|open| open.id == order.id) {
                let canceled = self.portfolio.orders.remove(index);
                self.trail_state.remove(&canceled.id);
                fills.push(canceled_fill(
                    canceled,
                    market.close_ts as i64,
                    "canceled by cancellation order".to_owned(),
                ));
            } else {
                fills.push(rejected_fill(
                    order,
                    market.close_ts as i64,
                    "cancel target is not open".to_owned(),
                ));
            }
            return;
        }

        if !self.known_order_ids.insert(order.id) {
            fills.push(rejected_fill(
                order,
                market.close_ts as i64,
                "duplicate order id".to_owned(),
            ));
            return;
        }

        if order.symbol.trim().is_empty() || !order.quantity.is_finite() {
            fills.push(rejected_fill(
                order,
                market.close_ts as i64,
                "order must have a symbol and finite quantity".to_owned(),
            ));
            return;
        }

        if order.order_type == OrderType::Market && order.symbol == market.symbol {
            fills.push(self.execute(
                order,
                market.close_ts as i64,
                market.close,
                None,
                "live market fill".to_owned(),
            ));
        } else {
            self.portfolio.orders.push(order);
        }
    }

    fn execute(
        &mut self,
        mut order: Order,
        timestamp: i64,
        raw_price: f64,
        limit_cap: Option<f64>,
        reason: String,
    ) -> PaperFill {
        if let Some(sizer) = order.sizer.take() {
            let equity = self.snapshot().equity;
            let stop_distance = order.price.and_then(|price| {
                let distance = (raw_price - price).abs();
                (distance > 0.0).then_some(distance)
            });
            let quantity = match sizer {
                SizerSlot::Builtin(builtin) => {
                    builtin.calculate(equity, raw_price, stop_distance, None)
                },
                SizerSlot::Custom(custom) => Python::attach(|py| -> PyResult<f64> {
                    custom
                        .bind(py)
                        .call_method1(
                            "calculate",
                            (equity, raw_price, stop_distance, Option::<f64>::None),
                        )?
                        .extract()
                })
                .map_err(|error| error.to_string()),
            };

            match quantity {
                Ok(quantity) => order.quantity = quantity,
                Err(error) => {
                    return rejected_fill(order, timestamp, format!("sizer failed: {error}"));
                },
            }
        }

        if !raw_price.is_finite()
            || raw_price <= 0.0
            || !order.quantity.is_finite()
            || is_negligible(order.quantity)
        {
            return rejected_fill(order, timestamp, "invalid fill price or quantity".to_owned());
        }

        let fill_price = apply_slippage(raw_price, order.quantity, self.config.slippage, limit_cap);
        let old_quantity = self.portfolio.positions.amount(&order.symbol);
        let new_quantity = old_quantity + order.quantity;

        if !self.config.allow_short && new_quantity < 0.0 && is_significant(new_quantity) {
            return rejected_fill(order, timestamp, "short selling is disabled".to_owned());
        }

        let notional = fill_price * order.quantity.abs();
        let commission =
            notional * self.config.commission_pct / 100.0 + self.config.commission_fixed;
        let cash = self.portfolio.cash.amount(&self.config.base_currency);
        let next_cash = cash - order.quantity * fill_price - commission;
        if !self.config.allow_margin && next_cash < 0.0 && is_significant(next_cash) {
            return rejected_fill(order, timestamp, "insufficient cash".to_owned());
        }

        self.portfolio.cash.insert(self.config.base_currency, next_cash);
        if is_negligible(new_quantity) {
            self.portfolio.positions.remove(&order.symbol);
        } else {
            self.portfolio.positions.insert(order.symbol.clone(), new_quantity);
        }

        let pnl = self.update_cost_basis(
            &order.symbol,
            old_quantity,
            order.quantity,
            fill_price,
            commission,
        );
        self.trail_state.remove(&order.id);

        PaperFill {
            order,
            timestamp,
            status: OrderStatus::Filled,
            fill_price: Some(fill_price),
            commission,
            realized_pnl: Some(pnl),
            reason,
        }
    }

    fn update_cost_basis(
        &mut self,
        symbol: &str,
        old_quantity: f64,
        delta: f64,
        price: f64,
        commission: f64,
    ) -> f64 {
        let old_cost = self.average_cost.get(symbol).copied().unwrap_or(price);
        let new_quantity = old_quantity + delta;
        let same_direction = is_negligible(old_quantity) || old_quantity.signum() == delta.signum();
        let mut realized = -commission;

        if same_direction {
            let total = old_quantity.abs() + delta.abs();
            let average = if is_negligible(total) {
                price
            } else {
                (old_cost * old_quantity.abs() + price * delta.abs()) / total
            };
            self.average_cost.insert(symbol.to_owned(), average);
        } else {
            let closed = old_quantity.abs().min(delta.abs());
            realized += if old_quantity > 0.0 {
                (price - old_cost) * closed
            } else {
                (old_cost - price) * closed
            };

            if is_negligible(new_quantity) {
                self.average_cost.remove(symbol);
            } else if new_quantity.signum() != old_quantity.signum() {
                self.average_cost.insert(symbol.to_owned(), price);
            }
        }

        self.realized_pnl += realized;
        realized
    }
}

fn validate_config(config: &PaperTradingConfig) -> Result<(), String> {
    if !config.initial_cash.is_finite() || config.initial_cash < 0.0 {
        return Err("initial_cash must be finite and non-negative".to_owned());
    }
    if !config.commission_pct.is_finite() || config.commission_pct < 0.0 {
        return Err("commission_pct must be finite and non-negative".to_owned());
    }
    if !config.commission_fixed.is_finite() || config.commission_fixed < 0.0 {
        return Err("commission_fixed must be finite and non-negative".to_owned());
    }
    if !config.slippage.is_finite() || config.slippage < 0.0 {
        return Err("slippage must be finite and non-negative".to_owned());
    }
    if config.max_history == 0 {
        return Err("max_history must be positive".to_owned());
    }
    Ok(())
}

fn canceled_fill(order: Order, timestamp: i64, reason: String) -> PaperFill {
    PaperFill {
        order,
        timestamp,
        status: OrderStatus::Canceled,
        fill_price: None,
        commission: 0.0,
        realized_pnl: None,
        reason,
    }
}

fn rejected_fill(order: Order, timestamp: i64, reason: String) -> PaperFill {
    PaperFill {
        order,
        timestamp,
        status: OrderStatus::Rejected,
        fill_price: None,
        commission: 0.0,
        realized_pnl: None,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::models::{Order, OrderId};
    use crate::data::models::Currency;

    fn update(close: f64, timestamp: u64) -> MarketUpdate {
        MarketUpdate {
            provider: "mock".to_owned(),
            symbol: "BTC-USD".to_owned(),
            interval: "1m".to_owned(),
            open_ts: timestamp,
            close_ts: timestamp + 60,
            open: close,
            high: close,
            low: close,
            close,
            volume: 1.0,
            n_trades: Some(1),
            is_final: true,
            received_ts: timestamp as i64 + 60,
        }
    }

    fn market(quantity: f64) -> Order {
        Order {
            id: OrderId::new(),
            symbol: "BTC-USD".to_owned(),
            quantity,
            order_type: OrderType::Market,
            price: None,
            limit_price: None,
            sizer: None,
        }
    }

    #[test]
    fn market_buy_updates_cash_position_and_equity() {
        let mut broker = PaperBroker::new(PaperTradingConfig::default()).unwrap();
        let result = broker.process(update(100.0, 1_000), vec![market(10.0)]);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].status, OrderStatus::Filled);
        assert_eq!(result.snapshot.portfolio.positions["BTC-USD"], 10.0);
        assert_eq!(result.snapshot.portfolio.cash[&Currency::USD], 99_000.0);
        assert_eq!(result.snapshot.equity, 100_000.0);
    }

    #[test]
    fn closing_trade_realizes_pnl() {
        let mut broker = PaperBroker::new(PaperTradingConfig::default()).unwrap();
        broker.process(update(100.0, 1_000), vec![market(10.0)]);
        let result = broker.process(update(110.0, 1_060), vec![market(-10.0)]);

        assert_eq!(result.snapshot.realized_pnl, 100.0);
        assert_eq!(result.snapshot.equity, 100_100.0);
        assert!(result.snapshot.portfolio.positions.is_empty());
    }

    #[test]
    fn partial_updates_only_mark_to_market_by_default() {
        let mut partial = update(100.0, 1_000);
        partial.is_final = false;
        let mut broker = PaperBroker::new(PaperTradingConfig::default()).unwrap();
        let result = broker.process(partial, vec![market(1.0)]);

        assert!(!result.processed);
        assert!(result.fills.is_empty());
        assert_eq!(result.snapshot.processed_bars, 0);
    }

    #[test]
    fn limit_order_rests_until_reached() {
        let mut broker = PaperBroker::new(PaperTradingConfig::default()).unwrap();
        let mut order = market(1.0);
        order.order_type = OrderType::Limit;
        order.price = Some(90.0);

        let first = broker.process(update(100.0, 1_000), vec![order]);
        assert_eq!(first.snapshot.portfolio.orders.len(), 1);

        let mut next = update(95.0, 1_060);
        next.low = 89.0;
        let second = broker.process(next, Vec::new());
        assert_eq!(second.fills[0].fill_price, Some(90.0));
        assert!(second.snapshot.portfolio.orders.is_empty());
    }

    #[test]
    fn short_and_margin_guards_reject_risky_orders() {
        let config = PaperTradingConfig {
            initial_cash: 100.0,
            ..PaperTradingConfig::default()
        };
        let mut broker = PaperBroker::new(config).unwrap();

        let short = broker.process(update(10.0, 1_000), vec![market(-1.0)]);
        assert_eq!(short.fills[0].status, OrderStatus::Rejected);
        let margin = broker.process(update(10.0, 1_060), vec![market(20.0)]);
        assert_eq!(margin.fills[0].status, OrderStatus::Rejected);
        assert_eq!(margin.snapshot.equity, 100.0);
    }

    #[test]
    fn duplicate_and_stale_final_bars_are_not_processed_twice() {
        let mut broker = PaperBroker::new(PaperTradingConfig::default()).unwrap();
        let first = broker.process(update(100.0, 1_000), vec![market(1.0)]);
        assert!(first.processed);

        let duplicate = broker.process(update(101.0, 1_000), vec![market(1.0)]);
        assert!(!duplicate.processed);
        assert!(duplicate.fills.is_empty());
        assert_eq!(duplicate.snapshot.processed_bars, 1);

        let stale = broker.process(update(99.0, 900), vec![market(1.0)]);
        assert!(!stale.processed);
        assert_eq!(stale.snapshot.latest_prices["BTC-USD"], 101.0);
        assert_eq!(stale.snapshot.portfolio.positions["BTC-USD"], 1.0);
    }
}
