//! Python interface for the data module.

use crate::constants::Symbol;
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::download_info::DownloadInfo;
use crate::data::models::interval::Interval;
use crate::engine::Engine;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyAnyMethods;
use pyo3::{pyfunction, Bound, PyAny, PyResult};

/// Parse input from Python into a list of symbols.
fn parse_asset(symbols: Bound<'_, PyAny>) -> PyResult<Vec<Symbol>> {
    if let Ok(seq) = symbols.extract::<Vec<Bound<'_, PyAny>>>() {
        // Parse symbols: Sequence[str | Asset]
        seq.into_iter()
            .map(|item| {
                if let Ok(symbol) = item.extract::<String>() {
                    Ok(symbol)
                } else if let Ok(asset) = item.extract::<Asset>() {
                    Ok(asset.symbol)
                } else {
                    Err(PyValueError::new_err(
                        "Parameter symbols must be a str, Asset or a sequence of those.",
                    ))
                }
            })
            .collect::<PyResult<_>>()
    } else {
        // Parse symbols: str | Asset
        if let Ok(symbol) = symbols.extract::<String>() {
            Ok(vec![symbol])
        } else if let Ok(asset) = symbols.extract::<Asset>() {
            Ok(vec![asset.symbol])
        } else {
            Err(PyValueError::new_err(
                "Parameter symbols must be a str, Asset or a sequence of those.",
            ))
        }
    }
}

/// Parse input from Python into a vec of intervals.
fn parse_interval(interval: Bound<'_, PyAny>) -> PyResult<Vec<Interval>> {
    if let Ok(seq) = interval.extract::<Vec<Bound<'_, PyAny>>>() {
        // Parse symbols: Sequence[str | Interval]
        seq.into_iter().map(|item| item.extract::<Interval>()).collect::<PyResult<_>>()
    } else {
        // Parse symbols: str | Asset
        Ok(vec![interval.extract::<Interval>()?])
    }
}

/// Get assets given their symbols.
///
/// Parameters
/// ----------
/// symbols : str | [Asset] | list[str | [Asset]]
///     Symbols for which to get the assets. The symbols should be of the
///     [canonical form][nom-symbol] expected by backtide.
///
/// asset_type : str | [AssetType]
///     For which [asset type] to get the assets.
///
/// Returns
/// -------
/// list[[Asset]]
///     Assets corresponding to the provided symbols.
///
/// See Also
/// --------
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
#[pyo3(signature = (symbols: "str | Asset | Sequence[int | Asset]", asset_type: "str | AssetType") -> "list[Asset]")]
pub fn get_assets(symbols: Bound<'_, PyAny>, asset_type: Bound<'_, PyAny>) -> PyResult<Vec<Asset>> {
    let symbols = parse_asset(symbols)?;
    let asset_type = asset_type.extract::<AssetType>()?;

    let engine = Engine::get()?;
    Ok(engine.get_assets(symbols, asset_type)?)
}

/// Retrieve the required download information for a set of symbols.
///
/// Resolves all assets corresponding to the provided symbols. Also resolves
/// the required assets to convert the given symbols to the base currency,
/// including any triangulation intermediaries.
///
/// Parameters
/// ----------
/// symbols : str | [Asset] | list[str | [Asset]]
///     Symbols for which to get the assets. The symbols should be of the
///     [canonical form][nom-symbol] expected by backtide.
///
/// asset_type : str | [AssetType]
///     For which [asset type] to get the assets.
///
/// interval : str | [Interval] | list[str | [Interval]]
///     Interval(s) for which to resolve the download information.
///
/// Returns
/// -------
/// [DownloadInfo]
///     Assets and currency legs corresponding to the provided symbols.
///
/// See Also
/// --------
/// - backtide.data:get_assets
/// - backtide.data:list_assets
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import get_download_info
///
/// print(get_download_info(["AAPL", "MSFT"], "stocks", "1d"))
/// ```
#[pyfunction]
#[pyo3(signature = (symbols: "str | Asset | Sequence[int | Asset]", asset_type: "str | AssetType", interval: "str | Interval | Sequcen[str | Interval]") -> "DownloadInfo")]
pub fn get_download_info(
    symbols: Bound<'_, PyAny>,
    asset_type: Bound<'_, PyAny>,
    interval: Bound<'_, PyAny>,
) -> PyResult<DownloadInfo> {
    let symbols = parse_asset(symbols)?;
    let asset_type = asset_type.extract::<AssetType>()?;
    let interval = parse_interval(interval)?;

    let engine = Engine::get()?;
    Ok(engine.get_download_info(symbols, asset_type, interval)?)
}

/// List available assets for a given asset type.
///
/// The function may not return all available assets, but a subset of the most
/// important ones instead.
///
/// Parameters
/// ----------
/// asset_type : str | [AssetType]
///     For which [asset type] to list the assets.
///
/// limit : int, default=100
///     Maximum number of assets to return. The actual number may be smaller,
///     but not larger.
///
/// Returns
/// -------
/// list[[Asset]]
///     Assets for the given asset type.
///
/// See Also
/// --------
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
#[pyo3(signature = (asset_type: "str | AssetType", limit: "int"=100))]
pub fn list_assets(asset_type: Bound<'_, PyAny>, limit: usize) -> PyResult<Vec<Asset>> {
    let asset_type = asset_type.extract::<AssetType>()?;

    let engine = Engine::get()?;
    Ok(engine.list_assets(asset_type, limit)?)
}
