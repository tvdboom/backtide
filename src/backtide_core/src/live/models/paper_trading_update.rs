//! Paper-trading state transitions.

use super::{MarketUpdate, PaperFill, PaperTradingSnapshot};
use pyo3::prelude::*;
use std::collections::HashMap;

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
///
/// indicators : dict[str, dict[str, list[list[float]]]]
///     Latest values for configured and strategy-required indicators, grouped
///     by deterministic indicator name and symbol.
///
/// See Also
/// --------
/// - backtide.live:MarketUpdate
/// - backtide.live:PaperTradingSession
///
/// Examples
/// --------
/// ```pycon
/// from backtide.live import MarketUpdate, PaperTradingSession
///
/// market = MarketUpdate(
///     "BTC-USD", "1m", 1_700_000_000, 1_700_000_060,
///     100.0, 102.0, 99.0, 101.0,
/// )
/// update = PaperTradingSession().on_bar(market)
/// print(update.processed)
/// print(update.snapshot.equity)
/// ```
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
    /// Latest configured indicator outputs by indicator and symbol.
    pub indicators: HashMap<String, HashMap<String, Vec<Vec<f64>>>>,
}

