//! Simulation state data model.
//!
//! Represents the state of a running backtest at a single point in time,
//! including the current timestamp, bar index, total bars, and warmup flag.

use crate::config::interface::Config;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

/// The simulation state passed to a strategy's `evaluate` method on every tick.
///
/// Contains metadata about the current position in the simulation: the UTC
/// timestamp of the bar being processed, the zero-based bar index, the total
/// number of bars in the dataset, and whether the engine is still in the
/// warmup phase (where indicators are computed but no orders are placed).
///
/// Attributes
/// ----------
/// timestamp : int
///     UTC timestamp of the current bar in seconds since the Unix epoch.
///
/// bar_index : int
///     Zero-based index of the current bar in the dataset.
///
/// total_bars : int
///     Total number of bars in the dataset.
///
/// is_warmup : bool
///     Whether the engine is currently in the warmup phase. During warmup
///     indicators are computed but orders are not executed.
///
/// datetime : datetime.datetime
///     The `timestamp` as a timezone-aware datetime. Uses the timezone from
///     `config.display.timezone`. Falls back to the system's local timezone
///     if none is configured.
///
/// See Also
/// --------
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:Order
/// - backtide.backtest:Portfolio
#[pyclass(get_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// UTC timestamp of the current bar (seconds since Unix epoch).
    pub timestamp: i64,
    /// Zero-based index of the current bar in the dataset.
    pub bar_index: u64,
    /// Total number of bars in the dataset.
    pub total_bars: u64,
    /// Whether the engine is currently in the warmup phase.
    pub is_warmup: bool,
}

#[pymethods]
impl State {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        timestamp: "int" = 0,
        bar_index: "int" = 0,
        total_bars: "int" = 0,
        is_warmup: "bool" = false,
    ))]
    fn new(timestamp: i64, bar_index: u64, total_bars: u64, is_warmup: bool) -> Self {
        Self {
            timestamp,
            bar_index,
            total_bars,
            is_warmup,
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
        format!(
            "State(timestamp={}, bar_index={}, total_bars={}, is_warmup={})",
            self.timestamp, self.bar_index, self.total_bars, self.is_warmup,
        )
    }
}
