//! Implementation of the [`DataIngester`].

use pyo3::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use futures::future::join_all;
use strum::IntoEnumIterator;
use tokio::runtime::Runtime;

use crate::config::Config;
use crate::ingestion::errors::{IngestionError, IngestionResult};
use crate::ingestion::provider::traits::{DataProvider};
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
    // ────────────────────────────────────────────────────────────────────────
    // Public interface
    // ────────────────────────────────────────────────────────────────────────

    /// Return a `&'static` reference to the global ingester.
    pub fn get() -> Result<&'static DataIngester, IngestionError> {
        // Replace block with get_or_try_init when it becomes stable
        if let Some(cfg) = INGESTER.get() {
            Ok(cfg)
        } else {
            let _ = INGESTER.set(DataIngester::init()?);
            Ok(INGESTER.get().unwrap())
        }
    }

    /// Run the provider's list_assets.
    pub fn list_assets(&self, asset_type: AssetType, limit: usize) -> IngestionResult<Vec<Asset>> {
        self.rt.block_on(self.providers.get(&asset_type).unwrap().list_assets(asset_type, limit))
    }

    /// Fetch all symbols concurrently from the provider.
    pub fn get_assets(&self, asset_type: AssetType, symbols: Vec<String>) -> IngestionResult<Vec<Asset>> {
        self.rt.block_on(async {
            let provider = self.providers.get(&asset_type).unwrap();
            let tasks: Vec<_> = symbols
                .iter()
                .map(|symbol| provider.get_asset(asset_type, symbol.as_str()))
                .collect();

            join_all(tasks)
                .await
                .into_iter()
                .collect()
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private interface
    // ────────────────────────────────────────────────────────────────────────

    /// Initialize the singleton from the active config.
    fn init() -> Result<Self, IngestionError> {
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
                        Arc::new(rt.block_on(YahooFinance::new()).map_err(|e| IngestionError::Http(e))?)
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

    /// Download a logo from LogoKit and write it to `path`.
    async fn download_logo(
        &self,
        symbol: &str,
        path: &Path,
        api_key: String,
    ) -> Result<(), IngestionError> {
        let url = format!(
            "https://img.logokit.com/ticker/{symbol}?token={api_key}"
        );

        let resp = self.client.get(&url, None).await?;
        let bytes = resp.bytes().await.map_err(|e| {
            IngestionError::Http(format!("Failed to read logo bytes for {symbol}: {e}"))
        })?;

        // Ensure the logos directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                IngestionError::Io(format!("Failed to create logo cache dir: {e}"))
            })?;
        }

        tokio::fs::write(path, &bytes).await.map_err(|e| {
            IngestionError::Io(format!("Failed to write logo for {symbol}: {e}"))
        })?;

        Ok(())
    }
}
