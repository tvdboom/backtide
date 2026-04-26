//! Order data model.
//!
//! Represents a single order submitted to the simulated exchange
//! during a backtest.

use crate::backtest::models::order_type::OrderType;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

/// A trading order submitted during the simulation.
///
/// Attributes
/// ----------
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
    ///     The ticker symbol this order targets.
    ///
    /// order_type : str | OrderType, default="market"
    ///     The execution semantics.
    ///
    /// quantity : int, default=0
    ///     Signed quantity (positive = buy, negative = sell).
    ///
    /// price : float | None, default=None
    ///     Limit / stop price. ``None`` for market orders.
    #[new]
    #[pyo3(signature = (
        symbol: "str",
        order_type: "str | OrderType" = OrderType::default(),
        quantity: "int" = 0,
        price: "float | None" = None,
    ))]
    fn new(symbol: &str, order_type: OrderType, quantity: i64, price: Option<f64>) -> Self {
        Self {
            symbol: symbol.to_owned(),
            order_type,
            quantity,
            price,
        }
    }

    fn __repr__(&self) -> String {
        match self.price {
            Some(p) => format!(
                "Order(symbol={:?}, type={}, qty={}, price={})",
                self.symbol, self.order_type, self.quantity, p,
            ),
            None => format!(
                "Order(symbol={:?}, type={}, qty={})",
                self.symbol, self.order_type, self.quantity,
            ),
        }
    }
}
