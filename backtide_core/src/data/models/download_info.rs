use crate::data::models::asset::Asset;
use crate::data::models::asset_meta::AssetMeta;
use pyo3::{pyclass, pymethods, Bound, PyAny, PyResult, Python};
use serde::Deserialize;

/// All information required to download a set of symbols.
///
/// Attributes
/// ----------
/// assets : list[[AssetMeta]]
///     Assets with corresponding metadata to download.
///
/// legs : list[[AssetMeta]]
///     Assets with metadata for the conversion legs that `assets` require
///     to convert their currencies to the base currency.
///
/// See Also
/// --------
/// - backtide.data:Asset
/// - backtide.data:AssetMeta
/// - backtide.data:Interval
#[pyclass(from_py_object, get_all, frozen, module = "backtide.data")]
#[derive(Clone, Debug, Deserialize)]
pub struct DownloadInfo {
    /// All assets including injected forex dependencies.
    pub assets: Vec<AssetMeta>,

    /// Currency legs to convert from every asset to the base currency.
    pub legs: Vec<AssetMeta>,
}

#[pymethods]
impl DownloadInfo {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    fn new(assets: Vec<AssetMeta>, legs: Vec<AssetMeta>) -> Self {
        Self {
            assets,
            legs,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (Vec<AssetMeta>, Vec<AssetMeta>))> {
        let cls = py.get_type::<Asset>().into_any();
        Ok((cls, (self.assets.clone(), self.legs.clone())))
    }

    fn __repr__(&self) -> String {
        format!("DownloadInfo(assets={:?}, legs={:?})", self.assets.to_vec(), self.legs.to_vec())
    }
}
