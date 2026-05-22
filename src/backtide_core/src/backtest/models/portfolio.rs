use crate::backtest::models::order::Order;
use crate::constants::{Cash, Positions};
use crate::data::models::Currency;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

/// A snapshot of the portfolio's holdings at a point in time.
///
/// Cash is represented as a mapping from currency to amount, allowing
/// multi-currency portfolios. Positions are a mapping from ticker
/// symbol to signed quantity (positive = long, negative = short).
///
/// Attributes
/// ----------
/// cash : dict[[Currency], float]
///     Cash balances keyed by currency. Each value is the amount held
///     in that currency.
///
/// positions : dict[str, float]
///     Open positions keyed by ticker symbol. Positive values are long
///     positions, negative values are short positions. Fractional values
///     are supported only for crypto instruments (e.g., 0.0234 BTC).
///
/// orders : list[[Order]]
///     Currently open (unfilled) orders.
///
/// See Also
/// --------
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:Order
/// - backtide.backtest:State
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Portfolio {
    /// Cash balances keyed by currency.
    pub cash: Cash,

    /// Open positions keyed by ticker symbol.
    pub positions: Positions,

    /// Currently open (unfilled) orders.
    pub orders: Vec<Order>,
}

impl Default for Portfolio {
    fn default() -> Self {
        let mut cash = Cash::new();
        cash.insert(Currency::default(), 0.0);
        Self {
            cash,
            positions: Positions::new(),
            orders: Vec::new(),
        }
    }
}

#[pymethods]
impl Portfolio {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        cash: "dict[str | Currency, float]" = Cash::from([(Currency::default(), 0.)]),
        positions: "dict[str, float]" = Positions::new(),
        orders: "list[Order]" = vec![],
    ))]
    fn new(cash: Cash, positions: Positions, orders: Vec<Order>) -> Self {
        Self {
            cash,
            positions,
            orders,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Portfolio(cash={:?}, positions={:?}, orders={:?})",
            self.cash, self.positions, self.orders,
        )
    }
}
