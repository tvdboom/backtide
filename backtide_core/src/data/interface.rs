//! Python interface for the data module.

use crate::constants::Symbol;
use crate::data::download::DataDownload;
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::interval::Interval;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyAnyMethods;
use pyo3::{pyfunction, Bound, PyAny, PyResult};

/// Get a single asset given its symbols.
///
/// The returned assets contain all defined metadata fields.
///
/// Parameters
/// ----------
/// symbol : str
///     Symbol for which to get the asset. The symbol should be of the form
///     expected by the [provider][nom-provider] corresponding to the selected
///     `asset_type`.
///
/// asset_type : str | [`AssetType`]
///     For which [asset type] to get the asset.
///
/// See Also
/// --------
/// - backtide.data:get_assets
/// - backtide.data:list_assets
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import get_asset
///
/// print(get_asset("AAPL", "stocks"))
/// ```
#[pyfunction]
pub fn get_asset(symbol: Symbol, asset_type: Bound<'_, PyAny>) -> PyResult<Asset> {
    let asset_type: AssetType = if let Ok(s) = asset_type.extract::<String>() {
        s.parse().map_err(|_| PyValueError::new_err(format!("invalid asset type: {s}")))?
    } else {
        asset_type.extract::<AssetType>()?
    };

    let ingester = DataDownload::get()?;
    Ok(ingester.get_asset(symbol, asset_type)?)
}

/// Get a list of assets given their symbols.
///
/// The returned assets contain all defined metadata fields.
///
/// Parameters
/// ----------
/// symbols : list[str]
///     Symbols for which to get the asset. The symbols should be of the form
///     expected by the [provider][nom-provider] corresponding to the selected
///     `asset_type`.
///
/// asset_type : str | [`AssetType`]
///     For which [asset type] to get the assets.
///
/// See Also
/// --------
/// - backtide.data:get_asset
/// - backtide.data:list_assets
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import get_assets
///
/// print(get_assets(["AAPL", "MSFT"], "stocks"))
/// ```
#[pyfunction]
pub fn get_assets(symbols: Vec<Symbol>, asset_type: Bound<'_, PyAny>) -> PyResult<Vec<Asset>> {
    let asset_type: AssetType = if let Ok(s) = asset_type.extract::<String>() {
        s.parse().map_err(|_| PyValueError::new_err(format!("invalid asset type: {s}")))?
    } else {
        asset_type.extract::<AssetType>()?
    };

    let ingester = DataDownload::get()?;
    Ok(ingester.get_assets(symbols, asset_type)?)
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
///     For which [asset type] to list the assets.
///
/// limit : int, default=100
///     Maximum number of assets to return. The actual number may be smaller,
///     but not larger.
///
/// See Also
/// --------
/// - backtide.data:get_asset
/// - backtide.data:get_assets
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import list_assets
///
/// print(list_assets("crypto", limit=5))
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
    Ok(ingester.list_assets(asset_type, limit)?)
}
