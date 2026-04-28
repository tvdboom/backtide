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
///     Limit / stop price. ``None`` for market orders.
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
    /// Limit / stop price, or None for market orders.
    pub price: Option<f64>,
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
    ///     Limit / stop price. ``None`` for market orders.
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
        id: "str | None" = None,
    ))]
    fn new(
        symbol: &str,
        order_type: OrderType,
        quantity: i64,
        price: Option<f64>,
        id: Option<String>,
    ) -> Self {
        Self {
            id: id.unwrap_or_else(new_order_id),
            symbol: symbol.to_owned(),
            order_type,
            quantity,
            price,
        }
    }

    fn __repr__(&self) -> String {
        match self.price {
            Some(p) => format!(
                "Order(id={:?}, symbol={:?}, type={}, qty={}, price={})",
                self.id, self.symbol, self.order_type, self.quantity, p,
            ),
            None => format!(
                "Order(id={:?}, symbol={:?}, type={}, qty={})",
                self.id, self.symbol, self.order_type, self.quantity,
            ),
        }
    }
}
