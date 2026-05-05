//! Python interface for the backtest module.

use crate::backtest::models::experiment_config::ExperimentConfig;
use crate::backtest::models::experiment_result::ExperimentResult;
use crate::engine::Engine;
use pyo3::prelude::*;

/// Run a backtest experiment with the provided configuration.
///
/// Performs the full pipeline end-to-end:
///
/// 1. Resolves and downloads any missing market data (skipped if already
///    present in the local database).
/// 2. Computes every selected indicator once over the entire dataset.
/// 3. Runs every selected strategy in parallel — each strategy has its own
///    independent portfolio, order book and equity curve.
/// 4. Persists the aggregated [`ExperimentResult`] (and per-strategy artifacts)
///    into the database.
///
/// Parameters
/// ----------
/// config : [ExperimentConfig]
///     The complete experiment configuration.
///
/// verbose : bool, default=True
///     Whether to display a progress bar while running.
///
/// Returns
/// -------
/// [ExperimentResult]
///     The aggregated result of the run.
///
/// See Also
/// --------
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:ExperimentResult
/// - backtide.storage:query_experiments
///
/// Examples
/// --------
/// ```pycon
/// from backtide.backtest import ExperimentConfig, run_experiment
///
/// cfg = ExperimentConfig()
/// result = run_experiment(cfg)
/// print(result)
/// ```
#[pyfunction]
#[pyo3(signature = (config: "ExperimentConfig", *, verbose: "bool" = true) -> "ExperimentResult")]
pub fn run_experiment(
    py: Python<'_>,
    config: PyRef<'_, ExperimentConfig>,
    verbose: bool,
) -> PyResult<ExperimentResult> {
    let cfg = config.clone();
    let engine = Engine::get()?;

    // Release the GIL so rayon workers can acquire it.
    Ok(py.detach(|| engine.run_experiment(&cfg, verbose))?)
}
