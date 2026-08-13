//! Mark-to-market paper-account snapshots.

use crate::backtest::models::Portfolio;
use pyo3::prelude::*;
use std::collections::HashMap;

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
///
/// gross_exposure : float
///     Sum of absolute marked position values.
///
/// net_exposure : float
///     Signed marked value of all positions.
///
/// leverage : float
///     Gross exposure divided by equity.
///
/// buying_power : float
///     Remaining gross exposure capacity under the configured leverage cap.
///
/// drawdown : float
///     Fractional decline from peak session equity.
///
/// peak_equity : float
///     Highest marked equity observed during the session.
///
/// total_costs : float
///     Cumulative commissions, margin interest, and short-borrow charges.
///
/// trading_halted : bool
///     Whether a configured risk guard is rejecting exposure-increasing orders.
///
/// halt_reason : str | None
///     Human-readable reason for the active risk halt.
///
/// metrics : dict[str, float]
///     Selected live-compatible performance metrics computed from session state.
///
/// See Also
/// --------
/// - backtide.live:PaperTradingSession
///
/// Examples
/// --------
/// ```pycon
/// from backtide.live import PaperTradingSession
///
/// snapshot = PaperTradingSession().snapshot()
/// print(snapshot.equity)
/// print(snapshot.portfolio.positions)
/// ```
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
    /// Sum of absolute marked position values.
    pub gross_exposure: f64,
    /// Signed marked value of all positions.
    pub net_exposure: f64,
    /// Gross exposure divided by equity.
    pub leverage: f64,
    /// Remaining gross exposure capacity under the configured leverage cap.
    pub buying_power: f64,
    /// Fractional decline from peak equity.
    pub drawdown: f64,
    /// Highest equity observed during the session.
    pub peak_equity: f64,
    /// Cumulative commissions and financing costs.
    pub total_costs: f64,
    /// Whether exposure-increasing orders are currently halted by a risk control.
    pub trading_halted: bool,
    /// Human-readable reason for the active trading halt.
    pub halt_reason: Option<String>,
    /// Live-compatible performance metrics.
    pub metrics: HashMap<String, f64>,
}

