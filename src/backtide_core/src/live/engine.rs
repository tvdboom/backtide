//! Deterministic simulated execution and portfolio accounting.

use crate::backtest::fx::FxTable;
use crate::backtest::models::{
    EquitySample, Order, OrderId, OrderStatus, OrderType, Portfolio, SizerSlot, Trade,
};
use crate::backtest::orders::{apply_slippage, resolve_trigger, TriggerOutcome};
use crate::backtest::utils::{is_negligible, is_significant};
use crate::constants::{CashAmount, PositionAmount};
use crate::live::models::{
    MarketUpdate, SessionConfig, SessionFill, SessionSnapshot, SessionUpdate,
};
use crate::metrics::engine::{compute_builtin_metrics, is_builtin_metric};
use pyo3::prelude::*;
use std::collections::{HashMap, HashSet};

/// Stateful, deterministic simulation broker.
///
/// The broker has no network or wall-clock dependencies. Callers provide
/// market updates and orders, making the same implementation usable by live
/// WebSocket feeds, replay tests, and benchmarks.
#[derive(Debug)]
pub struct SessionBroker {
    config: SessionConfig,
    portfolio: Portfolio,
    latest_prices: HashMap<String, f64>,
    latest_price_timestamps: HashMap<String, i64>,
    quote_currencies: HashMap<String, String>,
    fx: FxTable,
    average_cost: HashMap<String, f64>,
    trail_state: HashMap<OrderId, (f64, f64)>,
    known_order_ids: HashSet<OrderId>,
    last_seen_open_ts: HashMap<String, u64>,
    last_processed_final_ts: HashMap<String, u64>,
    realized_pnl: f64,
    processed_bars: u64,
    peak_equity: f64,
    total_costs: f64,
    equity_curve: Vec<EquitySample>,
    trades: Vec<Trade>,
    entry_timestamps: HashMap<String, i64>,
    last_accrual_ts: Option<i64>,
    trading_halted: bool,
    halt_reason: Option<String>,
}

impl SessionBroker {
    /// Create a broker from validated session configuration.
    pub fn new(config: SessionConfig) -> Result<Self, String> {
        validate_config(&config)?;
        let initial_cash = config.initial_cash;
        let base_currency = config.base_currency.to_string();
        let mut portfolio = Portfolio::default();
        portfolio.cash.clear();
        portfolio.cash.insert(config.base_currency, config.initial_cash);

        Ok(Self {
            config,
            portfolio,
            latest_prices: HashMap::new(),
            latest_price_timestamps: HashMap::new(),
            quote_currencies: HashMap::new(),
            fx: FxTable::new(base_currency),
            average_cost: HashMap::new(),
            trail_state: HashMap::new(),
            known_order_ids: HashSet::new(),
            last_seen_open_ts: HashMap::new(),
            last_processed_final_ts: HashMap::new(),
            realized_pnl: 0.0,
            processed_bars: 0,
            peak_equity: initial_cash,
            total_costs: 0.0,
            equity_curve: Vec::new(),
            trades: Vec::new(),
            entry_timestamps: HashMap::new(),
            last_accrual_ts: None,
            trading_halted: false,
            halt_reason: None,
        })
    }

    /// Read the current portfolio without cloning it.
    pub fn portfolio(&self) -> &Portfolio {
        &self.portfolio
    }

    /// Read the bounded equity history used by performance metrics.
    pub(crate) fn equity_curve(&self) -> &[EquitySample] {
        &self.equity_curve
    }

    /// Read the bounded completed-trade history used by performance metrics.
    pub(crate) fn trades(&self) -> &[Trade] {
        &self.trades
    }

    /// Record a timestamped conversion rate for live account valuation.
    pub fn set_exchange_rate(
        &mut self,
        from_currency: &str,
        to_currency: &str,
        rate: f64,
        timestamp: i64,
    ) -> Result<(), String> {
        let from = from_currency.trim().to_uppercase();
        let to = to_currency.trim().to_uppercase();
        if from.is_empty() || to.is_empty() {
            return Err("exchange-rate currencies must be non-empty".to_owned());
        }
        if !rate.is_finite() || rate <= 0.0 {
            return Err("exchange rate must be finite and positive".to_owned());
        }
        self.fx.add_rate(&from, &to, timestamp, rate, self.config.max_history);
        Ok(())
    }

    /// Whether a conversion path is available at `timestamp`.
    pub fn has_exchange_rate(&self, from_currency: &str, timestamp: i64) -> bool {
        self.fx.rate(from_currency, &self.config.base_currency.to_string(), timestamp).is_some()
    }

    /// Process one market update and any orders produced from that update.
    ///
    /// Resting orders are matched first. Newly submitted market orders for the
    /// update's symbol fill against its latest close; all other orders rest
    /// until a matching symbol update arrives.
    pub fn process(&mut self, market: MarketUpdate, orders: Vec<Order>) -> SessionUpdate {
        let (mut fills, should_process) = self.begin_update(&market);
        let orders_submitted = orders.len();
        if should_process {
            self.submit_orders(orders, &market, &mut fills, false);
            self.finish_update(market.close_ts as i64);
        }

        SessionUpdate {
            market,
            fills,
            snapshot: self.snapshot(),
            orders_submitted,
            processed: should_process,
            indicators: HashMap::new(),
        }
    }

    pub(crate) fn begin_update(&mut self, market: &MarketUpdate) -> (Vec<SessionFill>, bool) {
        let mut fills = Vec::new();
        let structurally_valid = market.is_valid_bar();

        let is_stale = self
            .last_seen_open_ts
            .get(&market.symbol)
            .is_some_and(|timestamp| market.open_ts < *timestamp);
        let valid_market = structurally_valid && !is_stale;
        if valid_market {
            self.latest_prices.insert(market.symbol.clone(), market.close);
            self.latest_price_timestamps.insert(market.symbol.clone(), market.close_ts as i64);
            if let Some(quote_currency) = market.quote_currency.as_deref() {
                self.quote_currencies
                    .insert(market.symbol.clone(), quote_currency.trim().to_uppercase());
            }
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
            self.accrue_financing(market.close_ts as i64);
            self.processed_bars += 1;
            self.match_resting_orders(&market.symbol, &bar, &mut fills);
            self.enforce_maintenance_margin(market.close_ts as i64, &mut fills);
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
        fills: &mut Vec<SessionFill>,
        fit_buys_to_cash: bool,
    ) {
        for order in orders {
            self.submit_order(order, market, fills, fit_buys_to_cash);
        }
    }

    /// Return a cloned mark-to-market account snapshot.
    pub fn snapshot(&self) -> SessionSnapshot {
        let cash = self.portfolio.cash.amount(&self.config.base_currency);
        let mut market_value = 0.0;
        let mut gross_exposure = 0.0;
        let mut unrealized_pnl = 0.0;

        for (symbol, quantity) in &self.portfolio.positions {
            let (Some(price), Some(timestamp)) =
                (self.latest_prices.get(symbol), self.latest_price_timestamps.get(symbol))
            else {
                continue;
            };
            let Some(account_price) = self.account_price(symbol, *price, *timestamp) else {
                continue;
            };
            market_value += quantity * account_price;
            gross_exposure += quantity.abs() * account_price;

            if let Some(cost) = self.average_cost.get(symbol) {
                unrealized_pnl += if *quantity >= 0.0 {
                    (account_price - cost) * quantity
                } else {
                    (cost - account_price) * quantity.abs()
                };
            }
        }

        let equity = cash + market_value;
        let leverage = if equity > 0.0 {
            gross_exposure / equity
        } else {
            f64::INFINITY
        };
        let leverage_cap = self.effective_leverage_cap();
        let buying_power = if equity > 0.0 && leverage_cap.is_finite() {
            (equity * leverage_cap - gross_exposure).max(0.0)
        } else if equity > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        let drawdown = if self.peak_equity > 0.0 {
            (equity - self.peak_equity) / self.peak_equity
        } else {
            0.0
        };
        let selected = self
            .config
            .metrics
            .iter()
            .filter(|key| !matches!(key.as_str(), "alpha" | "excess_return"))
            .cloned()
            .collect::<Vec<_>>();
        let metrics = compute_builtin_metrics(
            &selected,
            self.config.initial_cash,
            self.config.risk_free_rate,
            &self.equity_curve,
            &self.trades,
        );

        SessionSnapshot {
            portfolio: self.portfolio.clone(),
            latest_prices: self.latest_prices.clone(),
            equity,
            realized_pnl: self.realized_pnl,
            unrealized_pnl,
            processed_bars: self.processed_bars,
            gross_exposure,
            net_exposure: market_value,
            leverage,
            buying_power,
            drawdown,
            peak_equity: self.peak_equity,
            total_costs: self.total_costs,
            trading_halted: self.trading_halted,
            halt_reason: self.halt_reason.clone(),
            metrics,
        }
    }

    /// Record the authoritative account sample after processing an eligible update.
    pub(crate) fn finish_update(&mut self, timestamp: i64) {
        let snapshot = self.snapshot();
        self.peak_equity = self.peak_equity.max(snapshot.equity);
        let drawdown = if self.peak_equity > 0.0 {
            (snapshot.equity - self.peak_equity) / self.peak_equity
        } else {
            0.0
        };
        self.equity_curve.push(EquitySample {
            timestamp,
            equity: snapshot.equity,
            cash: snapshot.portfolio.cash,
            drawdown,
        });
        if self.equity_curve.len() > self.config.max_history {
            self.equity_curve.remove(0);
        }
        if self.config.max_drawdown > 0.0 && -drawdown * 100.0 >= self.config.max_drawdown {
            self.trading_halted = true;
            self.halt_reason =
                Some(format!("maximum drawdown {:.2}% reached", self.config.max_drawdown));
        }
    }

    fn effective_leverage_cap(&self) -> f64 {
        if !self.config.allow_margin {
            return 1.0;
        }
        let leverage = if self.config.max_leverage > 0.0 {
            self.config.max_leverage
        } else {
            f64::INFINITY
        };
        let initial_margin = if self.config.initial_margin > 0.0 {
            100.0 / self.config.initial_margin
        } else {
            f64::INFINITY
        };
        leverage.min(initial_margin)
    }

    fn current_gross_exposure(&self) -> f64 {
        self.portfolio
            .positions
            .iter()
            .filter_map(|(symbol, quantity)| {
                let price = self.latest_prices.get(symbol)?;
                let timestamp = self.latest_price_timestamps.get(symbol)?;
                self.account_price(symbol, *price, *timestamp)
                    .map(|account_price| quantity.abs() * account_price)
            })
            .sum()
    }

    fn quote_currency(&self, symbol: &str) -> String {
        self.quote_currencies
            .get(symbol)
            .cloned()
            .unwrap_or_else(|| self.config.base_currency.to_string())
    }

    fn account_price(&self, symbol: &str, quote_price: f64, timestamp: i64) -> Option<f64> {
        let quote_currency = self.quote_currency(symbol);
        self.fx
            .convert(
                quote_price,
                &quote_currency,
                &self.config.base_currency.to_string(),
                timestamp,
            )
            .filter(|price| price.is_finite() && *price > 0.0)
    }

    fn accrue_financing(&mut self, timestamp: i64) {
        let Some(previous) = self.last_accrual_ts.replace(timestamp) else {
            return;
        };
        let elapsed = timestamp.saturating_sub(previous) as f64;
        if elapsed <= 0.0 {
            return;
        }
        let year_fraction = elapsed / (365.25 * 24.0 * 60.0 * 60.0);
        let cash = self.portfolio.cash.amount(&self.config.base_currency);
        let borrowed = (-cash).max(0.0);
        let short_notional: f64 = self
            .portfolio
            .positions
            .iter()
            .filter(|(_, quantity)| **quantity < 0.0)
            .filter_map(|(symbol, quantity)| {
                let price = self.latest_prices.get(symbol)?;
                self.account_price(symbol, *price, timestamp)
                    .map(|account_price| quantity.abs() * account_price)
            })
            .sum();
        let cost = borrowed * self.config.margin_interest / 100.0 * year_fraction
            + short_notional * self.config.borrow_rate / 100.0 * year_fraction;
        if cost <= 0.0 || !cost.is_finite() {
            return;
        }
        self.portfolio.cash.insert(self.config.base_currency, cash - cost);
        self.realized_pnl -= cost;
        self.total_costs += cost;
    }

    fn enforce_maintenance_margin(&mut self, timestamp: i64, fills: &mut Vec<SessionFill>) {
        if !self.config.allow_margin || self.config.maintenance_margin <= 0.0 {
            return;
        }
        let snapshot = self.snapshot();
        if snapshot.gross_exposure <= 0.0
            || snapshot.equity / snapshot.gross_exposure * 100.0 >= self.config.maintenance_margin
        {
            return;
        }
        self.trading_halted = true;
        self.halt_reason = Some(format!(
            "maintenance margin {:.2}% breached; positions liquidated",
            self.config.maintenance_margin
        ));
        let positions = self
            .portfolio
            .positions
            .iter()
            .map(|(symbol, quantity)| (symbol.clone(), *quantity))
            .collect::<Vec<_>>();
        for (symbol, quantity) in positions {
            let Some(price) = self.latest_prices.get(&symbol).copied() else {
                continue;
            };
            let order = Order {
                id: OrderId::new(),
                symbol,
                quantity: -quantity,
                order_type: OrderType::Market,
                price: None,
                limit_price: None,
                sizer: None,
            };
            let (fill, _) = self.execute(
                order,
                timestamp,
                price,
                None,
                "maintenance-margin liquidation".to_owned(),
                false,
                None,
            );
            fills.push(fill);
        }
        self.portfolio.orders.clear();
        self.trail_state.clear();
    }

    fn match_resting_orders(
        &mut self,
        symbol: &str,
        bar: &crate::data::models::Bar,
        fills: &mut Vec<SessionFill>,
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
                } => {
                    let (fill, remainder) = self.execute(
                        order,
                        bar.open_ts as i64,
                        raw_px,
                        limit_cap,
                        reason,
                        false,
                        Some(bar.volume),
                    );
                    fills.push(fill);
                    if let Some(remainder) = remainder {
                        still_open.push(remainder);
                    }
                },
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

    fn submit_order(
        &mut self,
        order: Order,
        market: &MarketUpdate,
        fills: &mut Vec<SessionFill>,
        fit_buys_to_cash: bool,
    ) {
        if order.order_type != OrderType::Cancel
            && !self.config.allowed_order_types.contains(&order.order_type)
        {
            fills.push(rejected_fill(
                order,
                market.close_ts as i64,
                "order type is disabled for this session".to_owned(),
            ));
            return;
        }
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
            let (fill, remainder) = self.execute(
                order,
                market.close_ts as i64,
                market.close,
                None,
                "live market fill".to_owned(),
                fit_buys_to_cash,
                Some(market.volume),
            );
            fills.push(fill);
            if let Some(remainder) = remainder {
                self.portfolio.orders.push(remainder);
            }
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
        mut reason: String,
        fit_buy_to_cash: bool,
        available_volume: Option<f64>,
    ) -> (SessionFill, Option<Order>) {
        if !raw_price.is_finite() || raw_price <= 0.0 {
            return (
                rejected_fill(order, timestamp, "invalid fill price or quantity".to_owned()),
                None,
            );
        }
        let quote_currency = self.quote_currency(&order.symbol);
        let base_currency = self.config.base_currency.to_string();
        let Some(quote_to_base_rate) = self
            .fx
            .rate(&quote_currency, &base_currency, timestamp)
            .filter(|rate| rate.is_finite() && *rate > 0.0)
        else {
            return (
                rejected_fill(
                    order,
                    timestamp,
                    format!(
                        "missing {quote_currency}/{base_currency} conversion rate for live accounting"
                    ),
                ),
                None,
            );
        };

        if let Some(sizer) = order.sizer.take() {
            let stop_distance = order.price.and_then(|price| {
                let distance = (raw_price - price).abs();
                (distance > 0.0).then_some(distance)
            });
            let quantity = match sizer {
                SizerSlot::Builtin(builtin) => {
                    let capital = if builtin.uses_cash_capital() {
                        self.portfolio.cash.amount(&self.config.base_currency) / quote_to_base_rate
                    } else {
                        self.snapshot().equity / quote_to_base_rate
                    };
                    builtin.calculate(capital, raw_price, stop_distance, None)
                },
                SizerSlot::Custom(custom) => Python::attach(|py| -> PyResult<f64> {
                    let equity = self.snapshot().equity / quote_to_base_rate;
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
                    return (
                        rejected_fill(order, timestamp, format!("sizer failed: {error}")),
                        None,
                    );
                },
            }
        }

        let requested_quantity = order.quantity;
        let mut volume_limited = false;
        if self.config.partial_fills {
            if let Some(volume) = available_volume.filter(|value| value.is_finite() && *value > 0.0)
            {
                let maximum = volume * self.config.max_volume_participation / 100.0;
                if order.quantity.abs() > maximum && is_significant(order.quantity.abs() - maximum)
                {
                    order.quantity = order.quantity.signum() * maximum;
                    volume_limited = true;
                    reason.push_str("; quantity capped by candle-volume participation");
                }
            }
        }

        if !order.quantity.is_finite() || is_negligible(order.quantity) {
            return (
                rejected_fill(order, timestamp, "invalid fill price or quantity".to_owned()),
                None,
            );
        }

        let fill_price = apply_slippage(raw_price, order.quantity, self.config.slippage, limit_cap);
        let account_fill_price = fill_price * quote_to_base_rate;
        let old_quantity = self.portfolio.positions.amount(&order.symbol);
        let mut new_quantity = old_quantity + order.quantity;

        if !self.config.allow_short && new_quantity < 0.0 && is_significant(new_quantity) {
            return (rejected_fill(order, timestamp, "short selling is disabled".to_owned()), None);
        }

        let mut notional = account_fill_price * order.quantity.abs();
        let mut commission =
            notional * self.config.commission_pct / 100.0 + self.config.commission_fixed;
        let cash = self.portfolio.cash.amount(&self.config.base_currency);
        let mut next_cash = cash - order.quantity * account_fill_price - commission;
        if fit_buy_to_cash
            && order.quantity > 0.0
            && !self.config.allow_margin
            && cash_deficit_is_significant(next_cash, cash)
        {
            let price_with_variable_fee =
                account_fill_price * (1.0 + self.config.commission_pct / 100.0);
            let available_cash = cash - self.config.commission_fixed;
            let affordable_quantity = available_cash / price_with_variable_fee;
            if affordable_quantity.is_finite()
                && affordable_quantity > 0.0
                && is_significant(affordable_quantity)
            {
                order.quantity = order.quantity.min(affordable_quantity);
                notional = account_fill_price * order.quantity;
                commission =
                    notional * self.config.commission_pct / 100.0 + self.config.commission_fixed;
                next_cash = cash - notional - commission;
                new_quantity = old_quantity + order.quantity;
                reason.push_str("; quantity reduced to fit available cash");
            }
        }
        let increasing_exposure = new_quantity.abs() > old_quantity.abs();
        if increasing_exposure {
            if self.trading_halted {
                return (
                    rejected_fill(
                        order,
                        timestamp,
                        self.halt_reason
                            .clone()
                            .unwrap_or_else(|| "trading is halted by a risk control".to_owned()),
                    ),
                    None,
                );
            }
            let equity = self.snapshot().equity;
            if equity <= 0.0 || !equity.is_finite() {
                return (
                    rejected_fill(order, timestamp, "account equity is not positive".to_owned()),
                    None,
                );
            }
            let position_notional = new_quantity.abs() * account_fill_price;
            let position_cap = equity * self.config.max_position_size / 100.0;
            if position_notional > position_cap && is_significant(position_notional - position_cap)
            {
                return (
                    rejected_fill(
                        order,
                        timestamp,
                        format!(
                            "order would exceed max_position_size ({:.2}% of equity)",
                            self.config.max_position_size
                        ),
                    ),
                    None,
                );
            }
            let current_symbol_notional = old_quantity.abs() * account_fill_price;
            let proposed_gross =
                self.current_gross_exposure() - current_symbol_notional + position_notional;
            let leverage_cap = self.effective_leverage_cap();
            if proposed_gross > equity * leverage_cap
                && is_significant(proposed_gross - equity * leverage_cap)
            {
                return (
                    rejected_fill(
                        order,
                        timestamp,
                        format!("order would exceed maximum leverage ({leverage_cap:.2}x)"),
                    ),
                    None,
                );
            }
        }
        if !self.config.allow_margin && cash_deficit_is_significant(next_cash, cash) {
            return (rejected_fill(order, timestamp, "insufficient cash".to_owned()), None);
        }

        let remainder =
            if volume_limited && is_significant(requested_quantity.abs() - order.quantity.abs()) {
                let mut remainder = order.clone();
                remainder.quantity = requested_quantity - order.quantity;
                Some(remainder)
            } else {
                None
            };

        self.portfolio.cash.insert(self.config.base_currency, normalize_cash(next_cash, cash));
        if is_negligible(new_quantity) {
            self.portfolio.positions.remove(&order.symbol);
        } else {
            self.portfolio.positions.insert(order.symbol.clone(), new_quantity);
        }

        let pnl = self.update_cost_basis(
            &order.symbol,
            old_quantity,
            order.quantity,
            account_fill_price,
            commission,
            timestamp,
        );
        self.total_costs += commission;
        self.trail_state.remove(&order.id);

        (
            SessionFill {
                order,
                timestamp,
                status: OrderStatus::Filled,
                fill_price: Some(fill_price),
                commission,
                realized_pnl: Some(pnl),
                reason,
            },
            remainder,
        )
    }

    fn update_cost_basis(
        &mut self,
        symbol: &str,
        old_quantity: f64,
        delta: f64,
        price: f64,
        commission: f64,
        timestamp: i64,
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
            self.entry_timestamps.entry(symbol.to_owned()).or_insert(timestamp);
        } else {
            let closed = old_quantity.abs().min(delta.abs());
            realized += if old_quantity > 0.0 {
                (price - old_cost) * closed
            } else {
                (old_cost - price) * closed
            };
            self.trades.push(Trade {
                symbol: symbol.to_owned(),
                quantity: old_quantity.signum() * closed,
                entry_ts: self.entry_timestamps.get(symbol).copied().unwrap_or(timestamp),
                exit_ts: timestamp,
                entry_price: old_cost,
                exit_price: price,
                pnl: realized,
            });
            if self.trades.len() > self.config.max_history {
                self.trades.remove(0);
            }

            if is_negligible(new_quantity) {
                self.average_cost.remove(symbol);
                self.entry_timestamps.remove(symbol);
            } else if new_quantity.signum() != old_quantity.signum() {
                self.average_cost.insert(symbol.to_owned(), price);
                self.entry_timestamps.insert(symbol.to_owned(), timestamp);
            }
        }

        self.realized_pnl += realized;
        realized
    }
}

fn cash_tolerance(reference: f64) -> f64 {
    f64::EPSILON * reference.abs().max(1.0) * 16.0
}

fn cash_deficit_is_significant(next_cash: f64, reference: f64) -> bool {
    next_cash < -cash_tolerance(reference)
}

fn normalize_cash(next_cash: f64, reference: f64) -> f64 {
    if next_cash.abs() <= cash_tolerance(reference) {
        0.0
    } else {
        next_cash
    }
}

fn validate_config(config: &SessionConfig) -> Result<(), String> {
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
    if !config.max_leverage.is_finite() || config.max_leverage < 1.0 {
        return Err("max_leverage must be finite and at least 1".to_owned());
    }
    if !config.initial_margin.is_finite()
        || config.initial_margin < 0.0
        || config.initial_margin > 100.0
    {
        return Err("initial_margin must be between 0 and 100".to_owned());
    }
    if !config.maintenance_margin.is_finite()
        || config.maintenance_margin < 0.0
        || config.maintenance_margin > 100.0
    {
        return Err("maintenance_margin must be between 0 and 100".to_owned());
    }
    if config.maintenance_margin > config.initial_margin && config.initial_margin > 0.0 {
        return Err("maintenance_margin cannot exceed initial_margin".to_owned());
    }
    for (name, value) in [
        ("margin_interest", config.margin_interest),
        ("borrow_rate", config.borrow_rate),
        ("max_drawdown", config.max_drawdown),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!("{name} must be finite and non-negative"));
        }
    }
    if !config.max_position_size.is_finite()
        || config.max_position_size <= 0.0
        || config.max_position_size > 100.0
    {
        return Err("max_position_size must be between 0 and 100".to_owned());
    }
    if config.allowed_order_types.is_empty() {
        return Err("allowed_order_types must not be empty".to_owned());
    }
    if !config.max_volume_participation.is_finite()
        || config.max_volume_participation <= 0.0
        || config.max_volume_participation > 100.0
    {
        return Err("max_volume_participation must be between 0 and 100".to_owned());
    }
    if !config.risk_free_rate.is_finite() {
        return Err("risk_free_rate must be finite".to_owned());
    }
    for metric in config.metrics.iter() {
        if !is_builtin_metric(metric) && !config.metrics.has_implementation(metric) {
            return Err(format!("custom live metric {metric:?} requires a Python metric instance"));
        }
    }
    Ok(())
}

fn canceled_fill(order: Order, timestamp: i64, reason: String) -> SessionFill {
    SessionFill {
        order,
        timestamp,
        status: OrderStatus::Canceled,
        fill_price: None,
        commission: 0.0,
        realized_pnl: None,
        reason,
    }
}

fn rejected_fill(order: Order, timestamp: i64, reason: String) -> SessionFill {
    SessionFill {
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
    use crate::backtest::models::{BuiltinSizer, Order, OrderId};
    use crate::data::models::Currency;
    use pyo3::types::PyModule;

    fn update(close: f64, timestamp: u64) -> MarketUpdate {
        MarketUpdate {
            provider: "mock".to_owned(),
            symbol: "BTC-USD".to_owned(),
            quote_currency: Some("USD".to_owned()),
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

    fn market_for(symbol: &str, quantity: f64) -> Order {
        Order {
            symbol: symbol.to_owned(),
            ..market(quantity)
        }
    }

    fn update_for(symbol: &str, close: f64, timestamp: u64) -> MarketUpdate {
        MarketUpdate {
            symbol: symbol.to_owned(),
            ..update(close, timestamp)
        }
    }

    fn custom_sizer(raises: bool) -> Py<PyAny> {
        Python::attach(|py| {
            let module = PyModule::from_code(
                py,
                pyo3::ffi::c_str!(
                    r#"
class Sizer:
    def __init__(self, raises):
        self.raises = raises
    def calculate(self, equity, price, stop_distance, atr):
        if self.raises:
            raise RuntimeError("deliberate sizer error")
        return 2.0
"#
                ),
                pyo3::ffi::c_str!("live_engine_sizer.py"),
                pyo3::ffi::c_str!("live_engine_sizer"),
            )
            .unwrap();
            module.getattr("Sizer").unwrap().call1((raises,)).unwrap().unbind()
        })
    }

    #[test]
    fn market_buy_updates_cash_position_and_equity() {
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
        let result = broker.process(update(100.0, 1_000), vec![market(10.0)]);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].status, OrderStatus::Filled);
        assert_eq!(result.snapshot.portfolio.positions["BTC-USD"], 10.0);
        assert_eq!(result.snapshot.portfolio.cash[&Currency::USD], 99_000.0);
        assert_eq!(result.snapshot.equity, 100_000.0);
    }

    #[test]
    fn closing_trade_realizes_pnl() {
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
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
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
        let result = broker.process(partial, vec![market(1.0)]);

        assert!(!result.processed);
        assert!(result.fills.is_empty());
        assert_eq!(result.snapshot.processed_bars, 0);
    }

    #[test]
    fn limit_order_rests_until_reached() {
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
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
        let config = SessionConfig {
            initial_cash: 100.0,
            ..SessionConfig::default()
        };
        let mut broker = SessionBroker::new(config).unwrap();

        let short = broker.process(update(10.0, 1_000), vec![market(-1.0)]);
        assert_eq!(short.fills[0].status, OrderStatus::Rejected);
        let margin = broker.process(update(10.0, 1_060), vec![market(20.0)]);
        assert_eq!(margin.fills[0].status, OrderStatus::Rejected);
        assert_eq!(margin.snapshot.equity, 100.0);
    }

    #[test]
    fn duplicate_and_stale_final_bars_are_not_processed_twice() {
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
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

    #[test]
    fn leverage_limit_applies_across_symbols() {
        let config = SessionConfig {
            initial_cash: 100.0,
            allow_margin: true,
            ..SessionConfig::default()
        };
        let mut broker = SessionBroker::new(config).unwrap();
        broker.process(update(10.0, 1_000), vec![market(10.0)]);
        broker.process(update_for("ETH-USD", 10.0, 1_060), vec![market_for("ETH-USD", 10.0)]);

        let result =
            broker.process(update_for("SOL-USD", 1.0, 1_120), vec![market_for("SOL-USD", 1.0)]);

        assert_eq!(result.fills[0].status, OrderStatus::Rejected);
        assert!(result.fills[0].reason.contains("maximum leverage"));
        assert_eq!(result.snapshot.gross_exposure, 200.0);
    }

    #[test]
    fn maintenance_margin_breach_liquidates_and_halts() {
        let config = SessionConfig {
            initial_cash: 100.0,
            allow_margin: true,
            ..SessionConfig::default()
        };
        let mut broker = SessionBroker::new(config).unwrap();
        broker.process(update(10.0, 1_000), vec![market(10.0)]);
        broker.process(update_for("ETH-USD", 10.0, 1_060), vec![market_for("ETH-USD", 10.0)]);

        let result = broker.process(update(1.0, 1_120), Vec::new());

        assert!(result.snapshot.portfolio.positions.is_empty());
        assert!(result.snapshot.trading_halted);
        assert!(result
            .snapshot
            .halt_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("maintenance margin")));
        assert_eq!(result.fills.len(), 2);
        assert!(result
            .fills
            .iter()
            .all(|fill| fill.reason.contains("maintenance-margin liquidation")));
    }

    #[test]
    fn partial_fill_respects_volume_participation() {
        let config = SessionConfig {
            partial_fills: true,
            max_volume_participation: 10.0,
            ..SessionConfig::default()
        };
        let mut broker = SessionBroker::new(config).unwrap();

        let result = broker.process(update(100.0, 1_000), vec![market(1.0)]);

        assert_eq!(result.fills[0].status, OrderStatus::Filled);
        assert!((result.fills[0].order.quantity - 0.1).abs() < 1e-12);
        assert!(result.fills[0].reason.contains("candle-volume participation"));
        assert_eq!(result.snapshot.portfolio.orders.len(), 1);
        assert!((result.snapshot.portfolio.orders[0].quantity - 0.9).abs() < 1e-12);

        let next = broker.process(update(101.0, 1_060), Vec::new());
        assert!((next.fills[0].order.quantity - 0.1).abs() < 1e-12);
        assert!((next.snapshot.portfolio.orders[0].quantity - 0.8).abs() < 1e-12);
    }

    #[test]
    fn selected_metrics_update_after_a_round_trip() {
        let config = SessionConfig {
            metrics: vec!["total_return".to_owned(), "pnl".to_owned(), "n_trades".to_owned()]
                .into(),
            ..SessionConfig::default()
        };
        let mut broker = SessionBroker::new(config).unwrap();
        broker.process(update(100.0, 1_000), vec![market(10.0)]);

        let result = broker.process(update(110.0, 1_060), vec![market(-10.0)]);

        assert_eq!(result.snapshot.metrics["pnl"], 100.0);
        assert_eq!(result.snapshot.metrics["n_trades"], 1.0);
        assert!((result.snapshot.metrics["total_return"] - 0.001).abs() < 1e-12);
    }

    #[test]
    fn maximum_drawdown_halts_new_risk() {
        let config = SessionConfig {
            initial_cash: 100.0,
            max_drawdown: 10.0,
            ..SessionConfig::default()
        };
        let mut broker = SessionBroker::new(config).unwrap();
        broker.process(update(10.0, 1_000), vec![market(10.0)]);

        let drawdown = broker.process(update(8.0, 1_060), Vec::new());
        assert!(drawdown.snapshot.trading_halted);
        assert!((drawdown.snapshot.drawdown + 0.2).abs() < 1e-12);

        let rejected =
            broker.process(update_for("ETH-USD", 1.0, 1_120), vec![market_for("ETH-USD", 1.0)]);
        assert_eq!(rejected.fills[0].status, OrderStatus::Rejected);
        assert!(rejected.fills[0].reason.contains("maximum drawdown"));
    }

    #[test]
    fn borrowed_cash_accrues_configured_margin_interest() {
        let config = SessionConfig {
            initial_cash: 100.0,
            allow_margin: true,
            margin_interest: 10.0,
            ..SessionConfig::default()
        };
        let mut broker = SessionBroker::new(config).unwrap();
        broker.process(update(10.0, 1_000), vec![market(10.0)]);
        broker.process(update_for("ETH-USD", 10.0, 1_060), vec![market_for("ETH-USD", 10.0)]);

        let one_year_later = 1_060 + (365.25 * 24.0 * 60.0 * 60.0) as u64;
        let result = broker.process(update(10.0, one_year_later), Vec::new());

        assert!((result.snapshot.total_costs - 10.0).abs() < 1e-6);
        assert!((result.snapshot.equity - 90.0).abs() < 1e-6);
    }

    #[test]
    fn invalid_risk_configuration_is_rejected() {
        let config = SessionConfig {
            initial_margin: 20.0,
            maintenance_margin: 25.0,
            ..SessionConfig::default()
        };

        assert_eq!(
            SessionBroker::new(config).unwrap_err(),
            "maintenance_margin cannot exceed initial_margin"
        );
    }

    #[test]
    fn invalid_configuration_reports_every_validation_family() {
        let invalid = [
            (
                SessionConfig {
                    initial_cash: -1.0,
                    ..SessionConfig::default()
                },
                "initial_cash",
            ),
            (
                SessionConfig {
                    commission_pct: f64::NAN,
                    ..SessionConfig::default()
                },
                "commission_pct",
            ),
            (
                SessionConfig {
                    commission_fixed: -1.0,
                    ..SessionConfig::default()
                },
                "commission_fixed",
            ),
            (
                SessionConfig {
                    slippage: -1.0,
                    ..SessionConfig::default()
                },
                "slippage",
            ),
            (
                SessionConfig {
                    max_history: 0,
                    ..SessionConfig::default()
                },
                "max_history",
            ),
            (
                SessionConfig {
                    max_leverage: 0.5,
                    ..SessionConfig::default()
                },
                "max_leverage",
            ),
            (
                SessionConfig {
                    initial_margin: 101.0,
                    ..SessionConfig::default()
                },
                "initial_margin",
            ),
            (
                SessionConfig {
                    maintenance_margin: -1.0,
                    ..SessionConfig::default()
                },
                "maintenance_margin",
            ),
            (
                SessionConfig {
                    margin_interest: -1.0,
                    ..SessionConfig::default()
                },
                "margin_interest",
            ),
            (
                SessionConfig {
                    borrow_rate: -1.0,
                    ..SessionConfig::default()
                },
                "borrow_rate",
            ),
            (
                SessionConfig {
                    max_drawdown: -1.0,
                    ..SessionConfig::default()
                },
                "max_drawdown",
            ),
            (
                SessionConfig {
                    max_position_size: 0.0,
                    ..SessionConfig::default()
                },
                "max_position_size",
            ),
            (
                SessionConfig {
                    allowed_order_types: vec![],
                    ..SessionConfig::default()
                },
                "allowed_order_types",
            ),
            (
                SessionConfig {
                    max_volume_participation: 101.0,
                    ..SessionConfig::default()
                },
                "max_volume_participation",
            ),
            (
                SessionConfig {
                    risk_free_rate: f64::INFINITY,
                    ..SessionConfig::default()
                },
                "risk_free_rate",
            ),
            (
                SessionConfig {
                    metrics: vec!["custom".to_owned()].into(),
                    ..SessionConfig::default()
                },
                "requires a Python metric",
            ),
        ];

        for (config, message) in invalid {
            assert!(SessionBroker::new(config).unwrap_err().contains(message));
        }
    }

    #[test]
    fn exchange_rates_accessors_and_history_bounds_are_exercised() {
        let mut broker = SessionBroker::new(SessionConfig {
            max_history: 2,
            ..SessionConfig::default()
        })
        .unwrap();
        assert!(broker.set_exchange_rate("", "USD", 1.0, 0).unwrap_err().contains("non-empty"));
        assert!(broker.set_exchange_rate("EUR", "USD", 0.0, 0).unwrap_err().contains("positive"));
        broker.set_exchange_rate(" eur ", " usd ", 1.1, 0).unwrap();
        assert!(broker.has_exchange_rate("EUR", 0));
        assert!(!broker.has_exchange_rate("GBP", 0));
        assert!(broker.portfolio().positions.is_empty());
        assert!(broker.equity_curve().is_empty());
        assert!(broker.trades().is_empty());

        for (index, price) in [100.0, 101.0, 102.0].into_iter().enumerate() {
            broker.process(update(price, 1_000 + index as u64 * 60), Vec::new());
        }
        assert_eq!(broker.equity_curve().len(), 2);
    }

    #[test]
    fn order_submission_rejects_disabled_duplicate_invalid_and_missing_cancel_targets() {
        let mut broker = SessionBroker::new(SessionConfig {
            allowed_order_types: vec![OrderType::Market],
            ..SessionConfig::default()
        })
        .unwrap();
        let market_update = update(100.0, 1_000);

        let mut limit = market(1.0);
        limit.order_type = OrderType::Limit;
        let disabled = broker.process(market_update.clone(), vec![limit]);
        assert!(disabled.fills[0].reason.contains("disabled"));

        let mut cancel = market(0.0);
        cancel.order_type = OrderType::Cancel;
        let missing = broker.process(update(100.0, 1_060), vec![cancel]);
        assert!(missing.fills[0].reason.contains("not open"));

        let order = market(1.0);
        let duplicate = order.clone();
        let result = broker.process(update(100.0, 1_120), vec![order, duplicate]);
        assert!(result.fills.iter().any(|fill| fill.reason.contains("duplicate order id")));

        let mut invalid = market(f64::NAN);
        invalid.symbol.clear();
        let result = broker.process(update(100.0, 1_180), vec![invalid]);
        assert!(result.fills[0].reason.contains("symbol and finite"));
    }

    #[test]
    fn resting_orders_can_fill_cancel_or_remain_for_another_symbol() {
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
        let eth = market_for("ETH-USD", 1.0);
        let first = broker.process(update(100.0, 1_000), vec![eth.clone()]);
        assert_eq!(first.snapshot.portfolio.orders.len(), 1);

        let mut invalid_limit = market(1.0);
        invalid_limit.order_type = OrderType::Limit;
        let second = broker.process(update(100.0, 1_060), vec![invalid_limit]);
        assert!(second.snapshot.portfolio.orders.len() >= 2);

        let third = broker.process(update(100.0, 1_120), Vec::new());
        assert!(third.fills.iter().any(|fill| fill.status == OrderStatus::Canceled));
        assert!(third.snapshot.portfolio.orders.iter().any(|order| order.symbol == "ETH-USD"));

        let mut cancel = market_for("ETH-USD", 0.0);
        cancel.order_type = OrderType::Cancel;
        cancel.id = eth.id;
        let canceled = broker.process(update(100.0, 1_180), vec![cancel]);
        assert!(canceled.fills.iter().any(|fill| fill.status == OrderStatus::Canceled));
    }

    #[test]
    fn execution_handles_conversion_invalid_values_and_python_sizers() {
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
        broker.quote_currencies.insert("BTC-USD".to_owned(), "EUR".to_owned());
        let (missing_rate, _) =
            broker.execute(market(1.0), 1_000, 100.0, None, String::new(), false, None);
        assert!(missing_rate.reason.contains("missing EUR/USD"));

        broker.quote_currencies.insert("BTC-USD".to_owned(), "USD".to_owned());
        let (invalid_price, _) =
            broker.execute(market(1.0), 1_000, f64::NAN, None, String::new(), false, None);
        assert_eq!(invalid_price.status, OrderStatus::Rejected);
        let (zero, _) = broker.execute(market(0.0), 1_000, 100.0, None, String::new(), false, None);
        assert_eq!(zero.status, OrderStatus::Rejected);

        let mut sized = market(0.0);
        sized.sizer = Some(SizerSlot::Custom(custom_sizer(false)));
        let (filled, _) = broker.execute(sized, 1_000, 100.0, None, String::new(), false, None);
        assert_eq!(filled.order.quantity, 2.0);

        let mut failed = market(0.0);
        failed.sizer = Some(SizerSlot::Custom(custom_sizer(true)));
        let (rejected, _) = broker.execute(failed, 1_000, 100.0, None, String::new(), false, None);
        assert!(rejected.reason.contains("sizer failed"));

        let mut built_in = market(0.0);
        built_in.sizer = Some(SizerSlot::Builtin(BuiltinSizer::FixedQuantity(
            crate::sizers::FixedQuantity::new(1.0),
        )));
        let (filled, _) = broker.execute(built_in, 1_000, 100.0, None, String::new(), false, None);
        assert_eq!(filled.order.quantity, 1.0);
    }

    #[test]
    fn snapshot_handles_missing_marks_shorts_and_non_positive_equity() {
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
        broker.portfolio.positions.insert("UNMARKED".to_owned(), 2.0);
        assert_eq!(broker.snapshot().gross_exposure, 0.0);

        broker.latest_prices.insert("UNMARKED".to_owned(), 10.0);
        broker.latest_price_timestamps.insert("UNMARKED".to_owned(), 1_000);
        broker.quote_currencies.insert("UNMARKED".to_owned(), "EUR".to_owned());
        assert_eq!(broker.snapshot().gross_exposure, 0.0);

        broker.portfolio.positions.clear();
        broker.config.allow_short = true;
        broker.config.allow_margin = true;
        broker.process(update(100.0, 1_000), vec![market(-2.0)]);
        let marked = broker.process(update(90.0, 1_060), Vec::new()).snapshot;
        assert_eq!(marked.unrealized_pnl, 20.0);

        broker.portfolio.cash.insert(Currency::USD, -1_000.0);
        broker.portfolio.positions.clear();
        broker.peak_equity = 0.0;
        broker.config.max_leverage = 0.0;
        broker.config.initial_margin = 0.0;
        let insolvent = broker.snapshot();
        assert!(insolvent.leverage.is_infinite());
        assert_eq!(insolvent.buying_power, 0.0);
        assert_eq!(insolvent.drawdown, 0.0);

        broker.portfolio.cash.insert(Currency::USD, 1_000.0);
        let unlimited = broker.snapshot();
        assert!(unlimited.buying_power.is_infinite());
        assert!(broker.effective_leverage_cap().is_infinite());

        broker.portfolio.cash.insert(Currency::USD, 0.0);
        broker.peak_equity = 0.0;
        broker.finish_update(2_000);
        assert_eq!(broker.equity_curve.last().unwrap().drawdown, 0.0);
    }

    #[test]
    fn financing_and_margin_checks_skip_unpriced_or_duplicate_timestamps() {
        let mut broker = SessionBroker::new(SessionConfig {
            allow_short: true,
            allow_margin: true,
            margin_interest: 5.0,
            borrow_rate: 5.0,
            ..SessionConfig::default()
        })
        .unwrap();
        broker.portfolio.positions.insert("UNMARKED".to_owned(), -1.0);
        broker.accrue_financing(1_000);
        let cash = broker.portfolio.cash[&Currency::USD];
        broker.accrue_financing(1_000);
        assert_eq!(broker.portfolio.cash[&Currency::USD], cash);

        let mut fills = Vec::new();
        broker.config.maintenance_margin = 101.0;
        broker.enforce_maintenance_margin(1_000, &mut fills);
        assert!(fills.is_empty());

        broker.portfolio.cash.insert(Currency::USD, 0.0);
        broker.portfolio.positions.insert("MARKED".to_owned(), 1.0);
        broker.latest_prices.insert("MARKED".to_owned(), 1.0);
        broker.latest_price_timestamps.insert("MARKED".to_owned(), 1_000);
        broker.quote_currencies.insert("MARKED".to_owned(), "USD".to_owned());
        broker.enforce_maintenance_margin(1_000, &mut fills);
        assert_eq!(fills.len(), 1);
        assert!(broker.portfolio.positions.contains_key("UNMARKED"));
    }

    #[test]
    fn execution_rejects_non_positive_equity_and_fixed_commission_cash_deficits() {
        let mut insolvent = SessionBroker::new(SessionConfig::default()).unwrap();
        insolvent.portfolio.cash.insert(Currency::USD, -1.0);
        let (fill, _) =
            insolvent.execute(market(1.0), 1_000, 1.0, None, String::new(), false, None);
        assert!(fill.reason.contains("equity is not positive"));

        let mut cash_limited = SessionBroker::new(SessionConfig {
            initial_cash: 100.0,
            commission_fixed: 1.0,
            ..SessionConfig::default()
        })
        .unwrap();
        let (fill, _) =
            cash_limited.execute(market(10.0), 1_000, 10.0, None, String::new(), false, None);
        assert!(fill.reason.contains("insufficient cash"));

        assert_eq!(cash_limited.update_cost_basis("BTC-USD", 0.0, 0.0, 10.0, 0.0, 1_000), 0.0);
    }

    #[test]
    fn resting_limit_can_remain_pending_across_multiple_matching_bars() {
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
        let mut order = market(1.0);
        order.order_type = OrderType::Limit;
        order.price = Some(50.0);
        broker.process(update(100.0, 1_000), vec![order]);

        let first = broker.process(update(90.0, 1_060), Vec::new());
        let second = broker.process(update(80.0, 1_120), Vec::new());

        assert!(first.fills.is_empty());
        assert!(second.fills.is_empty());
        assert_eq!(second.snapshot.portfolio.orders.len(), 1);
    }

    #[test]
    fn cost_basis_tracks_short_profit_reversals_and_history_eviction() {
        let mut broker = SessionBroker::new(SessionConfig {
            allow_short: true,
            allow_margin: true,
            max_history: 1,
            ..SessionConfig::default()
        })
        .unwrap();

        broker.process(update(100.0, 1_000), vec![market(-2.0)]);
        let covered = broker.process(update(90.0, 1_060), vec![market(3.0)]);
        assert_eq!(covered.snapshot.portfolio.positions["BTC-USD"], 1.0);
        assert_eq!(covered.snapshot.realized_pnl, 20.0);

        broker.process(update(95.0, 1_120), vec![market(-1.0)]);
        broker.process(update(96.0, 1_180), vec![market(1.0)]);
        broker.process(update(97.0, 1_240), vec![market(-1.0)]);
        assert_eq!(broker.trades().len(), 1);
    }

    #[test]
    fn custom_sizer_receives_a_stop_distance() {
        let mut broker = SessionBroker::new(SessionConfig::default()).unwrap();
        let mut order = market(0.0);
        order.price = Some(90.0);
        order.sizer = Some(SizerSlot::Custom(custom_sizer(false)));

        let (filled, _) = broker.execute(order, 1_000, 100.0, None, String::new(), false, None);

        assert_eq!(filled.status, OrderStatus::Filled);
        assert_eq!(filled.order.quantity, 2.0);
    }
}
