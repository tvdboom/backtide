use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, IntoEnumIterator};

use std::str::FromStr;

/// The type of order that can be submitted to the exchange.
///
/// Defines which execution semantics apply to a trade request.
/// The engine validates that only allowed order types (configured
/// in the exchange settings) are submitted during the simulation.
///
/// Read more in the [user guide][orders].
///
/// Attributes
/// ----------
/// name : str
///     The human-readable display name of the variant.
///
/// See Also
/// --------
/// - backtide.backtest:CommissionType
/// - backtide.backtest:ExchangeExpConfig
/// - backtide.backtest:Order
#[pyclass(skip_from_py_object, frozen, eq, hash, module = "backtide.backtest")]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    Display,
    EnumIter,
    SerializeDisplay,
    DeserializeFromStr,
)]
pub enum OrderType {
    #[default]
    Market,
    Limit,
    StopLoss,
    TakeProfit,
    StopLossLimit,
    TakeProfitLimit,
    TrailingStop,
    TrailingStopLimit,
    SettlePosition,
    Cancel,
}

impl OrderType {
    /// Parse an order type from a flexible string representation.
    ///
    /// Accepted formats (all case-insensitive):
    ///
    /// - PascalCase variant name: `"StopLoss"`, `"Cancel"`
    /// - snake_case: `"stop_loss"`, `"cancel_order"`
    /// - Without trailing "order": `"cancel"`, `"Cancel"`
    /// - Any mix: `"trailing_stop_limit"`, `"TrailingStopLimit"`, `"STOPLOSS"`
    pub fn parse_flexible(s: &str) -> Result<Self, String> {
        // Normalize: lowercase and strip underscores
        let normalized: String = s.trim().to_ascii_lowercase().replace('_', "");

        match normalized.as_str() {
            "market" => Ok(Self::Market),
            "limit" => Ok(Self::Limit),
            "stoploss" | "stop" => Ok(Self::StopLoss),
            "takeprofit" => Ok(Self::TakeProfit),
            "stoplosslimit" => Ok(Self::StopLossLimit),
            "takeprofitlimit" => Ok(Self::TakeProfitLimit),
            "trailingstop" => Ok(Self::TrailingStop),
            "trailingstoplimit" => Ok(Self::TrailingStopLimit),
            "settleposition" | "settle" => Ok(Self::SettlePosition),
            "cancel" => Ok(Self::Cancel),
            _ => Err(format!("Unknown order type: {s:?}")),
        }
    }
}

#[pymethods]
impl OrderType {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        Self::parse_flexible(s).map_err(PyValueError::new_err)
    }

    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }
    pub fn __str__(&self) -> String {
        self.to_string()
    }

    /// The human-readable display name of the variant.
    #[getter]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Market => "Market",
            Self::Limit => "Limit",
            Self::StopLoss => "Stop-Loss",
            Self::TakeProfit => "Take-Profit",
            Self::StopLossLimit => "Stop-Loss-Limit",
            Self::TakeProfitLimit => "Take-Profit-Limit",
            Self::TrailingStop => "Trailing-Stop",
            Self::TrailingStopLimit => "Trailing-Stop-Limit",
            Self::SettlePosition => "Settle-Position",
            Self::Cancel => "Cancel-Order",
        }
    }

    /// Return a description of the order type.
    ///
    /// Returns
    /// -------
    /// str
    ///     A brief explanation of the order's execution semantics.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Market => "Fills immediately at the current market price.",
            Self::Limit => "Fills only at the specified price or better.",
            Self::StopLoss => "Becomes a market order once the stop price is hit, used to limit losses.",
            Self::TakeProfit => "Becomes a market order once the target price is hit, used to lock in gains.",
            Self::StopLossLimit => "Becomes a limit order once the stop price is hit, combining stop-loss protection with price control.",
            Self::TakeProfitLimit => "Becomes a limit order once the target price is hit, combining profit-taking with price control.",
            Self::TrailingStop => "A stop order whose trigger price trails the market price by a fixed offset, locking in gains as the price moves favourably.",
            Self::TrailingStopLimit => "A trailing stop that converts to a limit order instead of a market order when triggered.",
            Self::SettlePosition => "Closes an existing open position entirely at the current market price.",
            Self::Cancel => "Cancels a previously submitted order, identified by its id.",
        }
    }

    /// Return the default variant.
    ///
    /// Returns
    /// -------
    /// self
    ///     The default variant.
    #[staticmethod]
    fn get_default(py: Python<'_>) -> Py<Self> {
        Py::new(py, Self::default()).unwrap()
    }

    /// Return all variants.
    ///
    /// Returns
    /// -------
    /// list[self]
    ///     All variants of this type.
    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }
}

impl FromStr for OrderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_flexible(s)
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for OrderType {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<OrderType>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string (flexible: snake_case, PascalCase)
        let s: String = obj.extract()?;
        Self::parse_flexible(&s).map_err(PyValueError::new_err)
    }
}
