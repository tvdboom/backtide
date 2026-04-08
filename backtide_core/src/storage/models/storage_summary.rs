use pyo3::prelude::*;

/// Summary of one (symbol, provider, interval) group in storage.
///
/// Attributes
/// ----------
/// symbol : str
///     Canonical symbol name.
///
/// provider : str
///     Data provider that fetched the bars.
///
/// interval : str
///     Bar interval.
///
/// asset_type : str
///     Asset type.
///
/// first_ts : int
///     Earliest `open_ts` in Unix seconds.
///
/// last_ts : int
///     Latest ``open_ts`` in Unix seconds.
///
/// n_rows : int
///     Total number of stored bars.
///
/// sparkline : list[float]
///     Last 365 `adj_close` values (oldest → newest).
///
/// See Also
/// --------
/// - backtide.storage:get_summary
/// - backtide.storage:delete_rows
#[pyclass(from_py_object, get_all, frozen, module = "backtide.storage")]
#[derive(Clone, Debug)]
pub struct StorageSummary {
    pub symbol: String,
    pub provider: String,
    pub interval: String,
    pub asset_type: String,
    pub first_ts: u64,
    pub last_ts: u64,
    pub n_rows: u64,
    pub sparkline: Vec<f64>,
}
