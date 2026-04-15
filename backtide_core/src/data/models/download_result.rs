//! Result of a bulk download operation.

use pyo3::{pyclass, pymethods};

/// Summary returned by [`download_bars`] after all tasks finish.
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
/// - backtide.data:download_bars
/// - backtide.data:fetch_instruments
/// - backtide.data:list_instruments
#[derive(Debug, Clone)]
#[pyclass(from_py_object, get_all, frozen, module = "backtide.core.data")]
pub struct DownloadResult {
    pub n_succeeded: usize,
    pub n_failed: usize,
    pub warnings: Vec<String>,
}

#[pymethods]
impl DownloadResult {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    fn __repr__(&self) -> String {
        format!(
            "DownloadResult(n_succeeded={}, n_failed={}, warnings=[{}])",
            self.n_succeeded,
            self.n_failed,
            self.warnings.iter().map(|w| format!("{w:?}")).collect::<Vec<_>>().join(", ")
        )
    }
}
