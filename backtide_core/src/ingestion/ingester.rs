//! Implementation of the [`DataIngester`].

use crate::config::Config;
use crate::ingestion::errors::{IngestionError, IngestionResult};
use crate::ingestion::provider::traits::DataProvider;
use crate::ingestion::provider::yahoo::YahooFinance;
use crate::ingestion::provider::Provider;
use crate::models::asset::{Asset, AssetType};
use crate::models::bar::Interval;
use crate::utils::http::{HttpClient, HttpError};
use futures::future::join_all;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use strum::IntoEnumIterator;
use tokio::runtime::Runtime;
use tracing::warn;

/// Process-wide data ingestion singleton.
static INGESTER: OnceLock<DataIngester> = OnceLock::new();

/// Singleton-like data ingestion struct.
pub struct DataIngester {
    /// Mapping of each asset type to its provider.
    providers: HashMap<AssetType, Arc<dyn DataProvider>>,

    /// Tokio runtime.
    rt: Runtime,

    /// Http client wrapper.
    pub client: HttpClient,
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

    /// Get a list of assets given their symbols.
    pub fn get_assets(
        &self,
        asset_type: AssetType,
        symbols: Vec<String>,
    ) -> IngestionResult<Vec<Asset>> {
        let config = Config::get()?;

        self.rt.block_on(async {
            let provider = self.providers.get(&asset_type).unwrap();
            let tasks: Vec<_> = symbols
                .iter()
                .map(|symbol| provider.get_asset(asset_type, symbol.as_str()))
                .collect();

            let results = join_all(tasks).await;

            // Collect to surface errors
            let assets: Vec<Asset> = results.into_iter().collect::<Result<Vec<_>, _>>()?;

            // Download logos if the logokit API key is configured
            if let Some(api_key) = &config.display.logokit_api_key {
                let logo_tasks = assets
                    .iter()
                    .map(|asset| async move {
                        let path = config
                            .ingestion
                            .storage_path
                            .join("logos")
                            .join(format!("{}.png", asset.symbol));

                        if !path.exists() {
                            if let Err(e) = self.download_logo(&asset, api_key, &path).await {
                                eprintln!("Failed to download logo for {}: {e}", asset.symbol);
                                warn!("Failed to download logo for {}: {e}", asset.symbol);
                            }
                        }
                    })
                    .collect::<Vec<_>>();

                join_all(logo_tasks).await;
            }

            Ok(assets)
        })
    }

    /// List available assets for a given asset type.
    pub fn list_assets(&self, asset_type: AssetType, limit: usize) -> IngestionResult<Vec<Asset>> {
        self.rt.block_on(self.providers.get(&asset_type).unwrap().list_assets(asset_type, limit))
    }

    /// List the available intervals for an asset type.
    pub fn list_intervals(&self, asset_type: AssetType) -> Vec<Interval> {
        let provider = self.providers.get(&asset_type).unwrap();
        provider.list_intervals()
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private interface
    // ────────────────────────────────────────────────────────────────────────

    /// Initialize the singleton from the active config.
    fn init() -> IngestionResult<Self> {
        let rt = Runtime::new()?;
        let client = HttpClient::new()?;
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
                    Provider::Yahoo => Arc::new(rt.block_on(YahooFinance::new())?),
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
            client,
        })
    }

    /// Download a logo from LogoKit and write it to `path`.
    async fn download_logo(
        &self,
        asset: &Asset,
        api_key: &String,
        path: &Path,
    ) -> IngestionResult<()> {
        let (param, symbol) = match asset.asset_type {
            AssetType::Forex => ("ticker", &format!("{}:CUR", asset.symbol)),
            AssetType::Crypto => ("crypto", &asset.currency),
            _ => ("ticker", &asset.symbol),
        };

        let url = format!("https://img.logokit.com/{param}/{symbol}?token={api_key}");

        let resp = self.client.get(&url, None).await?;
        let bytes = resp.bytes().await.map_err(|e| IngestionError::Http(HttpError::Decode(e)))?;

        // Ensure the logos directory exists
        if let Some(parent) = path.parent() {
            eprintln!("parent: {parent:?}");
            tokio::fs::create_dir_all(parent).await.map_err(|e| IngestionError::Io(e))?;
        }

        tokio::fs::write(path, &bytes).await.map_err(|e| IngestionError::Io(e))?;

        Ok(())
    }
}
