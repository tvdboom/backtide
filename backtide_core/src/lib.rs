mod data;
mod utils;

use crate::data::asset::Asset;
use crate::data::provider::yahoo::YahooFinance;
use crate::data::MarketDataProvider;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use tokio::runtime::Runtime;

/// Python-facing market data client backed by Yahoo Finance.
///
/// Initializing this object authenticates with Yahoo Finance (fetches a crumb
/// and session cookie).  All `list_*` methods are synchronous from Python's
/// perspective — they block on a dedicated Tokio runtime internally.
///
/// # Errors
///
/// All methods raise `RuntimeError` if the underlying request fails after
/// three retries, so the frontend can catch and display the message directly.
///
/// # Example
///
/// ```python
/// from backtide.core import MarketData
///
/// md = MarketData()
/// for asset in md.get_stocks(100):
///     print(asset.symbol, asset.price)
/// ```
#[pyclass]
pub struct MarketData {
    provider: YahooFinance,
    runtime: Runtime,
}

#[pymethods]
impl MarketData {
    /// Create a new [`MarketData`] client and authenticate with Yahoo Finance.
    ///
    /// Raises `RuntimeError` if authentication fails.
    #[new]
    fn new() -> PyResult<Self> {
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let provider = runtime.block_on(YahooFinance::new()).map_err(PyErr::from)?;

        Ok(Self {
            provider,
            runtime,
        })
    }

    /// Return the top `limit` most active stocks across US, European and Asian
    /// exchanges, sorted by descending volume.
    ///
    /// :param limit: Maximum number of assets to return (default 300).
    /// :raises RuntimeError: If the request fails after three retries.
    #[pyo3(signature = (limit = 300))]
    fn list_stocks(&self, limit: usize) -> PyResult<Vec<Asset>> {
        self.runtime.block_on(self.provider.list_stocks(limit)).map_err(PyErr::from)
    }

    /// Return the top `limit` most active forex pairs sorted by volume.
    ///
    /// :param limit: Maximum number of assets to return (default 300).
    /// :raises RuntimeError: If the request fails after three retries.
    #[pyo3(signature = (limit = 300))]
    fn list_forex(&self, limit: usize) -> PyResult<Vec<Asset>> {
        self.runtime.block_on(self.provider.list_forex(limit)).map_err(PyErr::from)
    }

    /// Return the top `limit` most active ETFs sorted by volume.
    ///
    /// :param limit: Maximum number of assets to return (default 300).
    /// :raises RuntimeError: If the request fails after three retries.
    #[pyo3(signature = (limit = 300))]
    fn list_etf(&self, limit: usize) -> PyResult<Vec<Asset>> {
        self.runtime.block_on(self.provider.list_etf(limit)).map_err(PyErr::from)
    }

    /// Return the top `limit` most active cryptocurrencies sorted by volume.
    ///
    /// :param limit: Maximum number of assets to return (default 300).
    /// :raises RuntimeError: If the request fails after three retries.
    #[pyo3(signature = (limit = 300))]
    fn list_crypto(&self, limit: usize) -> PyResult<Vec<Asset>> {
        self.runtime.block_on(self.provider.list_crypto(limit)).map_err(PyErr::from)
    }
}

// ─── Module registration ─────────────────────────────────────────────────────

#[pymodule]
fn core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<MarketData>()?;
    m.add_class::<Asset>()?;
    Ok(())
}
