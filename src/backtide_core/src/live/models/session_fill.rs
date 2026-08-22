//! Simulated-order matching results.

use crate::backtest::models::{Order, OrderStatus};
use pyo3::prelude::*;

/// Result of matching one simulated order.
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
///     Fee charged in the simulated account's base currency.
///
/// realized_pnl : float | None
///     Change in realized PnL from this fill, net of its commission.
///
/// reason : str
///     Human-readable matching or rejection reason.
///
/// See Also
/// --------
/// - backtide.live:SessionUpdate
///
/// Examples
/// --------
/// ```pycon
/// from backtide.backtest import Order
/// from backtide.live import MarketUpdate, Session
///
/// market = MarketUpdate(
///     "BTC-USD", "1m", 1_700_000_000, 1_700_000_060,
///     100.0, 102.0, 99.0, 101.0,
/// )
/// fill = Session().on_bar(market, [Order("BTC-USD", 1.0)]).fills[0]
/// print(fill.fill_price)
/// ```
#[pyclass(get_all, frozen, skip_from_py_object, module = "backtide.live")]
#[derive(Clone, Debug)]
pub struct SessionFill {
    /// Submitted order after any sizer resolution.
    pub order: Order,

    /// Fill, cancellation, or rejection Unix timestamp in seconds.
    pub timestamp: i64,

    /// Terminal order status.
    pub status: OrderStatus,

    /// Executed quote-currency price, or `None` when not filled.
    pub fill_price: Option<f64>,

    /// Fee charged in the simulated account's base currency.
    pub commission: f64,

    /// Change in realized PnL from this fill, net of its commission.
    pub realized_pnl: Option<f64>,

    /// Human-readable matching or rejection reason.
    pub reason: String,
}

#[pymethods]
impl SessionFill {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;
}
