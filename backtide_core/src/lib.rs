//! `market_data` — async market data with Python bindings.
//!
//! # Architecture
//!
//! ```text
//! Python
//!   └── MarketData (PyO3 wrapper)
//!         └── YahooFinance (impl MarketDataProvider)
//!               ├── auth    — crumb + cookie auth
//!               └── http    — paginated screener calls with retry
//! ```
//!
//! # Python usage
//!
//! ```python
//! import market_data
//!
//! md = market_data.MarketData()
//! stocks = md.get_stocks(300)   # list[Asset]
//! crypto = md.get_crypto(300)
//! etfs   = md.get_etf(300)
//! forex  = md.get_forex(300)
//! ```

pub mod error;
pub mod models;
pub mod provider;
pub mod yahoo;

use pyo3::prelude::*;
use tokio::runtime::Runtime;

use crate::models::Asset;
use crate::provider::MarketDataProvider;
use crate::yahoo::YahooFinance;

// ─── Python-facing wrapper ────────────────────────────────────────────────────

/// Python-facing market data client backed by Yahoo Finance.
///
/// Initialising this object authenticates with Yahoo Finance (fetches a crumb
/// and session cookie).  All `get_*` methods are synchronous from Python's
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
/// import market_data
///
/// md = market_data.MarketData()
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
        let runtime =
            Runtime::new().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let provider = runtime.block_on(YahooFinance::new()).map_err(pyo3::PyErr::from)?;

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
    fn get_stocks(&self, limit: usize) -> PyResult<Vec<Asset>> {
        self.runtime.block_on(self.provider.get_stocks(limit)).map_err(pyo3::PyErr::from)
    }

    /// Return the top `limit` most active forex pairs sorted by volume.
    ///
    /// :param limit: Maximum number of assets to return (default 300).
    /// :raises RuntimeError: If the request fails after three retries.
    #[pyo3(signature = (limit = 300))]
    fn get_forex(&self, limit: usize) -> PyResult<Vec<Asset>> {
        self.runtime.block_on(self.provider.get_forex(limit)).map_err(pyo3::PyErr::from)
    }

    /// Return the top `limit` most active ETFs sorted by volume.
    ///
    /// :param limit: Maximum number of assets to return (default 300).
    /// :raises RuntimeError: If the request fails after three retries.
    #[pyo3(signature = (limit = 300))]
    fn get_etf(&self, limit: usize) -> PyResult<Vec<Asset>> {
        self.runtime.block_on(self.provider.get_etf(limit)).map_err(pyo3::PyErr::from)
    }

    /// Return the top `limit` most active cryptocurrencies sorted by volume.
    ///
    /// :param limit: Maximum number of assets to return (default 300).
    /// :raises RuntimeError: If the request fails after three retries.
    #[pyo3(signature = (limit = 300))]
    fn get_crypto(&self, limit: usize) -> PyResult<Vec<Asset>> {
        self.runtime.block_on(self.provider.get_crypto(limit)).map_err(pyo3::PyErr::from)
    }
}

// ─── Module registration ─────────────────────────────────────────────────────

/// Register the Python module.
#[pymodule]
fn market_data(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<MarketData>()?;
    m.add_class::<Asset>()?;
    Ok(())
}
