//! Python interface for the backtest module.

use crate::backtest::models::experiment_config::ExperimentConfig;
use crate::backtest::models::experiment_result::ExperimentResult;
use crate::config::models::log_level::LogLevel;
use crate::engine::Engine;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag that signals a running experiment should abort as soon as
/// possible.  Set from Python via [`request_abort`] and polled from the
/// engine's hot loop. Automatically cleared when a new experiment starts.
pub static ABORT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Returns `true` if the abort flag is currently set.
#[inline]
pub fn check_abort() -> bool {
    ABORT_REQUESTED.load(Ordering::Relaxed)
}

/// Signal the Rust engine to abort the current experiment.
#[pyfunction]
pub fn request_abort() {
    ABORT_REQUESTED.store(true, Ordering::Relaxed);
}

/// Write a message to the active experiment's log file.
///
/// This is intended to be called from a custom strategy's `evaluate()`
/// method. The message is routed through the `tracing` layer so it
/// ends up in the per-experiment `logs.txt` alongside engine events.
#[pyfunction]
#[pyo3(signature = (message: "str", level: "str | LogLevel" = LogLevel::Info))]
pub fn experiment_log(message: &str, level: LogLevel) {
    match level {
        LogLevel::Trace => tracing::trace!("{message}"),
        LogLevel::Debug => tracing::debug!("{message}"),
        LogLevel::Info => tracing::info!("{message}"),
        LogLevel::Warn => tracing::warn!("{message}"),
        LogLevel::Error => tracing::error!("{message}"),
    }
}

/// Low-level entry point that runs an already-built experiment
/// configuration.
///
/// This is **not** the public API — Python callers should use
/// `backtide.backtest.run_experiment`, which handles kwargs
/// translation and polymorphic strategies/indicators before
/// delegating here.
#[pyfunction]
#[pyo3(
    signature = (
        config: "ExperimentConfig",
        verbose: "bool" = true,
        strategy_overrides: "dict[str, Any] | None" = None,
        indicator_overrides: "dict[str, Any] | None" = None,
    )
)]
pub fn run_experiment(
    py: Python<'_>,
    config: PyRef<'_, ExperimentConfig>,
    verbose: bool,
    strategy_overrides: Option<HashMap<String, Py<PyAny>>>,
    indicator_overrides: Option<HashMap<String, Py<PyAny>>>,
) -> PyResult<ExperimentResult> {
    // Always start with a clean abort flag.
    ABORT_REQUESTED.store(false, Ordering::Relaxed);

    let cfg = (*config).clone();
    let engine = Engine::get()?;
    let strat = strategy_overrides.unwrap_or_default();
    let ind = indicator_overrides.unwrap_or_default();

    // Release the GIL so rayon workers can acquire it.
    Ok(py.detach(|| engine.run_experiment(&cfg, verbose, &strat, &ind))?)
}
