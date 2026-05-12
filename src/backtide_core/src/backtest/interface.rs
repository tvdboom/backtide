//! Python interface for the backtest module.

use crate::backtest::models::experiment_config::ExperimentConfig;
use crate::backtest::models::experiment_result::ExperimentResult;
use crate::engine::Engine;
use pyo3::prelude::*;
use std::collections::HashMap;

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
        config,
        verbose = true,
        strategy_overrides = None,
        indicator_overrides = None,
    )
)]
pub fn run_experiment(
    py: Python<'_>,
    config: PyRef<'_, ExperimentConfig>,
    verbose: bool,
    strategy_overrides: Option<HashMap<String, Py<PyAny>>>,
    indicator_overrides: Option<HashMap<String, Py<PyAny>>>,
) -> PyResult<ExperimentResult> {
    let cfg = (*config).clone();
    let engine = Engine::get()?;
    let strat = strategy_overrides.unwrap_or_default();
    let ind = indicator_overrides.unwrap_or_default();

    // Release the GIL so rayon workers can acquire it.
    Ok(py.detach(|| engine.run_experiment(&cfg, verbose, &strat, &ind))?)
}
