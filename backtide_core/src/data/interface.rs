//! Python interface for the data module.

use crate::constants::Symbol;
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
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

/// Get assets given their symbols.
///
/// Unlike [`list_assets`], the returned assets contain all defined metadata
/// fields.
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

/// List available assets for a given asset type.
///
/// The returned assets may not contain all the metadata fields exposed
/// in [`Asset`]. The function often doesn't return all available assets,
/// but a subset of the most important ones instead.
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
///     Major assets for the given asset type.
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

/// Validate a set of symbols.
///
/// Resolves all assets required to price the given symbols in the
/// project's base currency, including any triangulation intermediaries.
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
/// - backtide.data:get_assets
/// - backtide.data:list_assets
///
/// Examples
/// --------
/// ```pycon
/// from backtide.data import validate_symbols
///
/// print(validate_symbols(["AAPL", "MSFT"], "stocks"))
/// ```
#[pyfunction]
#[pyo3(signature = (symbols: "str | Asset | Sequence[int | Asset]", asset_type: "str | AssetType") -> "list[Asset]")]
pub fn validate_symbols(
    symbols: Bound<'_, PyAny>,
    asset_type: Bound<'_, PyAny>,
) -> PyResult<Vec<Asset>> {
    let symbols = parse_asset(symbols)?;
    let asset_type = asset_type.extract::<AssetType>()?;

    let engine = Engine::get()?;
    Ok(engine.validate_symbols(symbols, asset_type)?)
}
