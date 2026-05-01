//! Order data model.
//!
//! Represents a single order submitted to the simulated exchange
//! during a backtest.

use crate::backtest::models::order_type::OrderType;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generate a fresh short order id.
pub fn new_order_id() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_owned()
}

/// A trading order submitted during the simulation.
///
/// Attributes
/// ----------
/// id : str
///     Unique identifier of the order. Auto-generated if not provided.
///     For [`OrderType.CancelOrder`][OrderType] orders, the `id` field
///     identifies the *target* order that should be cancelled.
///
/// symbol : str
///     The ticker symbol this order targets.
///
/// order_type : [OrderType]
///     The execution semantics (market, limit, stop-loss, etc.).
///
/// quantity : int
///     Signed quantity. Positive for buy orders, negative for sell orders.
///
/// price : float | None
///     Primary price for the order. The exact meaning depends on
///     `order_type`:
///
///     - ``Market`` / ``CancelOrder`` / ``SettlePosition``: ignored.
///     - ``Limit`` / ``TakeProfit``: the limit / target price.
///     - ``StopLoss``: the stop (trigger) price.
///     - ``StopLossLimit`` / ``TakeProfitLimit``: the stop (trigger)
///       price; once hit the order converts to a limit at
///       ``limit_price``.
///     - ``TrailingStop`` / ``TrailingStopLimit``: the trail amount in
///       price units (positive). The engine maintains the running
///       extreme internally.
///
/// limit_price : float | None
///     Secondary limit price used by the ``StopLossLimit``,
///     ``TakeProfitLimit`` and ``TrailingStopLimit`` order types.
///     Once the stop component triggers, the order converts to a
///     limit order resting at this price. Ignored for all other
///     order types.
///
/// See Also
/// --------
/// - backtide.backtest:OrderType
/// - backtide.backtest:Portfolio
/// - backtide.backtest:State
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Order {
    /// Unique identifier of the order.
    pub id: String,
    /// The ticker symbol this order targets.
    pub symbol: String,
    /// The execution semantics.
    pub order_type: OrderType,
    /// Signed quantity (positive = buy, negative = sell).
    pub quantity: i64,
    /// Primary price (limit / stop / trail amount).
    pub price: Option<f64>,
    /// Secondary limit price for `*Limit` stop-style orders.
    #[serde(default)]
    pub limit_price: Option<f64>,
}

#[pymethods]
impl Order {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    /// Create a new order.
    ///
    /// Parameters
    /// ----------
    /// symbol : str
    ///     The ticker symbol this order targets. For ``CancelOrder``
    ///     orders this can be left empty; the `id` field identifies
    ///     the order to cancel.
    ///
    /// order_type : str | OrderType, default="market"
    ///     The execution semantics.
    ///
    /// quantity : int, default=0
    ///     Signed quantity (positive = buy, negative = sell).
    ///
    /// price : float | None, default=None
    ///     Primary price (limit, stop or trail amount depending on
    ///     `order_type`). ``None`` for market orders.
    ///
    /// limit_price : float | None, default=None
    ///     Secondary limit price for `*Limit` stop-style orders. Once
    ///     the stop triggers, the order rests as a limit at this price.
    ///
    /// id : str | None, default=None
    ///     Optional explicit order id. When ``None`` (default) a fresh
    ///     short uuid is generated. When the `order_type` is
    ///     ``CancelOrder`` this should be set to the id of the order
    ///     that you want to cancel.
    #[new]
    #[pyo3(signature = (
        symbol: "str" = "",
        order_type: "str | OrderType" = OrderType::default(),
        quantity: "int" = 0,
        price: "float | None" = None,
        limit_price: "float | None" = None,
        id: "str | None" = None,
    ))]
    fn new(
        symbol: &str,
        order_type: OrderType,
        quantity: i64,
        price: Option<f64>,
        limit_price: Option<f64>,
        id: Option<String>,
    ) -> Self {
        Self {
            id: id.unwrap_or_else(new_order_id),
            symbol: symbol.to_owned(),
            order_type,
            quantity,
            price,
            limit_price,
        }
    }

    fn __repr__(&self) -> String {
        match (self.price, self.limit_price) {
            (Some(p), Some(l)) => format!(
                "Order(id={:?}, symbol={:?}, type={}, qty={}, price={}, limit={})",
                self.id, self.symbol, self.order_type, self.quantity, p, l,
            ),
            (Some(p), None) => format!(
                "Order(id={:?}, symbol={:?}, type={}, qty={}, price={})",
                self.id, self.symbol, self.order_type, self.quantity, p,
            ),
            _ => format!(
                "Order(id={:?}, symbol={:?}, type={}, qty={})",
                self.id, self.symbol, self.order_type, self.quantity,
            ),
        }
    }
}
