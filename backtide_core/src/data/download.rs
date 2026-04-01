//! Global [`DataDownload`] singleton — the entry point for all data access.
//!
//! Wraps one or more [`DataProvider`] implementations (keyed by [`AssetType`]),
//! a shared Tokio runtime, and a TTL asset cache.

use crate::config::Config;
use crate::constants::{Symbol, ASSET_CACHE_TTL};
use crate::data::errors::DataResult;
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;
use crate::data::providers::traits::DataProvider;
use crate::data::providers::yahoo::YahooFinance;
use crate::utils::tracing::ensure_tracing;
use futures::future::join_all;
use moka::future::Cache;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use strum::IntoEnumIterator;
use tokio::runtime::Runtime;
use tracing::{debug, info, instrument};

/// Process-wide [`DataDownload`] singleton.
static DOWNLOADER: OnceLock<DataDownload> = OnceLock::new();

/// Central data access handle.
///
/// Holds a provider per [`AssetType`], a dedicated Tokio runtime for
/// bridging sync callers to async providers, and a TTL-bounded in-memory
/// asset cache. Obtain via [`DataDownload::get`] — do not construct directly.
pub struct DataDownload {
    /// One provider arc per asset type, potentially shared across types.
    providers: HashMap<AssetType, Arc<dyn DataProvider>>,

    /// Dedicated runtime for blocking async calls from sync contexts.
    rt: Runtime,

    /// TTL asset cache keyed by symbol, avoiding redundant provider round-trips.
    cache: Cache<Symbol, Arc<Asset>>,
}

impl DataDownload {
    // ────────────────────────────────────────────────────────────────────────
    // Public interface
    // ────────────────────────────────────────────────────────────────────────

    /// Return a `&'static` reference to the global [`DataDownload`].
    ///
    /// Initializes the singleton on first call; subsequent calls are free.
    /// Returns an error if config loading or any provider handshake fails.
    pub fn get() -> DataResult<&'static DataDownload> {
        ensure_tracing(None)?;

        // Replace block with get_or_try_init when it becomes stable
        if let Some(cfg) = DOWNLOADER.get() {
            Ok(cfg)
        } else {
            let _ = DOWNLOADER.set(DataDownload::init()?);
            Ok(DOWNLOADER.get().unwrap())
        }
    }

    /// Fetch a single asset by symbol, using the cache when available.
    #[instrument(skip(self), fields(%symbol, ?asset_type))]
    pub fn get_asset(&self, symbol: Symbol, asset_type: AssetType) -> DataResult<Asset> {
        self.rt.block_on(self.load_asset(&symbol, asset_type))
    }

    /// Fetch multiple assets concurrently, using the cache where possible.
    #[instrument(skip(self), fields(count = symbols.len(), ?asset_type))]
    pub fn get_assets(
        &self,
        symbols: Vec<Symbol>,
        asset_type: AssetType,
    ) -> DataResult<Vec<Asset>> {
        debug!(count = symbols.len(), ?asset_type, "Fetching assets concurrently");
        self.rt.block_on(async {
            let tasks: Vec<_> = symbols.iter().map(|s| self.load_asset(s, asset_type)).collect();

            join_all(tasks).await.into_iter().collect::<Result<Vec<_>, _>>()
        })
    }

    /// List the most liquid assets for a given asset type, capped at `limit`.
    ///
    /// Delegates directly to the provider — callers should cache the result
    /// as this may trigger multiple network requests.
    pub fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>> {
        debug!(?asset_type, %limit, "Listing assets");
        self.rt.block_on(self.providers.get(&asset_type).unwrap().list_assets(asset_type, limit))
    }

    /// Return the bar intervals supported by the provider for `asset_type`.
    pub fn list_intervals(&self, asset_type: AssetType) -> Vec<Interval> {
        let provider = self.providers.get(&asset_type).unwrap();
        provider.list_intervals()
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private interface
    // ────────────────────────────────────────────────────────────────────────

    /// Build the singleton from the active [`Config`].
    ///
    /// Provider instances are deduplicated — if two asset types share the same
    /// [`Provider`] variant they receive the same [`Arc`].
    fn init() -> DataResult<Self> {
        info!("Initializing DataDownload singleton");

        let rt = Runtime::new()?;
        let pc = &Config::get()?.data.providers;

        let mut cache: HashMap<Provider, Arc<dyn DataProvider>> = HashMap::new();
        let mut providers: HashMap<AssetType, Arc<dyn DataProvider>> = HashMap::new();

        for asset_type in AssetType::iter() {
            let default = asset_type.default();
            let provider = pc.get(&asset_type).unwrap_or(&default);
            let p = if let Some(p) = cache.get(provider) {
                debug!(?asset_type, ?provider, "Reusing existing provider instance");
                p.clone()
            } else {
                debug!(?asset_type, ?provider, "Creating new provider instance");
                let p: Arc<dyn DataProvider> = match provider {
                    Provider::Yahoo => Arc::new(rt.block_on(YahooFinance::new())?),
                    _ => unreachable!(),
                };
                cache.insert(*provider, p.clone());
                p
            };

            providers.insert(asset_type, p);
        }

        info!(asset_types = providers.len(), "DataDownload initialized");
        Ok(Self {
            providers,
            rt,
            cache: Cache::builder().time_to_live(ASSET_CACHE_TTL).build(),
        })
    }

    /// Resolve an asset, returning the cached value if still live.
    ///
    /// On a cache miss the provider is queried and the result is inserted
    /// before returning.
    async fn load_asset(&self, symbol: &Symbol, asset_type: AssetType) -> DataResult<Asset> {
        if let Some(asset) = self.cache.get(symbol).await {
            debug!(%symbol, "Asset cache hit");
            return Ok(asset.as_ref().clone());
        }

        let provider = self.providers.get(&asset_type).unwrap();
        let asset = provider.get_asset(symbol, asset_type).await?;
        self.cache.insert(symbol.clone(), Arc::new(asset.clone())).await;
        debug!(%symbol, "Asset cached");
        Ok(asset)
    }
}
