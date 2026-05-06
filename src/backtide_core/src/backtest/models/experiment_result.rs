//! Experiment result data models.
//!
//! Holds everything produced by a single backtest run: per-strategy
//! equity curves, executed trades, order history, and summary metrics.

use crate::backtest::models::order::Order;
use crate::data::models::currency::Currency;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single equity-curve sample taken once per simulated bar.
///
/// Attributes
/// ----------
/// timestamp : int
///     UTC timestamp in seconds since the Unix epoch.
///
/// equity : float
///     Total portfolio value (cash + positions) in the base currency.
///
/// cash : dict[str | Currency, float]
///     Cash balance per currency at this bar.
///
/// drawdown : float
///     Running drawdown (negative or zero) versus the all-time high
///     equity, expressed as a fraction (e.g., -0.12 = -12 %).
///
/// See Also
/// --------
/// - backtide.backtest:ExperimentResult
/// - backtide.analysis:plot_pnl
/// - backtide.backtest:RunResult
#[pyclass(get_all, eq, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EquitySample {
    pub timestamp: i64,
    pub equity: f64,
    pub cash: HashMap<Currency, f64>,
    pub drawdown: f64,
}

#[pymethods]
impl EquitySample {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!(
            "EquitySample(ts={}, equity={:.2}, cash_ccy={}, dd={:.4})",
            self.timestamp,
            self.equity,
            self.cash.len(),
            self.drawdown,
        )
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let bytes = serde_json::to_vec(self)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &bytes).unbind())
    }

    fn __setstate__(&mut self, state: &Bound<'_, PyBytes>) -> PyResult<()> {
        *self = serde_json::from_slice(state.as_bytes())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(())
    }

    fn __reduce__(&self, py: Python<'_>) -> PyResult<(Py<PyAny>, (Py<PyAny>,), Py<PyAny>)> {
        let cls = PyModule::import(py, "backtide.backtest")?.getattr("EquitySample")?.unbind();
        let state = self.__getstate__(py)?;
        let copyreg = py.import("copyreg")?;
        let reconstruct = copyreg.getattr("__newobj__")?.unbind();
        Ok((reconstruct, (cls,), state.into_any()))
    }
}

/// A single round-trip trade (open + close of a position).
///
/// Attributes
/// ----------
/// symbol : str
///     The traded instrument's symbol.
///
/// quantity : float
///     Signed quantity. Positive = long round trip, negative = short.
///     Floating-point so fractional units are tracked exactly for crypto.
///
/// entry_ts : int
///     Open timestamp (seconds since the Unix epoch).
///
/// exit_ts : int
///     Close timestamp (seconds since the Unix epoch).
///
/// entry_price : float
///     Average fill price at entry, in the instrument's quote currency.
///
/// exit_price : float
///     Average fill price at exit.
///
/// pnl : float
///     Profit and loss in the base currency, after commission.
///
/// See Also
/// --------
/// - backtide.backtest:Order
/// - backtide.backtest:OrderRecord
/// - backtide.backtest:RunResult
#[pyclass(get_all, eq, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub quantity: f64,
    pub entry_ts: i64,
    pub exit_ts: i64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
}

#[pymethods]
impl Trade {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!("Trade(symbol={:?}, qty={}, pnl={:.2})", self.symbol, self.quantity, self.pnl,)
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let bytes = serde_json::to_vec(self)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &bytes).unbind())
    }

    fn __setstate__(&mut self, state: &Bound<'_, PyBytes>) -> PyResult<()> {
        *self = serde_json::from_slice(state.as_bytes())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(())
    }

    fn __reduce__(&self, py: Python<'_>) -> PyResult<(Py<PyAny>, (Py<PyAny>,), Py<PyAny>)> {
        let cls = PyModule::import(py, "backtide.backtest")?.getattr("Trade")?.unbind();
        let state = self.__getstate__(py)?;
        let copyreg = py.import("copyreg")?;
        let reconstruct = copyreg.getattr("__newobj__")?.unbind();
        Ok((reconstruct, (cls,), state.into_any()))
    }
}

/// A record of an order as resolved by the engine.
///
/// Attributes
/// ----------
/// order : [Order]
///     The original order.
///
/// timestamp : int
///     The bar timestamp at which the order was processed.
///
/// status : str
///     `"filled"`, `"cancelled"`, `"rejected"` or `"pending"`.
///
/// fill_price : float | None
///     Average fill price (None if not filled).
///
/// reason : str
///     Human-readable note (rejection / cancellation reason).
///
/// commission : float
///     Commission charged on the fill, in the order's quote currency.
///     Zero for non-filled orders.
///
/// pnl : float | None
///     Realised profit & loss attributable to this order, in the base
///     currency, after commission. Populated only on closing fills
///     (sell that flattens / reduces an existing long, or buy-to-cover);
///     `None` for opening fills, cancellations and rejections.
///
/// See Also
/// --------
/// - backtide.backtest:Order
/// - backtide.backtest:RunResult
/// - backtide.backtest:Trade
#[pyclass(get_all, eq, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrderRecord {
    pub order: Order,
    pub timestamp: i64,
    pub status: String,
    pub fill_price: Option<f64>,
    pub reason: String,
    #[serde(default)]
    pub commission: f64,
    #[serde(default)]
    pub pnl: Option<f64>,
}

#[pymethods]
impl OrderRecord {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!(
            "OrderRecord(id={:?}, status={}, ts={})",
            self.order.id, self.status, self.timestamp,
        )
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let bytes = serde_json::to_vec(self)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &bytes).unbind())
    }

    fn __setstate__(&mut self, state: &Bound<'_, PyBytes>) -> PyResult<()> {
        *self = serde_json::from_slice(state.as_bytes())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(())
    }

    fn __reduce__(&self, py: Python<'_>) -> PyResult<(Py<PyAny>, (Py<PyAny>,), Py<PyAny>)> {
        let cls = PyModule::import(py, "backtide.backtest")?.getattr("OrderRecord")?.unbind();
        let state = self.__getstate__(py)?;
        let copyreg = py.import("copyreg")?;
        let reconstruct = copyreg.getattr("__newobj__")?.unbind();
        Ok((reconstruct, (cls,), state.into_any()))
    }
}

/// Result of running a single strategy as part of an experiment.
///
/// Attributes
/// ----------
/// strategy_id : str
///     Unique identifier for this strategy run.
///
/// strategy_name : str
///     The user-facing name of the strategy.
///
/// equity_curve : list[[EquitySample]]
///     Per-bar equity samples in chronological order.
///
/// trades : list[[Trade]]
///     All round-trip trades closed during the run.
///
/// orders : list[[OrderRecord]]
///     All orders the engine processed (filled, canceled, rejected).
///
/// metrics : dict[str, float]
///     Summary metrics (total_return, sharpe, max_drawdown, ...).
///
/// base_currency : [Currency]
///     The portfolio's base (accounting) currency for this run. Equity,
///     PnL and drawdown values stored on the run are denominated in this
///     currency. Captured from the `ExperimentConfig` so analysis tools
///     don't need to look the experiment config up to label axes.
///
/// error : str | None
///     `None` on success. Otherwise, the first error raised by the
///     strategy during the run (e.g., an exception thrown by
///     `evaluate(...)`). Strategies that fail still produce a result
///     row so the rest of the experiment isn't lost — the engine simply
///     records the error and reports the experiment status as
///     `"failed"`.
///
/// is_benchmark : bool
///     Whether this run is the benchmark run for the experiment.
///
/// See Also
/// --------
/// - backtide.backtest:EquitySample
/// - backtide.backtest:ExperimentResult
/// - backtide.storage:query_strategy_runs
#[pyclass(get_all, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunResult {
    #[serde(default)]
    pub strategy_id: String,
    pub strategy_name: String,
    pub equity_curve: Vec<EquitySample>,
    pub trades: Vec<Trade>,
    pub orders: Vec<OrderRecord>,
    pub metrics: HashMap<String, f64>,
    #[serde(default)]
    pub base_currency: Currency,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub is_benchmark: bool,
}

#[pymethods]
impl RunResult {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!(
            "RunResult(id={:?}, strategy={:?}, n_bars={}, n_trades={}, is_benchmark={}, error={:?})",
            self.strategy_id,
            self.strategy_name,
            self.equity_curve.len(),
            self.trades.len(),
            self.is_benchmark,
            self.error,
        )
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let bytes = serde_json::to_vec(self)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &bytes).unbind())
    }

    fn __setstate__(&mut self, state: &Bound<'_, PyBytes>) -> PyResult<()> {
        *self = serde_json::from_slice(state.as_bytes())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(())
    }

    fn __reduce__(&self, py: Python<'_>) -> PyResult<(Py<PyAny>, (Py<PyAny>,), Py<PyAny>)> {
        let cls = PyModule::import(py, "backtide.backtest")?.getattr("RunResult")?.unbind();
        let state = self.__getstate__(py)?;
        let copyreg = py.import("copyreg")?;
        let reconstruct = copyreg.getattr("__newobj__")?.unbind();
        Ok((reconstruct, (cls,), state.into_any()))
    }
}

/// The complete result of a single experiment run.
///
/// Attributes
/// ----------
/// experiment_id : str
///     Unique identifier of the persisted experiment row.
///
/// name : str
///     Human-readable name (mirrors the config).
///
/// tags : list[str]
///     Tags assigned to the experiment.
///
/// started_at : int
///     UTC timestamp (seconds) when the run started.
///
/// finished_at : int
///     UTC timestamp (seconds) when the run finished.
///
/// status : str
///     `"completed"` if every strategy succeeded, `"failed"` otherwise.
///
/// strategies : list[[RunResult]]
///     One result entry per evaluated strategy.
///
/// warnings : list[str]
///     Non-fatal warnings emitted during the run.
///
/// See Also
/// --------
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:run_experiment
/// - backtide.backtest:RunResult
#[pyclass(get_all, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub experiment_id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub started_at: i64,
    pub finished_at: i64,
    pub status: String,
    pub strategies: Vec<RunResult>,
    pub warnings: Vec<String>,
}

#[pymethods]
impl ExperimentResult {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!(
            "ExperimentResult(id={:?}, name={:?}, status={}, n_strategies={})",
            self.experiment_id,
            self.name,
            self.status,
            self.strategies.len(),
        )
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let bytes = serde_json::to_vec(self)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &bytes).unbind())
    }

    fn __setstate__(&mut self, state: &Bound<'_, PyBytes>) -> PyResult<()> {
        *self = serde_json::from_slice(state.as_bytes())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(())
    }

    fn __reduce__(&self, py: Python<'_>) -> PyResult<(Py<PyAny>, (Py<PyAny>,), Py<PyAny>)> {
        let cls = PyModule::import(py, "backtide.backtest")?.getattr("ExperimentResult")?.unbind();
        let state = self.__getstate__(py)?;
        let copyreg = py.import("copyreg")?;
        let reconstruct = copyreg.getattr("__newobj__")?.unbind();
        Ok((reconstruct, (cls,), state.into_any()))
    }
}
