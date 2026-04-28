//! Experiment result data models.
//!
//! Holds everything produced by a single backtest run: per-strategy
//! equity curves, executed trades, order history, and summary metrics.

use crate::backtest::models::order::Order;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single equity-curve sample taken once per simulated bar.
///
/// Attributes
/// ----------
/// timestamp : int
///     UTC timestamp in seconds since the Unix epoch.
///
/// equity : float
///     Total portfolio value (cash + positions) in the base currency.
///
/// cash : float
///     Cash balance in the base currency at this bar.
///
/// drawdown : float
///     Running drawdown (negative or zero) versus the all-time high
///     equity, expressed as a fraction (e.g. -0.12 = -12 %).
#[pyclass(get_all, eq, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EquitySample {
    pub timestamp: i64,
    pub equity: f64,
    pub cash: f64,
    pub drawdown: f64,
}

#[pymethods]
impl EquitySample {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!(
            "EquitySample(ts={}, equity={:.2}, cash={:.2}, dd={:.4})",
            self.timestamp, self.equity, self.cash, self.drawdown,
        )
    }
}

/// A single round-trip trade (open + close of a position).
///
/// Attributes
/// ----------
/// symbol : str
///     The traded instrument's symbol.
///
/// quantity : int
///     Signed quantity. Positive = long round trip, negative = short.
///
/// entry_ts : int
///     Open timestamp (seconds since the Unix epoch).
///
/// exit_ts : int
///     Close timestamp (seconds since the Unix epoch).
///
/// entry_price : float
///     Average fill price at entry, in the instrument's quote currency.
///
/// exit_price : float
///     Average fill price at exit.
///
/// pnl : float
///     Profit and loss in the base currency, after commission.
#[pyclass(get_all, eq, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub quantity: i64,
    pub entry_ts: i64,
    pub exit_ts: i64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
}

#[pymethods]
impl Trade {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!("Trade(symbol={:?}, qty={}, pnl={:.2})", self.symbol, self.quantity, self.pnl,)
    }
}

/// A record of an order as resolved by the engine.
///
/// Attributes
/// ----------
/// order : [Order]
///     The original order.
///
/// timestamp : int
///     The bar timestamp at which the order was processed.
///
/// status : str
///     ``"filled"``, ``"cancelled"``, ``"rejected"`` or ``"pending"``.
///
/// fill_price : float | None
///     Average fill price (None if not filled).
///
/// reason : str
///     Human-readable note (rejection / cancellation reason).
#[pyclass(get_all, eq, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrderRecord {
    pub order: Order,
    pub timestamp: i64,
    pub status: String,
    pub fill_price: Option<f64>,
    pub reason: String,
}

#[pymethods]
impl OrderRecord {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!(
            "OrderRecord(id={:?}, status={}, ts={})",
            self.order.id, self.status, self.timestamp,
        )
    }
}

/// Result of running a single strategy as part of an experiment.
///
/// Attributes
/// ----------
/// strategy_id : str
///     Unique identifier for this strategy run.
///
/// strategy_name : str
///     The user-facing name of the strategy.
///
/// equity_curve : list[[EquitySample]]
///     Per-bar equity samples in chronological order.
///
/// trades : list[[Trade]]
///     All round-trip trades closed during the run.
///
/// orders : list[[OrderRecord]]
///     All orders the engine processed (filled, cancelled, rejected).
///
/// metrics : dict[str, float]
///     Summary metrics (total_return, sharpe, max_drawdown, ...).
#[pyclass(get_all, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyRunResult {
    #[serde(default)]
    pub strategy_id: String,
    pub strategy_name: String,
    pub equity_curve: Vec<EquitySample>,
    pub trades: Vec<Trade>,
    pub orders: Vec<OrderRecord>,
    pub metrics: HashMap<String, f64>,
}

#[pymethods]
impl StrategyRunResult {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!(
            "StrategyRunResult(id={:?}, strategy={:?}, n_bars={}, n_trades={})",
            self.strategy_id,
            self.strategy_name,
            self.equity_curve.len(),
            self.trades.len(),
        )
    }
}

/// The complete result of a single experiment run.
///
/// Attributes
/// ----------
/// experiment_id : str
///     Unique identifier of the persisted experiment row.
///
/// name : str
///     Human-readable name (mirrors the config).
///
/// tags : list[str]
///     Tags assigned to the experiment.
///
/// started_at : int
///     UTC timestamp (seconds) when the run started.
///
/// finished_at : int
///     UTC timestamp (seconds) when the run finished.
///
/// status : str
///     ``"completed"`` if every strategy succeeded, ``"failed"`` otherwise.
///
/// strategies : list[[StrategyRunResult]]
///     One result entry per evaluated strategy.
///
/// warnings : list[str]
///     Non-fatal warnings emitted during the run.
#[pyclass(get_all, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub experiment_id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub started_at: i64,
    pub finished_at: i64,
    pub status: String,
    pub strategies: Vec<StrategyRunResult>,
    pub warnings: Vec<String>,
}

#[pymethods]
impl ExperimentResult {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!(
            "ExperimentResult(id={:?}, name={:?}, status={}, n_strategies={})",
            self.experiment_id,
            self.name,
            self.status,
            self.strategies.len(),
        )
    }
}
