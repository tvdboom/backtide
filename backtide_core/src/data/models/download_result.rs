//! Result of a bulk download operation.

use pyo3::pyclass;

/// Summary returned by [`download_assets`] after all tasks finish.
///
/// Individual task failures are captured as warnings rather than aborting
/// the entire download, so callers can report partial success.
#[derive(Debug, Clone)]
#[pyclass(from_py_object, get_all, frozen, module = "backtide.core.data")]
pub struct DownloadResult {
    /// Number of (symbol, interval) tasks that succeeded.
    pub n_succeeded: usize,

    /// Number of (symbol, interval) tasks that failed.
    pub n_failed: usize,

    /// Human-readable warning for each failed task.
    ///
    /// Format: `"SYMBOL (interval): error message"`.
    pub warnings: Vec<String>,
}
