use crate::backtest::models::order_type::OrderType;
use crate::sizers::*;
use duckdb::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyFloat, PyString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ────────────────────────────────────────────────────────────────────────────
// OrderId
// ────────────────────────────────────────────────────────────────────────────

/// A lightweight, `Copy` order identifier backed by a UUID v4.
///
/// When formatted as a string it produces a 32-character lowercase hex
/// representation (the "simple" UUID format), making it GUID-like while
/// staying on the stack with no heap allocation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderId(Uuid);

impl OrderId {
    /// Generate a fresh random order id.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// The nil (all-zeros) id, used as a sentinel for "not yet assigned".
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Returns `true` when this is the nil sentinel.
    pub fn is_nil(self) -> bool {
        self.0.is_nil()
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.simple())
    }
}

impl std::fmt::Debug for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.simple())
    }
}

impl Serialize for OrderId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for OrderId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Uuid::parse_str(&s).map(OrderId).map_err(serde::de::Error::custom)
    }
}

// DuckDB: OrderId is stored/loaded as TEXT — just string ↔ parse.
impl ToSql for OrderId {
    fn to_sql(&self) -> duckdb::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for OrderId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let s = value.as_str()?;
        Uuid::parse_str(s).map(OrderId).map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}

// PyO3 conversions so `get_all` / `set_all` work on the `id` field.

impl<'py> IntoPyObject<'py> for OrderId {
    type Target = PyString;
    type Output = Bound<'py, PyString>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(PyString::new(py, &self.to_string()))
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for OrderId {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let s: String = ob.extract()?;
        Uuid::parse_str(&s)
            .map(OrderId)
            .map_err(|e| PyErr::new::<PyTypeError, _>(format!("invalid order id: {e}")))
    }
}

/// A built-in sizer variant that can be resolved entirely in Rust
/// without crossing the Python boundary.
#[derive(Clone, Debug)]
pub enum BuiltinSizer {
    EqualWeight(EqualWeight),
    FixedFractional(FixedFractional),
    FixedNotional(FixedNotional),
    FixedQuantity(FixedQuantity),
    KellyCriterion(KellyCriterion),
    RiskBased(RiskBased),
    VolatilityScaled(VolatilityScaled),
}

impl BuiltinSizer {
    /// Run the sizing calculation entirely in Rust.
    pub fn calculate(
        &self,
        equity: f64,
        price: f64,
        stop_distance: Option<f64>,
        atr: Option<f64>,
    ) -> Result<f64, String> {
        macro_rules! delegate {
            ($($variant:ident),* $(,)?) => {
                match self {
                    $(Self::$variant(s) => Sizer::calculate(s, equity, price, stop_distance, atr),)*
                }
            };
        }

        delegate!(
            EqualWeight,
            FixedFractional,
            FixedNotional,
            FixedQuantity,
            KellyCriterion,
            RiskBased,
            VolatilityScaled,
        )
        .map_err(|e| Python::attach(|py| e.value(py).to_string()))
    }

    /// Try to extract a built-in sizer from a Python object.
    ///
    /// Returns `Some(BuiltinSizer)` if the object is one of the known
    /// Rust-backed sizer types, `None` otherwise (i.e. custom Python sizer).
    pub fn try_from_py(py: Python<'_>, obj: &Bound<'_, PyAny>) -> Option<Self> {
        let _ = py;

        macro_rules! try_dispatch {
            ($($variant:ident),* $(,)?) => {
                $(
                    if let Ok(cell) = obj.cast::<$variant>() {
                        return Some(Self::$variant(cell.borrow().clone()));
                    }
                )*
            };
        }

        try_dispatch!(
            EqualWeight,
            FixedFractional,
            FixedNotional,
            FixedQuantity,
            KellyCriterion,
            RiskBased,
            VolatilityScaled,
        );

        None
    }
}

/// Sizer slot stored on an [`Order`], either a built-in Rust sizer or a
/// custom Python object.
///
/// Built-in sizers are resolved entirely in Rust without acquiring the GIL.
/// Custom sizers fall back to calling `calculate()` through PyO3.
///
/// `Serialize`/`Deserialize` skip the field (sizers are transient — once
/// the engine resolves them, the slot is cleared).
pub enum SizerSlot {
    /// One of the seven built-in sizer types, resolved in pure Rust.
    Builtin(BuiltinSizer),
    /// A user-supplied Python sizer with a `calculate()` method.
    Custom(Py<PyAny>),
}

impl Clone for SizerSlot {
    fn clone(&self) -> Self {
        match self {
            Self::Builtin(b) => Self::Builtin(b.clone()),
            Self::Custom(obj) => Python::attach(|py| Self::Custom(obj.clone_ref(py))),
        }
    }
}

impl std::fmt::Debug for SizerSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin(b) => write!(f, "<builtin sizer: {b:?}>"),
            Self::Custom(_) => write!(f, "<custom sizer>"),
        }
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
        match self {
            Self::Custom(obj) => Ok(obj.into_bound(py)),
            Self::Builtin(_) => {
                // Built-in sizers round-trip as None on the Python side;
                // by the time Python sees the order, the quantity is already
                // resolved.
                Ok(py.None().into_bound(py))
            },
        }
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for SizerSlot {
    type Error = PyErr;
    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // Try to recognise a built-in sizer first.
        if let Some(builtin) = BuiltinSizer::try_from_py(ob.py(), ob.as_any()) {
            return Ok(SizerSlot::Builtin(builtin));
        }
        Ok(SizerSlot::Custom(ob.as_any().clone().unbind()))
    }
}

/// A trading order submitted during the simulation.
///
/// Read more in the [user guide][orders].
///
/// Attributes
/// ----------
/// id : str
///     Unique identifier of the order. Auto-generated if not provided. For
///     [`OrderType.Cancel`][OrderType] orders, the `id` field identifies the
///     target order that should be canceled. If an order with the same `id`
///     already exists in the order book, the duplicate is rejected.
///
/// symbol : str
///     The ticker symbol this order targets.
///
/// quantity : int | float | [BaseSizer], default=1
///     Signed quantity (positive = buy, negative = sell). Fractional values
///     are accepted only for crypto instruments. When a [sizer][sizers] is
///     passed, the engine resolves the quantity automatically at order-processing
///     time using portfolio equity converted to the asset's quote currency and
///     the asset's price.
///
/// order_type : [OrderType]
///     The execution semantics (market, limit, stop-loss, etc...). Also accepts
///     a string of the form PascalCase (`StopLoss`) or snake_case (`stop_loss`),
///     case-insensitively.
///
/// price : float | None
///     Primary price for the order. The exact meaning depends on
///     `order_type`:
///
/// - `Market` / `Cancel` / `SettlePosition`: ignored.
/// - `Limit` / `TakeProfit`: the limit / target price.
/// - `StopLoss`: the stop (trigger) price.
/// - `StopLossLimit` / `TakeProfitLimit`: the stop (trigger) price. Once hit, the
///   order converts to a limit at `limit_price`.
/// - `TrailingStop` / `TrailingStopLimit`: the trail amount in price units (positive).
///   The engine maintains the running extreme internally.
///
/// limit_price : float | None
///     Secondary limit price used by the `StopLossLimit`, `TakeProfitLimit` and
///     `TrailingStopLimit` order types. Once the stop component triggers, the order
///     converts to a limit order resting at this price. Ignored for all other order
///     types.
///
/// See Also
/// --------
/// - backtide.backtest:OrderType
/// - backtide.backtest:Portfolio
/// - backtide.backtest:State
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub symbol: String,
    pub quantity: f64,
    pub order_type: OrderType,
    pub price: Option<f64>,
    #[serde(default)]
    pub limit_price: Option<f64>,

    /// Optional position sizer. The engine resolves it into a concrete
    /// quantity at order-processing time using current equity converted
    /// to the instrument quote currency and price.
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
                } else if let Some(builtin) = BuiltinSizer::try_from_py(_py, &q) {
                    (0.0, Some(SizerSlot::Builtin(builtin)))
                } else if q.hasattr("calculate")? {
                    (0.0, Some(SizerSlot::Custom(q.unbind())))
                } else {
                    return Err(PyErr::new::<PyTypeError, _>(
                        "quantity must be an int, float, or a Sizer with a calculate() method",
                    ));
                }
            },
        };

        let order_id = match id {
            Some(s) if !s.is_empty() => Uuid::parse_str(&s)
                .map(OrderId)
                .map_err(|e| PyErr::new::<PyTypeError, _>(format!("invalid order id: {e}")))?,
            _ => OrderId::new(),
        };

        Ok(Self {
            id: order_id,
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
                self.id.to_string(),
                self.symbol,
                self.quantity,
                self.order_type,
                p,
                l,
                sizer_str,
            ),
            (Some(p), None) => format!(
                "Order(id={:?}, symbol={:?}, qty={}, type={}, price={}{})",
                self.id.to_string(),
                self.symbol,
                self.quantity,
                self.order_type,
                p,
                sizer_str,
            ),
            _ => format!(
                "Order(id={:?}, symbol={:?}, qty={}, type={}{})",
                self.id.to_string(),
                self.symbol,
                self.quantity,
                self.order_type,
                sizer_str,
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
                Some(self.id.to_string()),
            ),
        ))
    }
}
