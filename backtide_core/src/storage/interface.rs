//! Python interface for the storage module.

use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;
use crate::engine::Engine;
use crate::storage::models::storage_summary::StorageSummary;
use pyo3::prelude::*;

/// Return a summary of all data stored in the database.
///
/// Returns
/// -------
/// list[[StorageSummary]]
///     One entry per (symbol, interval, provider) group with metadata.
///
/// See Also
/// --------
/// - backtide.storage:delete_rows
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import get_summary
///
/// # Show what is currently stored
/// for row in get_summary():
///     print(f"{row.symbol:>10} {row.interval:>3}  {row.provider:<8}  {row.n_rows} bars")
/// ```
#[pyfunction]
pub fn get_summary() -> PyResult<Vec<StorageSummary>> {
    let engine = Engine::get()?;
    Ok(engine.get_summary()?)
}

/// Delete bars from the database.
///
/// Parameters
/// ----------
/// symbol : str | Sequence[str]
///     The symbols to delete.
///
/// interval : str | [Interval] | None = None
///     The bar interval for which to remove the data. If `None`, all
///     intervals will be deleted.
///
/// provider : str | [Provider] | None = None
///     The data provider for which to remove the data. If `None`, all
///     providers will be deleted.
///
/// Returns
/// -------
/// int
///     Number of rows deleted.
///
/// See Also
/// --------
/// - backtide.storage:get_summary
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import delete_rows
///
/// # Delete all stored data for a single symbol
/// delete_rows("AAPL")  # norun
///
/// # Delete all daily bars from for multiple symbols
/// delete_rows(["BTC-USDT", "ETH-USDT"], interval="1d")  # norun
/// ```
#[pyfunction]
#[pyo3(signature = (symbol: "str | Sequence[str]", interval: "str | Interval | None"=None, provider: "str | Provider | None"=None))]
pub fn delete_rows(
    symbol: Bound<'_, PyAny>,
    interval: Option<Bound<'_, PyAny>>,
    provider: Option<Bound<'_, PyAny>>,
) -> PyResult<u64> {
    let symbols: Vec<String> = if let Ok(s) = symbol.extract::<String>() {
        vec![s]
    } else {
        symbol.extract::<Vec<String>>()?
    };

    let provider = provider.map(|p| p.extract::<Provider>()).transpose()?;
    let interval = interval.map(|i| i.extract::<Interval>()).transpose()?;

    let engine = Engine::get()?;

    let mut total = 0u64;
    for sym in &symbols {
        total += engine.delete_rows(sym, interval, provider)?;
    }
    Ok(total)
}
