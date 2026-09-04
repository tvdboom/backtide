//! Python interface for the backtest module.

use crate::backtest::models::{ExperimentConfig, ExperimentResult};
use crate::config::models::LogLevel;
use crate::engine::Engine;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tracing::warn;

/// Global flag that signals a running experiment should abort as soon as
/// possible.  Set from Python via [`request_abort`] and polled from the
/// engine's hot loop. Automatically cleared when a new experiment starts.
pub static ABORT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Throttled bridge from parallel Rust simulation steps to a Python callback.
pub struct ProgressReporter {
    callback: Py<PyAny>,
    completed: AtomicU64,
    total: AtomicU64,
    next_report: AtomicU64,
}

impl ProgressReporter {
    /// Create a reporter for one Python `callback(completed, total)` callable.
    pub fn new(callback: Py<PyAny>) -> Self {
        Self {
            callback,
            completed: AtomicU64::new(0),
            total: AtomicU64::new(0),
            next_report: AtomicU64::new(0),
        }
    }

    /// Set the simulation work total and publish the initial zero position.
    pub fn set_total(&self, total: u64) {
        self.completed.store(0, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        self.next_report.store(0, Ordering::Relaxed);
        self.report(0, total);
    }

    /// Advance work while limiting Python calls to about 200 per experiment.
    pub fn advance(&self, amount: u64) {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return;
        }
        let completed = self.completed.fetch_add(amount, Ordering::Relaxed).saturating_add(amount);
        if completed >= total {
            self.report(total, total);
            return;
        }
        let stride = (total / 200).max(1);
        let next = self.next_report.load(Ordering::Relaxed);
        if completed < next {
            return;
        }
        if self
            .next_report
            .compare_exchange(
                next,
                completed.saturating_add(stride),
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            self.report(completed.min(total), total);
        }
    }

    /// Publish a final complete position after all strategy workers join.
    pub fn finish(&self) {
        let total = self.total.load(Ordering::Relaxed);
        self.completed.store(total, Ordering::Relaxed);
        self.report(total, total);
    }

    fn report(&self, completed: u64, total: u64) {
        Python::attach(|py| {
            if let Err(error) = self.callback.bind(py).call1((completed, total)) {
                warn!("Experiment progress callback failed: {error}");
            }
        });
    }
}

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
/// This is **not** the public API. Use [`Experiment`][backtide.backtest.Experiment],
/// which resolves polymorphic strategies and indicators before delegating here.
#[pyfunction(name = "_run_experiment")]
#[pyo3(
    signature = (
        config: "ExperimentConfig",
        verbose: "bool" = true,
        strategy_overrides: "dict[str, Any] | None" = None,
        indicator_overrides: "dict[str, Any] | None" = None,
        progress_callback: "Callable[[int, int], None] | None" = None,
    )
)]
pub fn run_experiment(
    py: Python<'_>,
    config: PyRef<'_, ExperimentConfig>,
    verbose: bool,
    strategy_overrides: Option<HashMap<String, Py<PyAny>>>,
    indicator_overrides: Option<HashMap<String, Py<PyAny>>>,
    progress_callback: Option<Py<PyAny>>,
) -> PyResult<ExperimentResult> {
    // Always start with a clean abort flag.
    ABORT_REQUESTED.store(false, Ordering::Relaxed);

    let cfg = (*config).clone();
    let engine = Engine::get()?;
    let strat = strategy_overrides.unwrap_or_default();
    let ind = indicator_overrides.unwrap_or_default();
    let metrics = cfg.metrics.implementations(py);
    let progress = progress_callback.map(ProgressReporter::new);

    // Release the GIL so rayon workers can acquire it.
    Ok(py.detach(|| {
        engine.run_experiment(&cfg, verbose, &strat, &ind, &metrics, progress.as_ref())
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::PyModule;

    fn callback(raises: bool) -> Py<PyAny> {
        Python::attach(|py| {
            let module = PyModule::from_code(
                py,
                pyo3::ffi::c_str!(
                    r#"
class Callback:
    def __init__(self, raises):
        self.raises = raises
        self.calls = []

    def __call__(self, completed, total):
        if self.raises:
            raise RuntimeError("deliberate callback error")
        self.calls.append((completed, total))
"#
                ),
                pyo3::ffi::c_str!("progress_test.py"),
                pyo3::ffi::c_str!("progress_test"),
            )
            .unwrap();
            module.getattr("Callback").unwrap().call1((raises,)).unwrap().unbind()
        })
    }

    #[test]
    fn progress_reporter_publishes_throttled_updates_and_completion() {
        let callback = callback(false);
        let reporter = Python::attach(|py| ProgressReporter::new(callback.clone_ref(py)));

        reporter.advance(1);
        reporter.set_total(1_000);
        reporter.advance(1);
        reporter.advance(1);
        reporter.advance(998);
        reporter.finish();

        let calls: Vec<(u64, u64)> =
            Python::attach(|py| callback.bind(py).getattr("calls").unwrap().extract().unwrap());
        assert_eq!(calls[0], (0, 1_000));
        assert!(calls.contains(&(1, 1_000)));
        assert_eq!(calls.last(), Some(&(1_000, 1_000)));
    }

    #[test]
    fn progress_reporter_ignores_callback_errors() {
        let reporter = ProgressReporter::new(callback(true));

        reporter.set_total(1);
        reporter.advance(1);
        reporter.finish();
    }

    #[test]
    fn experiment_logging_covers_every_level() {
        for level in
            [LogLevel::Trace, LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error]
        {
            experiment_log("coverage", level);
        }
    }
}
