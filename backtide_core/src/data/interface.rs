//! Python interface for the data module.

use crate::data::download::DataDownload;
use crate::data::models::asset::{Asset, AssetType};
use crate::data::models::bar::Interval;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::PyAnyMethods;
use pyo3::{pyfunction, Bound, PyAny, PyResult};

/// Get a list of assets given their symbols.
///
/// The returned assets should contain defined all metadata fields.
///
/// Parameters
/// ----------
/// asset_type : str | [`AssetType`]
///     For which [asset type][nom-asset-type] to get the assets.
///
/// symbols : list[str]
///     Symbols for which to get the asset. The symbols should be of the form
///     expected by the [provider][nom-provider] corresponding to the selected
///     `asset_type`.
///
/// See Also
/// --------
/// - backtide.data:list_assets
/// - backtide.data:list_intervals
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import get_assets
///
/// print(get_assets("stocks", ["APPL", "MSFT"]))
/// ```
#[pyfunction]
pub fn get_assets(asset_type: Bound<'_, PyAny>, symbols: Vec<String>) -> PyResult<Vec<Asset>> {
    let asset_type: AssetType = if let Ok(s) = asset_type.extract::<String>() {
        s.parse().map_err(|_| PyValueError::new_err(format!("invalid asset type: {s}")))?
    } else {
        asset_type.extract::<AssetType>()?
    };

    let ingester = DataDownload::get()?;
    ingester.get_assets(asset_type, symbols).map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

/// List available assets for a given asset type.
///
/// The returned assets may not contain all the metadata fields exposed
/// in [`Asset`]. The function often doesn't return all available assets,
/// but a subset of the most important ones instead.
///
/// Parameters
/// ----------
/// asset_type : str | [`AssetType`]
///     For which [asset type][nom-asset-type] to list the assets.
///
/// limit : int, default=100
///     Maximum number of assets to return. The actual number may be smaller,
///     but not larger.
///
/// See Also
/// --------
/// - backtide.data:get_assets
/// - backtide.data:list_intervals
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import list_assets
///
/// print(list_assets("crypto"))
/// ```
#[pyfunction]
#[pyo3(signature = (asset_type, limit=100))]
pub fn list_assets(asset_type: Bound<'_, PyAny>, limit: usize) -> PyResult<Vec<Asset>> {
    let asset_type: AssetType = if let Ok(s) = asset_type.extract::<String>() {
        s.parse().map_err(|_| PyValueError::new_err(format!("invalid asset type: {s}")))?
    } else {
        asset_type.extract::<AssetType>()?
    };

    let ingester = DataDownload::get()?;
    ingester.list_assets(asset_type, limit).map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

/// List the available intervals for an asset type.
///
/// Parameters
/// ----------
/// asset_type : str | [`AssetType`]
///     For which [asset type][nom-asset-type] to get the [intervals][nom-interval].
///
/// See Also
/// --------
/// - backtide.data:get_assets
/// - backtide.data:list_assets
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import list_intervals
///
/// print(list_intervals("stocks"))
/// ```
#[pyfunction]
pub fn list_intervals(asset_type: Bound<'_, PyAny>) -> PyResult<Vec<Interval>> {
    let asset_type: AssetType = if let Ok(s) = asset_type.extract::<String>() {
        s.parse().map_err(|_| PyValueError::new_err(format!("invalid asset type: {s}")))?
    } else {
        asset_type.extract::<AssetType>()?
    };

    let ingester = DataDownload::get()?;
    Ok(ingester.list_intervals(asset_type))
}
