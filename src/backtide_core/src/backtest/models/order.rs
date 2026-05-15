//! Order data model.
//!
//! Represents a single order submitted to the simulated exchange
//! during a backtest.

use crate::backtest::models::order_type::OrderType;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyFloat;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generate a fresh short order id.
pub fn new_order_id() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_owned()
}

/// Wrapper around a Python sizer object stored on an [`Order`].
///
/// Implements the standard derives by delegating: `Clone` clones the
/// reference-counted `Py<PyAny>`, `Debug` prints a placeholder, and
/// `Serialize`/`Deserialize` skip the field (sizers are transient —
/// once the engine resolves them the slot is cleared).
pub struct SizerSlot(pub Py<PyAny>);

impl Clone for SizerSlot {
    fn clone(&self) -> Self {
        Python::attach(|py| SizerSlot(self.0.clone_ref(py)))
    }
}

impl std::fmt::Debug for SizerSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<sizer>")
    }
}

impl PartialEq for SizerSlot {
    fn eq(&self, _other: &Self) -> bool {
        false // sizers are never structurally equal
    }
}

impl Serialize for SizerSlot {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for SizerSlot {
    fn deserialize<D: serde::Deserializer<'de>>(_deserializer: D) -> Result<Self, D::Error> {
        Err(serde::de::Error::custom("SizerSlot cannot be deserialized"))
    }
}

// PyO3 conversions so `get_all` / `set_all` work on the `sizer` field.
impl<'py> IntoPyObject<'py> for SizerSlot {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(self.0.into_bound(py))
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for SizerSlot {
    type Error = PyErr;
    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        Ok(SizerSlot(ob.as_any().clone().unbind()))
    }
}

/// A trading order submitted during the simulation.
///
/// Read more in the [user guide][orders].
///
/// Attributes
/// ----------
/// id : str
///     Unique identifier of the order. Auto-generated if not provided.
///     For [`OrderType.Cancel`][OrderType] orders, the `id` field
///     identifies the target order that should be canceled. If an order
///     with the same ``id`` already exists in the order book, the
///     duplicate is rejected.
///
/// symbol : str
///     The ticker symbol this order targets.
///
/// quantity : int | float | [BaseSizer], default=1
///     Signed quantity (positive = buy, negative = sell). Fractional values
///     are accepted only for crypto instruments. When a _sizer_ is passed, the
///     engine resolves the quantity automatically at order-processing time using
///     portfolio equity converted to the asset's quote currency and the asset's
///     price.
///
/// order_type : [OrderType]
///     The execution semantics (market, limit, stop-loss, etc...). Also accepts
///     a string of the form PascalCase (`StopLoss`) or snake_case (`stop_loss"),
///     case-insensitively.
///
/// price : float | None
///     Primary price for the order. The exact meaning depends on
///     `order_type`:
///
/// - `Market` / `Cancel` / `SettlePosition`: ignored.
/// - `Limit` / `TakeProfit`: the limit / target price.
/// - `StopLoss`: the stop (trigger) price.
/// - `StopLossLimit` / `TakeProfitLimit`: the stop (trigger)
///   price; once hit the order converts to a limit at
///   `limit_price`.
/// - `TrailingStop` / `TrailingStopLimit`: the trail amount in
///   price units (positive). The engine maintains the running
///   extreme internally.
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
    /// Signed quantity (positive = buy, negative = sell). Fractional values
    /// are allowed only for crypto instruments. When a sizer is attached this
    /// is ``0.0`` until the engine resolves it.
    pub quantity: f64,
    /// The execution semantics.
    pub order_type: OrderType,
    /// Primary price (limit / stop / trail amount).
    pub price: Option<f64>,
    /// Secondary limit price for `*Limit` stop-style orders.
    #[serde(default)]
    pub limit_price: Option<f64>,
    /// Optional position sizer. The engine resolves it into a concrete
    /// quantity at order-processing time using current equity converted to
    /// the instrument quote currency and price.
    #[serde(skip)]
    pub sizer: Option<SizerSlot>,
}

#[pymethods]
impl Order {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        symbol: "str" = "",
        quantity: "float | Sizer | None" = None,
        order_type: "str | OrderType" = OrderType::Market,
        price: "float | None" = None,
        limit_price: "float | None" = None,
        id: "str | None" = None,
    ))]
    fn new(
        _py: Python<'_>,
        symbol: &str,
        quantity: Option<Bound<'_, PyAny>>,
        order_type: OrderType,
        price: Option<f64>,
        limit_price: Option<f64>,
        id: Option<String>,
    ) -> PyResult<Self> {
        let (qty, sizer) = match quantity {
            None => (1.0, None),
            Some(q) => {
                if let Ok(f) = q.extract::<f64>() {
                    (f, None)
                } else if q.hasattr("calculate")? {
                    (0.0, Some(SizerSlot(q.unbind())))
                } else {
                    return Err(PyErr::new::<PyTypeError, _>(
                        "quantity must be an int, float, or a Sizer with a calculate() method",
                    ));
                }
            },
        };

        Ok(Self {
            id: id.unwrap_or_else(new_order_id),
            symbol: symbol.to_owned(),
            quantity: qty,
            order_type,
            price,
            limit_price,
            sizer,
        })
    }

    fn __repr__(&self) -> String {
        let sizer_str = if self.sizer.is_some() {
            ", sizer=<attached>"
        } else {
            ""
        };
        match (self.price, self.limit_price) {
            (Some(p), Some(l)) => format!(
                "Order(id={:?}, symbol={:?}, qty={}, type={}, price={}, limit={}{})",
                self.id, self.symbol, self.quantity, self.order_type, p, l, sizer_str,
            ),
            (Some(p), None) => format!(
                "Order(id={:?}, symbol={:?}, qty={}, type={}, price={}{})",
                self.id, self.symbol, self.quantity, self.order_type, p, sizer_str,
            ),
            _ => format!(
                "Order(id={:?}, symbol={:?}, qty={}, type={}{})",
                self.id, self.symbol, self.quantity, self.order_type, sizer_str,
            ),
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(
        Bound<'py, PyAny>,
        (String, Py<PyAny>, OrderType, Option<f64>, Option<f64>, Option<String>),
    )> {
        let cls = PyModule::import(py, "backtide.backtest")?.getattr("Order")?;
        // For pickling, serialize the resolved quantity as a float.
        // Sizers are lost on (de)serialization — by that point the quantity
        // has already been resolved by the engine.
        let qty_obj: Py<PyAny> = PyFloat::new(py, self.quantity).into_any().unbind();
        Ok((
            cls,
            (
                self.symbol.clone(),
                qty_obj,
                self.order_type,
                self.price,
                self.limit_price,
                Some(self.id.clone()),
            ),
        ))
    }
}
