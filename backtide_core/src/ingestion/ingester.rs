//! Implementation of the [`DataIngester`].

use std::sync::{Arc, OnceLock};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use tokio::runtime::Runtime;

use crate::config::config;
use crate::ingestion::provider::traits::Provider;

static INGESTER: OnceLock<DataIngester> = OnceLock::new();

pub struct DataIngester {
    provider: Arc<dyn Provider>,
    rt: Runtime,
}

impl DataIngester {
    /// Initialize the singleton from the active config.
    pub fn new() -> PyResult<()> {
        let rt = Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to build Tokio runtime: {e}")))?;

        let provider: Arc<dyn Provider> = {
            let cfg = config();

            match cfg.provider.as_str() {
                "yahoo" => {
                    let yf = rt
                        .block_on(YahooFinance::new())
                        .map_err(|e| PyRuntimeError::new_err(format!("Yahoo auth failed: {e}")))?;
                    Arc::new(yf)
                }
                other => {
                    return Err(PyRuntimeError::new_err(format!(
                        "Unknown provider in config: {other:?}"
                    )))
                }
            }
        };

        // If another thread raced us here, OnceLock silently keeps the winner.
        let _ = INGESTER.set(DataIngester { provider, rt });
        Ok(())
    }

    /// Return the singleton, or error if [`init`] has not been called yet.
    pub fn get() -> PyResult<&'static DataIngester> {
        INGESTER.get().ok_or_else(|| {
            PyRuntimeError::new_err(
                "DataIngester is not initialised — call backtide.init() first",
            )
        })
    }

    /// Block the calling thread until the provider's list_assets resolves.
    pub fn list_assets(&self, asset_type: AssetType) -> PyResult<Vec<Asset>> {
        self.rt
            .block_on(self.provider.list_assets(asset_type))
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
}
