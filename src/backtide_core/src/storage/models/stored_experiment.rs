//! Lightweight summary record for a stored experiment.
//!
//! Used by [`Storage::query_experiments`] to power the search UI on
//! the results page.

/// One stored experiment, plus enough metadata to render it in a list.
#[derive(Clone, Debug)]
pub struct StoredExperiment {
    pub id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub description: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub status: String,
    /// Best Sharpe ratio across the user-defined strategies.
    pub best_sharpe: Option<f64>,
    /// Number of strategies persisted under this experiment.
    pub n_strategies: i64,
}
