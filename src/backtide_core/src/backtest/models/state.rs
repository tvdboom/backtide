//! Simulation state data model.
//!
//! Represents the complete state of a running backtest at a single point
//! in time, including the portfolio snapshot and the current timestamp.

use crate::backtest::models::portfolio::Portfolio;
use crate::config::interface::Config;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

/// The simulation state passed to a strategy's `evaluate` method on every tick.
///
/// Contains the current portfolio (cash balances and open positions) and
/// the UTC timestamp of the bar being processed as seconds since the Unix
/// epoch.
///
/// Attributes
/// ----------
/// portfolio : [Portfolio]
///     Current portfolio holdings (cash and positions).
///
/// timestamp : int
///     UTC timestamp of the current bar in seconds since the Unix epoch.
///
/// datetime : datetime.datetime
///     The `timestamp` as a timezone-aware datetime. Uses the timezone from
///     `config.display.timezone`. Falls back to the system's local timezone
///     if none is configured.
///
/// See Also
/// --------
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:Portfolio
/// - backtide.backtest:Order
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Current portfolio holdings.
    pub portfolio: Portfolio,

    /// UTC timestamp of the current bar (seconds since Unix epoch).
    pub timestamp: i64,
}

#[pymethods]
impl State {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        portfolio: "Portfolio" = Portfolio::default(),
        timestamp: "int" = 0,
    ))]
    fn new(portfolio: Portfolio, timestamp: i64) -> Self {
        Self {
            portfolio,
            timestamp,
        }
    }

    /// Convert the timestamp to a datetime in the configured timezone.
    #[getter]
    fn datetime<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let dt_mod = py.import("datetime")?;
        let dt_cls = dt_mod.getattr("datetime")?;

        let tz = match Config::get()?.display.timezone.as_ref() {
            Some(name) => py
                .import("zoneinfo")
                .and_then(|m| m.getattr("ZoneInfo"))
                .and_then(|cls| cls.call1((name.as_str(),)))?,
            None => {
                // No configured timezone → use the local system timezone.
                let naive = dt_cls.call_method1("fromtimestamp", (self.timestamp,))?;
                return naive.call_method0("astimezone");
            },
        };

        dt_cls.call_method1("fromtimestamp", (self.timestamp, tz))
    }

    fn __repr__(&self) -> String {
        format!("State(portfolio={:?}, timestamp={})", self.portfolio, self.timestamp,)
    }
}
