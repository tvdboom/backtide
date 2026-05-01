use crate::backtest::models::order::Order;
use crate::data::models::currency::Currency;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
/// positions : dict[str, int]
///     Open positions keyed by ticker symbol. Positive values are long
///     positions, negative values are short positions.
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
    pub cash: HashMap<Currency, f64>,
    /// Open positions keyed by ticker symbol.
    pub positions: HashMap<String, i64>,
    /// Currently open (unfilled) orders.
    pub orders: Vec<Order>,
}

impl Default for Portfolio {
    fn default() -> Self {
        let mut cash = HashMap::new();
        cash.insert(Currency::default(), 0.0);
        Self {
            cash,
            positions: HashMap::new(),
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
        cash: "dict[str | Currency, float]" = HashMap::from([(Currency::default(), 0.0)]),
        positions: "dict[str, int]" = HashMap::new(),
        orders: "list[Order]" = vec![],
    ))]
    fn new(
        cash: HashMap<Currency, f64>,
        positions: HashMap<String, i64>,
        orders: Vec<Order>,
    ) -> Self {
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
