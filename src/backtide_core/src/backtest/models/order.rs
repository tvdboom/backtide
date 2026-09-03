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
        parse_stored_order_id(s).map_err(|error| FromSqlError::Other(Box::new(error)))
    }
}

fn parse_stored_order_id(value: &str) -> Result<OrderId, uuid::Error> {
    if value.len() == 12 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let mut normalized = String::with_capacity(32);
        normalized.push_str(value);
        normalized.push_str("00000000000000000000");
        return Uuid::parse_str(&normalized).map(OrderId);
    }
    Uuid::parse_str(value).map(OrderId)
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
    /// Equal-weight available cash after conversion to the instrument quote currency.
    CashEqualWeight(EqualWeight),
    EqualWeight(EqualWeight),
    FixedFractional(FixedFractional),
    FixedNotional(FixedNotional),
    FixedQuantity(FixedQuantity),
    KellyCriterion(KellyCriterion),
    RiskBased(RiskBased),
    VolatilityScaled(VolatilityScaled),
}

impl BuiltinSizer {
    /// Whether the sizer should receive available cash instead of total equity.
    pub(crate) fn uses_cash_capital(&self) -> bool {
        matches!(self, Self::CashEqualWeight(_))
    }

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
            CashEqualWeight,
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

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::{PyFloat, PyModule, PyString};

    #[test]
    fn order_id_supports_sentinel_serde_and_python_conversion() {
        let nil = OrderId::nil();
        assert!(nil.is_nil());
        assert_eq!(nil.to_string(), "00000000000000000000000000000000");
        assert_eq!(format!("{nil:?}"), nil.to_string());

        let generated = OrderId::default();
        assert!(!generated.is_nil());
        let json = serde_json::to_string(&generated).unwrap();
        assert_eq!(serde_json::from_str::<OrderId>(&json).unwrap(), generated);
        assert!(serde_json::from_str::<OrderId>("\"invalid\"").is_err());

        Python::attach(|py| {
            let value = generated.into_pyobject(py).unwrap().into_any();
            assert_eq!(value.extract::<OrderId>().unwrap(), generated);
            assert!(PyString::new(py, "invalid").extract::<OrderId>().is_err());
            assert!(PyFloat::new(py, 1.0).extract::<OrderId>().is_err());
        });
    }

    #[test]
    fn sizer_slots_clone_format_serialize_and_convert_to_python() {
        let builtin = BuiltinSizer::CashEqualWeight(EqualWeight::new(2));
        assert!(builtin.uses_cash_capital());
        assert_eq!(builtin.calculate(1_000.0, 100.0, None, None).unwrap(), 5.0);
        let ordinary = BuiltinSizer::FixedQuantity(FixedQuantity::new(3.0));
        assert!(!ordinary.uses_cash_capital());
        assert_eq!(ordinary.calculate(0.0, 0.0, None, None).unwrap(), 3.0);

        let builtin_slot = SizerSlot::Builtin(ordinary);
        assert!(format!("{builtin_slot:?}").contains("builtin sizer"));
        assert_ne!(builtin_slot, builtin_slot.clone());
        assert_eq!(serde_json::to_string(&builtin_slot).unwrap(), "null");
        assert!(serde_json::from_str::<SizerSlot>("null").is_err());

        Python::attach(|py| {
            assert!(builtin_slot.clone().into_pyobject(py).unwrap().is_none());

            let module = PyModule::from_code(
                py,
                pyo3::ffi::c_str!(
                    "class Sizer:\n    def calculate(self, *args):\n        return 4.0\nsizer = Sizer()\n"
                ),
                pyo3::ffi::c_str!("order_sizer.py"),
                pyo3::ffi::c_str!("order_sizer"),
            )
            .unwrap();
            let object = module.getattr("sizer").unwrap();
            let custom = SizerSlot::Custom(object.clone().unbind());
            assert!(format!("{custom:?}").contains("custom sizer"));
            assert!(custom.clone().into_pyobject(py).unwrap().hasattr("calculate").unwrap());
            assert!(matches!(object.extract::<SizerSlot>().unwrap(), SizerSlot::Custom(_)));

            let py_builtin =
                Py::new(py, FixedQuantity::new(7.0)).unwrap().into_bound(py).into_any();
            assert!(matches!(
                py_builtin.extract::<SizerSlot>().unwrap(),
                SizerSlot::Builtin(BuiltinSizer::FixedQuantity(_))
            ));
        });
    }

    #[test]
    fn order_constructor_handles_numeric_builtin_custom_and_invalid_quantities() {
        Python::attach(|py| {
            let default =
                Order::new(py, "AAPL", None, OrderType::Market, None, None, None).unwrap();
            assert_eq!(default.quantity, 1.0);
            assert!(default.sizer.is_none());
            assert!(default.__repr__().contains("type=Market"));

            let numeric = Order::new(
                py,
                "AAPL",
                Some(PyFloat::new(py, 2.5).into_any()),
                OrderType::Limit,
                Some(100.0),
                Some(99.0),
                Some(default.id.to_string()),
            )
            .unwrap();
            assert_eq!(numeric.id, default.id);
            assert!(numeric.__repr__().contains("limit=99"));

            let py_builtin =
                Py::new(py, FixedQuantity::new(2.0)).unwrap().into_bound(py).into_any();
            let sized = Order::new(
                py,
                "AAPL",
                Some(py_builtin),
                OrderType::Market,
                Some(100.0),
                None,
                None,
            )
            .unwrap();
            assert_eq!(sized.quantity, 0.0);
            assert!(matches!(sized.sizer, Some(SizerSlot::Builtin(_))));
            assert!(sized.__repr__().contains("sizer=<attached>"));

            let module = PyModule::from_code(
                py,
                pyo3::ffi::c_str!(
                    "class Sizer:\n    def calculate(self, *args):\n        return 4.0\nsizer = Sizer()\n"
                ),
                pyo3::ffi::c_str!("custom_order_sizer.py"),
                pyo3::ffi::c_str!("custom_order_sizer"),
            )
            .unwrap();
            let custom = Order::new(
                py,
                "AAPL",
                Some(module.getattr("sizer").unwrap()),
                OrderType::Market,
                None,
                None,
                None,
            )
            .unwrap();
            assert!(matches!(custom.sizer, Some(SizerSlot::Custom(_))));

            assert!(Order::new(
                py,
                "AAPL",
                Some(PyString::new(py, "bad").into_any()),
                OrderType::Market,
                None,
                None,
                None,
            )
            .is_err());
            assert!(Order::new(
                py,
                "AAPL",
                None,
                OrderType::Market,
                None,
                None,
                Some("invalid".to_owned()),
            )
            .is_err());
        });
    }

    #[test]
    fn parses_current_stored_order_id() {
        let value = "d3c2bf141cd9498caaf8072da0103790";

        let parsed = parse_stored_order_id(value).expect("valid UUID");

        assert_eq!(parsed.to_string(), value);
    }

    #[test]
    fn expands_legacy_stored_order_id() {
        let parsed = parse_stored_order_id("d3c2bf141cd9").expect("valid legacy ID");

        assert_eq!(parsed.to_string(), "d3c2bf141cd900000000000000000000");
    }

    #[test]
    fn rejects_invalid_stored_order_id() {
        assert!(parse_stored_order_id("not-an-order").is_err());
    }
}
