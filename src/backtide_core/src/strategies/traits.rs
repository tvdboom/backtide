use crate::backtest::models::order::Order;
use crate::backtest::models::portfolio::Portfolio;
use crate::backtest::models::state::State;
use crate::data::models::bar::Bar;
use crate::strategies::utils::IndicatorView;
use pyo3::{Py, PyAny, PyResult, Python};

/// Trait for all built-in strategies.
pub trait Strategy {
    /// Human-readable name.
    const NAME: &'static str;

    /// One-sentence explanation of what the strategy does.
    const DESCRIPTION: &'static str;

    /// Whether this is a portfolio-rotation (multi-asset) strategy.
    const IS_MULTI_ASSET: bool;

    /// Decide which orders to place on the current bar.
    fn evaluate_inner(
        &self,
        bars: &[(String, Vec<Bar>)],
        portfolio: &Portfolio,
        state: &State,
        indicators: &IndicatorView<'_>,
    ) -> Vec<Order>;

    /// Indicators that must be computed up-front for this strategy.
    fn required_indicators_inner(&self, _py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(Vec::new())
    }
}
