use pyo3::{Py, PyAny, PyResult, Python};
use crate::backtest::models::order::Order;
use crate::backtest::models::portfolio::Portfolio;
use crate::backtest::models::state::State;
use crate::strategies::utils::IndicatorView;

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
        _closes: &[(String, &[f64])],
        _indicators: &IndicatorView<'_>,
        _portfolio: &Portfolio,
        _state: &State,
    ) -> Vec<Order>;

    /// Indicators that must be computed up-front for this strategy.
    fn required_indicators_inner(&self, _py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(Vec::new())
    }
}
