use crate::constants::Symbol;
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::interval::Interval;
use pyo3::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

/// A wrapper around an asset with additional metadata.
///
/// Provides the information required to download an asset, including the
/// download period and required currency conversions to reach the `base_currency`.
///
/// Attributes
/// ----------
/// asset : [Asset]
///     Asset for which to provide the metadata.
///
/// earliest_ts : dict[[Interval], int]
///     Per interval, the earliest timestamp for which there is data (in UNIX
///     seconds).
///
/// latest_ts : dict[[Interval], int]
///     Per interval, the most recent timestamp for which there is data (in UNIX
///     seconds).
///
/// legs : list[str]
///     Symbols of the currency pairs required to convert from this asset
///     to the base_currency.
///
/// See Also
/// --------
/// - backtide.data:Asset
/// - backtide.data:Bar
/// - backtide.data:DownloadInfo
#[pyclass(from_py_object, get_all, frozen, module = "backtide.data")]
#[derive(Debug, Clone, Deserialize)]
pub struct AssetMeta {
    pub asset: Asset,
    pub earliest_ts: HashMap<Interval, u64>,
    pub latest_ts: HashMap<Interval, u64>,
    pub legs: Vec<Symbol>,
}

#[pymethods]
impl AssetMeta {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    fn new(
        asset: Asset,
        earliest_ts: HashMap<Interval, u64>,
        latest_ts: HashMap<Interval, u64>,
        legs: Vec<Symbol>,
    ) -> Self {
        Self {
            asset,
            earliest_ts,
            latest_ts,
            legs,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(
        Bound<'py, PyAny>,
        (Asset, HashMap<Interval, u64>, HashMap<Interval, u64>, Vec<Symbol>),
    )> {
        let cls = py.get_type::<Self>().into_any();
        Ok((
            cls,
            (
                self.asset.clone(),
                self.earliest_ts.clone(),
                self.latest_ts.clone(),
                self.legs.to_vec(),
            ),
        ))
    }

    fn __repr__(&self) -> String {
        let earliest: Vec<String> =
            self.earliest_ts.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        let latest: Vec<String> = self.latest_ts.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        format!(
            "AssetMeta(asset={}, earliest_ts={{{}}}, latest_ts={{{}}}, legs={:?})",
            self.asset.__repr__(),
            earliest.join(", "),
            latest.join(", "),
            self.legs,
        )
    }

    #[getter]
    fn symbol(&self) -> &str {
        &self.asset.symbol
    }
    #[getter]
    fn name(&self) -> &str {
        &self.asset.name
    }
    #[getter]
    fn base(&self) -> Option<&str> {
        self.asset.base.as_deref()
    }
    #[getter]
    fn quote(&self) -> &str {
        &self.asset.quote
    }
    #[getter]
    fn asset_type(&self) -> AssetType {
        self.asset.asset_type
    }
    #[getter]
    fn exchange(&self) -> &str {
        &self.asset.exchange
    }
}
