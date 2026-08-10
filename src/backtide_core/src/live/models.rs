//! Data models used by live market feeds and paper-trading sessions.

use crate::backtest::models::{Order, OrderStatus, Portfolio};
use crate::data::models::{Bar, Currency};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a paper-trading session.
///
/// Attributes
/// ----------
/// initial_cash : float, default=100000
///     Starting cash balance in `base_currency`.
///
/// base_currency : [Currency], default=Currency.USD
///     Accounting currency for cash, fills, and equity.
///
/// commission_pct : float, default=0
///     Percentage commission charged on every fill (for example, `0.1`
///     means 0.1%).
///
/// commission_fixed : float, default=0
///     Fixed commission charged on every fill.
///
/// slippage : float, default=0
///     Percentage slippage applied to fills.
///
/// allow_short : bool, default=False
///     Whether fills may create a negative position.
///
/// allow_margin : bool, default=False
///     Whether fills may create a negative cash balance.
///
/// trade_on_partial : bool, default=False
///     Whether strategy and order processing runs on incomplete candles.
///     Keeping the default avoids repeated decisions on the same candle.
///
/// max_history : int, default=10000
///     Maximum bars retained per symbol for strategy evaluation.
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.live")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaperTradingConfig {
    /// Starting balance in the base currency.
    pub initial_cash: f64,
    /// Currency used for cash and mark-to-market accounting.
    pub base_currency: Currency,
    /// Variable fee in percentage points (for example, `0.1` means 0.1%).
    pub commission_pct: f64,
    /// Fixed fee in the base currency charged per fill.
    pub commission_fixed: f64,
    /// Adverse fill adjustment in percentage points.
    pub slippage: f64,
    /// Whether an order may create a negative position.
    pub allow_short: bool,
    /// Whether an order may create a negative cash balance.
    pub allow_margin: bool,
    /// Whether incomplete candle updates may trigger orders.
    pub trade_on_partial: bool,
    /// Maximum number of candles retained per symbol.
    pub max_history: usize,
}

impl Default for PaperTradingConfig {
    fn default() -> Self {
        Self {
            initial_cash: 100_000.0,
            base_currency: Currency::USD,
            commission_pct: 0.0,
            commission_fixed: 0.0,
            slippage: 0.0,
            allow_short: false,
            allow_margin: false,
            trade_on_partial: false,
            max_history: 10_000,
        }
    }
}

#[pymethods]
impl PaperTradingConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        initial_cash: "float" = 100_000.0,
        base_currency: "str | Currency" = Currency::USD,
        commission_pct: "float" = 0.0,
        commission_fixed: "float" = 0.0,
        slippage: "float" = 0.0,
        allow_short: "bool" = false,
        allow_margin: "bool" = false,
        trade_on_partial: "bool" = false,
        max_history: "int" = 10_000,
    ))]
    fn new(
        initial_cash: f64,
        base_currency: Currency,
        commission_pct: f64,
        commission_fixed: f64,
        slippage: f64,
        allow_short: bool,
        allow_margin: bool,
        trade_on_partial: bool,
        max_history: usize,
    ) -> Self {
        Self {
            initial_cash,
            base_currency,
            commission_pct,
            commission_fixed,
            slippage,
            allow_short,
            allow_margin,
            trade_on_partial,
            max_history,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PaperTradingConfig(initial_cash={}, base_currency={}, allow_short={}, allow_margin={})",
            self.initial_cash, self.base_currency, self.allow_short, self.allow_margin,
        )
    }
}

/// A candle received from a live market-data connection.
///
/// `is_final` is `true` only when the provider has closed the candle, or when
/// Backtide observed the next candle and can therefore finalize the prior one.
///
/// Attributes
/// ----------
/// provider : str
///     Lowercase provider identifier, or `"mock"` for replay data.
///
/// symbol : str
///     Canonical provider-independent symbol.
///
/// interval : str
///     Canonical interval string.
///
/// open_ts : int
///     Candle-open Unix timestamp in seconds.
///
/// close_ts : int
///     Candle-close Unix timestamp in seconds.
///
/// open : float
///     Opening price in quote-currency units.
///
/// high : float
///     Highest price in quote-currency units.
///
/// low : float
///     Lowest price in quote-currency units.
///
/// close : float
///     Latest or final closing price in quote-currency units.
///
/// volume : float
///     Traded volume in base-asset units.
///
/// n_trades : int | None
///     Provider-reported trade count when available.
///
/// is_final : bool
///     Whether no further updates are expected for this candle.
///
/// received_ts : int
///     Local receipt Unix timestamp in seconds.
#[pyclass(get_all, frozen, from_py_object, module = "backtide.live")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketUpdate {
    /// Lowercase provider identifier, or `"mock"` for replay data.
    pub provider: String,
    /// Canonical provider-independent symbol (for example, `"BTC-USD"`).
    pub symbol: String,
    /// Canonical interval string (for example, `"1m"`).
    pub interval: String,
    /// Candle-open Unix timestamp in seconds.
    pub open_ts: u64,
    /// Candle-close Unix timestamp in seconds.
    pub close_ts: u64,
    /// Opening price in quote-currency units.
    pub open: f64,
    /// Highest price in quote-currency units.
    pub high: f64,
    /// Lowest price in quote-currency units.
    pub low: f64,
    /// Latest or final closing price in quote-currency units.
    pub close: f64,
    /// Traded volume in base-asset units.
    pub volume: f64,
    /// Provider-reported trade count when available.
    pub n_trades: Option<i32>,
    /// Whether no further updates are expected for this candle.
    pub is_final: bool,
    /// Local receipt Unix timestamp in seconds.
    pub received_ts: i64,
}

impl MarketUpdate {
    /// Whether the update contains a usable positive OHLC candle.
    pub fn is_valid_bar(&self) -> bool {
        self.close_ts > self.open_ts
            && [self.open, self.high, self.low, self.close]
                .iter()
                .all(|price| price.is_finite() && *price > 0.0)
            && self.high >= self.open.max(self.close)
            && self.low <= self.open.min(self.close)
            && self.high >= self.low
            && self.volume.is_finite()
            && self.volume >= 0.0
    }

    /// Convert the transport model to the engine's canonical bar model.
    pub fn bar(&self) -> Bar {
        Bar {
            open_ts: self.open_ts,
            close_ts: self.close_ts,
            open_ts_exchange: self.open_ts,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            adj_close: self.close,
            volume: self.volume,
            n_trades: self.n_trades,
        }
    }
}

#[pymethods]
impl MarketUpdate {
    #[new]
    #[pyo3(signature = (
        symbol,
        interval,
        open_ts,
        close_ts,
        open,
        high,
        low,
        close,
        volume=0.0,
        n_trades=None,
        is_final=true,
        provider: "str"="mock",
        received_ts=0,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        symbol: String,
        interval: String,
        open_ts: u64,
        close_ts: u64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        n_trades: Option<i32>,
        is_final: bool,
        provider: &str,
        received_ts: i64,
    ) -> Self {
        Self {
            provider: provider.to_owned(),
            symbol,
            interval,
            open_ts,
            close_ts,
            open,
            high,
            low,
            close,
            volume,
            n_trades,
            is_final,
            received_ts,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "MarketUpdate(provider={:?}, symbol={:?}, interval={:?}, close={}, is_final={})",
            self.provider, self.symbol, self.interval, self.close, self.is_final,
        )
    }
}

/// Result of matching one paper order.
///
/// Attributes
/// ----------
/// order : [Order]
///     Submitted order after any sizer resolution.
///
/// timestamp : int
///     Fill, cancellation, or rejection Unix timestamp in seconds.
///
/// status : [OrderStatus]
///     Terminal order status.
///
/// fill_price : float | None
///     Executed quote-currency price, or `None` when not filled.
///
/// commission : float
///     Fee charged in the paper account's base currency.
///
/// realized_pnl : float | None
///     Change in realized PnL from this fill, net of its commission.
///
/// reason : str
///     Human-readable matching or rejection reason.
#[pyclass(get_all, frozen, skip_from_py_object, module = "backtide.live")]
#[derive(Clone, Debug)]
pub struct PaperFill {
    /// Submitted order after any sizer resolution.
    pub order: Order,
    /// Fill, cancellation, or rejection Unix timestamp in seconds.
    pub timestamp: i64,
    /// Terminal order status.
    pub status: OrderStatus,
    /// Executed quote-currency price, or `None` when not filled.
    pub fill_price: Option<f64>,
    /// Fee charged in the paper account's base currency.
    pub commission: f64,
    /// Change in realized PnL from this fill, net of its commission.
    pub realized_pnl: Option<f64>,
    /// Human-readable matching or rejection reason.
    pub reason: String,
}

/// Mark-to-market snapshot of a paper-trading account.
///
/// Attributes
/// ----------
/// portfolio : [Portfolio]
///     Cash, positions, and currently resting orders.
///
/// latest_prices : dict[str, float]
///     Latest valid close per canonical symbol.
///
/// equity : float
///     Cash plus positions marked to `latest_prices`.
///
/// realized_pnl : float
///     Cumulative realized PnL net of commissions.
///
/// unrealized_pnl : float
///     Open-position PnL marked to `latest_prices`.
///
/// processed_bars : int
///     Number of updates that triggered matching or strategy evaluation.
#[pyclass(get_all, frozen, skip_from_py_object, module = "backtide.live")]
#[derive(Clone, Debug)]
pub struct PaperTradingSnapshot {
    /// Cash, positions, and currently resting orders.
    pub portfolio: Portfolio,
    /// Latest valid close per canonical symbol.
    pub latest_prices: HashMap<String, f64>,
    /// Cash plus positions marked to `latest_prices`.
    pub equity: f64,
    /// Cumulative realized PnL net of commissions.
    pub realized_pnl: f64,
    /// Open-position PnL marked to `latest_prices`.
    pub unrealized_pnl: f64,
    /// Number of market updates that triggered matching or strategy evaluation.
    pub processed_bars: u64,
}

/// State transition produced after processing a market update.
///
/// Attributes
/// ----------
/// market : [MarketUpdate]
///     Market update supplied by the caller.
///
/// fills : list[[PaperFill]]
///     Orders filled, canceled, or rejected during this transition.
///
/// snapshot : [PaperTradingSnapshot]
///     Account state after this transition.
///
/// orders_submitted : int
///     Number of explicit and strategy orders submitted on this update.
///
/// processed : bool
///     Whether this update was new, valid, and eligible for trading.
#[pyclass(get_all, frozen, skip_from_py_object, module = "backtide.live")]
#[derive(Clone, Debug)]
pub struct PaperTradingUpdate {
    /// Market update supplied by the caller.
    pub market: MarketUpdate,
    /// Orders filled, canceled, or rejected during this transition.
    pub fills: Vec<PaperFill>,
    /// Account state after this transition.
    pub snapshot: PaperTradingSnapshot,
    /// Number of explicit and strategy orders submitted on this update.
    pub orders_submitted: usize,
    /// Whether this update was new, valid, and eligible for trading.
    pub processed: bool,
}
