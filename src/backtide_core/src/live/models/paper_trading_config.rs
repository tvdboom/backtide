//! Paper-trading session configuration.

use crate::backtest::models::OrderType;
use crate::data::models::Currency;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

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
///
/// max_leverage : float, default=2
///     Maximum gross exposure divided by current equity when margin is enabled.
///
/// initial_margin : float, default=50
///     Minimum equity percentage required when increasing exposure.
///
/// maintenance_margin : float, default=25
///     Minimum equity percentage maintained against gross exposure. Breaches
///     trigger deterministic paper liquidation.
///
/// margin_interest : float, default=0
///     Annual percentage charged on negative base-currency cash.
///
/// borrow_rate : float, default=0
///     Annual percentage charged on short notional.
///
/// max_position_size : float, default=100
///     Maximum absolute per-symbol notional as a percentage of equity.
///
/// max_drawdown : float, default=0
///     Drawdown percentage that halts exposure-increasing orders. Zero disables
///     the guard.
///
/// allowed_order_types : list[str | OrderType], default=all order types
///     Order types accepted by the paper broker.
///
/// partial_fills : bool, default=False
///     Whether fills are capped by `max_volume_participation`.
///
/// max_volume_participation : float, default=100
///     Maximum percentage of a candle's volume available to one simulated fill.
///
/// metrics : list[str]
///     Built-in performance metric keys maintained during the session.
///
/// risk_free_rate : float, default=0
///     Annual risk-free rate used by risk-adjusted performance metrics.
///
/// See Also
/// --------
/// - backtide.live:PaperTradingSession
///
/// Examples
/// --------
/// ```pycon
/// from backtide.live import PaperTradingConfig
///
/// config = PaperTradingConfig(
///     initial_cash=25_000,
///     commission_pct=0.1,
///     slippage=0.05,
/// )
/// print(config.initial_cash)
/// ```
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
    /// Maximum gross-exposure-to-equity ratio when margin is enabled.
    pub max_leverage: f64,
    /// Equity percentage required to open or increase exposure.
    pub initial_margin: f64,
    /// Equity percentage required to maintain open exposure.
    pub maintenance_margin: f64,
    /// Annual percentage charged on negative cash.
    pub margin_interest: f64,
    /// Annual percentage charged on short notional.
    pub borrow_rate: f64,
    /// Per-symbol absolute notional cap as a percentage of equity.
    pub max_position_size: f64,
    /// Drawdown percentage that halts exposure-increasing orders; zero disables it.
    pub max_drawdown: f64,
    /// Order types accepted by the paper broker.
    pub allowed_order_types: Vec<OrderType>,
    /// Whether candle volume constrains fill quantity.
    pub partial_fills: bool,
    /// Maximum percentage of candle volume available to one fill.
    pub max_volume_participation: f64,
    /// Built-in metric keys maintained during the session.
    pub metrics: Vec<String>,
    /// Annual risk-free rate used by risk-adjusted metrics.
    pub risk_free_rate: f64,
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
            max_leverage: 2.0,
            initial_margin: 50.0,
            maintenance_margin: 25.0,
            margin_interest: 0.0,
            borrow_rate: 0.0,
            max_position_size: 100.0,
            max_drawdown: 0.0,
            allowed_order_types: vec![
                OrderType::Market,
                OrderType::Limit,
                OrderType::StopLoss,
                OrderType::TakeProfit,
                OrderType::StopLossLimit,
                OrderType::TakeProfitLimit,
                OrderType::TrailingStop,
                OrderType::TrailingStopLimit,
                OrderType::SettlePosition,
                OrderType::Cancel,
            ],
            partial_fills: false,
            max_volume_participation: 100.0,
            metrics: vec![
                "total_return".to_owned(),
                "pnl".to_owned(),
                "final_equity".to_owned(),
                "n_trades".to_owned(),
                "win_rate".to_owned(),
                "ann_volatility".to_owned(),
                "sharpe".to_owned(),
                "sortino".to_owned(),
                "max_dd".to_owned(),
            ],
            risk_free_rate: 0.0,
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
        max_leverage: "float" = 2.0,
        initial_margin: "float" = 50.0,
        maintenance_margin: "float" = 25.0,
        margin_interest: "float" = 0.0,
        borrow_rate: "float" = 0.0,
        max_position_size: "float" = 100.0,
        max_drawdown: "float" = 0.0,
        allowed_order_types: "list[str | OrderType]" = PaperTradingConfig::default().allowed_order_types,
        partial_fills: "bool" = false,
        max_volume_participation: "float" = 100.0,
        metrics: "list[str]" = PaperTradingConfig::default().metrics,
        risk_free_rate: "float" = 0.0,
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
        max_leverage: f64,
        initial_margin: f64,
        maintenance_margin: f64,
        margin_interest: f64,
        borrow_rate: f64,
        max_position_size: f64,
        max_drawdown: f64,
        allowed_order_types: Vec<OrderType>,
        partial_fills: bool,
        max_volume_participation: f64,
        metrics: Vec<String>,
        risk_free_rate: f64,
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
            max_leverage,
            initial_margin,
            maintenance_margin,
            margin_interest,
            borrow_rate,
            max_position_size,
            max_drawdown,
            allowed_order_types,
            partial_fills,
            max_volume_participation,
            metrics,
            risk_free_rate,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PaperTradingConfig(initial_cash={}, base_currency={}, allow_short={}, allow_margin={})",
            self.initial_cash, self.base_currency, self.allow_short, self.allow_margin,
        )
    }
}

