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

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::exceptions::PyAttributeError;
    use pyo3::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case(0, 0, vec![], "DownloadResult(n_succeeded=0, n_failed=0, warnings=[])")]
    #[case(2, 1, vec!["no data".to_owned()], r#"DownloadResult(n_succeeded=2, n_failed=1, warnings=["no data"])"#)]
    fn exposes_a_frozen_python_snapshot(
        #[case] succeeded: usize,
        #[case] failed: usize,
        #[case] warnings: Vec<String>,
        #[case] representation: &str,
    ) -> PyResult<()> {
        Python::attach(|py| {
            let result = Py::new(
                py,
                DownloadResult {
                    n_succeeded: succeeded,
                    n_failed: failed,
                    warnings: warnings.clone(),
                },
            )?;
            let bound = result.bind(py);

            assert_eq!(bound.getattr("n_succeeded")?.extract::<usize>()?, succeeded);
            assert_eq!(bound.getattr("n_failed")?.extract::<usize>()?, failed);
            assert_eq!(bound.repr()?.to_str()?, representation);
            let error = bound.setattr("n_succeeded", 999).unwrap_err();
            assert!(error.is_instance_of::<PyAttributeError>(py));

            bound.getattr("warnings")?.call_method1("append", ("caller mutation",))?;
            assert_eq!(bound.getattr("warnings")?.extract::<Vec<String>>()?, warnings);
            Ok(())
        })
    }
}
