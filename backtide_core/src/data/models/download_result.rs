//! Result of a bulk download operation.

use pyo3::pyclass;

/// Summary returned by [`download_assets`] after all tasks finish.
///
/// Individual task failures are captured as warnings rather than aborting
/// the entire download, so callers can report partial success.
///
/// Attributes
/// ----------
/// n_succeeded : int
///     Number of download tasks that succeeded.
///
/// n_failed : int
///     Number of download tasks that failed.
///
/// warnings : list[str]
///     Human-readable warning for each failed task.
///
/// See Also
/// --------
/// - backtide.data:DownloadInfo
/// - backtide.data:download_assets
#[derive(Debug, Clone)]
#[pyclass(from_py_object, get_all, frozen, module = "backtide.core.data")]
pub struct DownloadResult {
    pub n_succeeded: usize,
    pub n_failed: usize,
    pub warnings: Vec<String>,
}
