//! Implementation of the [`DataIngester`].

use anyhow::Context;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use strum::IntoEnumIterator;
use tokio::runtime::Runtime;

use crate::config::Config;
use crate::ingestion::provider::traits::{DataProvider, ProviderResult};
use crate::ingestion::provider::yahoo::YahooFinance;
use crate::ingestion::provider::Provider;
use crate::models::asset::{Asset, AssetType};

/// Process-wide data ingestion singleton.
static INGESTER: OnceLock<DataIngester> = OnceLock::new();

/// Singleton-like data ingestion struct.
pub struct DataIngester {
    /// Mapping of each asset type to its provider.
    providers: HashMap<AssetType, Arc<dyn DataProvider>>,

    /// Tokio runtime.
    rt: Runtime,
}

impl DataIngester {
    /// Initialize the singleton from the active config.
    pub fn init() -> Result<Self, anyhow::Error> {
        let rt = Runtime::new().context("Failed to build Tokio runtime")?;
        let pc = &Config::get()?.ingestion.providers;

        // One Arc per unique provider variant — shared across asset types.
        let mut cache: HashMap<Provider, Arc<dyn DataProvider>> = HashMap::new();
        let mut providers: HashMap<AssetType, Arc<dyn DataProvider>> = HashMap::new();

        for asset_type in AssetType::iter() {
            let default = asset_type.default();
            let provider = pc.get(&asset_type).unwrap_or(&default);
            let p = if let Some(p) = cache.get(&provider) {
                p.clone()
            } else {
                let p: Arc<dyn DataProvider> = match provider {
                    Provider::Yahoo => {
                        Arc::new(rt.block_on(YahooFinance::new()).context("Yahoo auth failed")?)
                    },
                    _ => unreachable!(),
                };
                cache.insert(*provider, p.clone());
                p
            };

            providers.insert(asset_type, p);
        }

        Ok(Self {
            providers,
            rt,
        })
    }

    /// Return a `&'static` reference to the global ingester.
    pub fn get() -> Result<&'static DataIngester, anyhow::Error> {
        // Replace block with get_or_try_init when it becomes stable
        if let Some(cfg) = INGESTER.get() {
            Ok(cfg)
        } else {
            let _ = INGESTER.set(DataIngester::init()?);
            Ok(INGESTER.get().unwrap())
        }
    }

    /// Block the calling thread until the provider's list_assets resolves.
    pub fn list_assets(&self, asset_type: AssetType, limit: usize) -> ProviderResult<Vec<Asset>> {
        self.rt.block_on(self.providers.get(&asset_type).unwrap().list_assets(asset_type, limit))
    }
}

#[pyfunction]
pub fn list_assets(asset_type: AssetType, limit: usize) -> PyResult<Vec<Asset>> {
    let ingester = DataIngester::get().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    ingester.list_assets(asset_type, limit).map_err(|e| PyRuntimeError::new_err(e.to_string()))
}
