//! Global [`DataDownload`] singleton — the entry point for all data access.
//!
//! Wraps one or more [`DataProvider`] implementations (keyed by [`AssetType`]),
//! a shared Tokio runtime, and a TTL asset cache.

use crate::config::Config;
use crate::constants::{Symbol, ASSET_CACHE_TTL};
use crate::data::errors::DataResult;
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::currency::Currency;
use crate::data::providers::binance::Binance;
use crate::data::providers::provider::Provider;
use crate::data::providers::traits::DataProvider;
use crate::data::providers::yahoo::YahooFinance;
use crate::utils::tracing::ensure_tracing;
use futures::future::{join_all, try_join_all};
use indexmap::IndexMap;
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
            info!("Initializing data downloader.");
            let _ = DOWNLOADER.set(DataDownload::init()?);
            Ok(DOWNLOADER.get().unwrap())
        }
    }

    /// Fetch assets concurrently, using the cache where possible.
    #[instrument(skip(self), fields(count = symbols.len(), ?asset_type))]
    pub fn get_assets(
        &self,
        symbols: Vec<Symbol>,
        asset_type: AssetType,
    ) -> DataResult<Vec<Asset>> {
        self.rt.block_on(async {
            let tasks: Vec<_> = symbols.iter().map(|s| self.load_asset(s, asset_type)).collect();
            join_all(tasks).await.into_iter().collect::<Result<Vec<_>, _>>()
        })
    }

    /// List the most liquid assets for a given asset type, capped at `limit`.
    ///
    /// Delegates directly to the provider — callers should cache the result
    /// as this may trigger multiple network requests.
    #[instrument(skip(self), fields(?asset_type))]
    pub fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>> {
        self.rt.block_on(self.providers.get(&asset_type).unwrap().list_assets(asset_type, limit))
    }

    /// Resolves all assets required to price the given symbols in the
    /// portfolio base currency, including any triangulation intermediaries.
    ///
    /// For each symbol:
    ///  - If quote == base currency → no conversion needed.
    ///  - If quote is fiat          → triangulate via `triangulation_fiat`.
    ///  - If quote is crypto        → triangulate via `triangulation_crypto`.
    ///
    /// Returns the full flat list of assets (originals + triangulation legs).
    #[instrument(skip(self), fields(count = symbols.len(), ?asset_type))]
    pub fn validate_symbols(
        &self,
        symbols: Vec<Symbol>,
        asset_type: AssetType,
    ) -> DataResult<Vec<Asset>> {
        let cfg = Config::get()?;
        let base = &cfg.general.base_currency.to_string();
        let tri_fiat = &cfg.general.triangulation_fiat.to_string();
        let tri_crypto = &cfg.general.triangulation_crypto;
        let tri_crypto_pegged = &cfg.general.triangulation_crypto_pegged.to_string();

        self.rt.block_on(async {
            // Resolve all primary assets concurrently.
            let assets: Vec<Asset> =
                try_join_all(symbols.iter().map(|sym| self.load_asset(sym, asset_type))).await?;

            // Compute which triangulation legs are needed.
            // Use IndexMap to preserve insertion order while deduplicating by symbol.
            let mut leg_symbols: IndexMap<String, (String, String, AssetType)> = IndexMap::new();

            for asset in &assets {
                let quote = &asset.quote;
                let is_fiat = quote.parse::<Currency>().is_ok();

                // Skip if already denominated in base — no extra legs needed.
                if quote == base {
                    continue;
                }

                let at = if is_fiat {
                    AssetType::Forex
                } else {
                    AssetType::Crypto
                };

                // Try direct conversion first.
                if self.load_asset_bidirectional(quote, base, at).await.is_ok() {
                    leg_symbols
                        .entry(format!("{quote}-{base}"))
                        .or_insert_with(|| (quote.clone(), base.clone(), at));
                    continue;
                }

                // Fall back to triangulation.
                let (mid1, mid2) = if is_fiat {
                    (tri_fiat, tri_fiat)
                } else {
                    (tri_crypto, tri_crypto_pegged)
                };

                let mut insert_leg = |a: &str, b: &str| {
                    leg_symbols
                        .entry(format!("{a}-{b}"))
                        .or_insert_with(|| (a.to_string(), b.to_string(), at));
                };

                if quote != mid1 {
                    insert_leg(quote, mid1);
                }
                if mid2 != base {
                    insert_leg(mid2, base);
                }
            }

            // Fetch each unique leg symbol exactly once, concurrently.
            let legs: Vec<Asset> = try_join_all(
                leg_symbols.values().map(|(a, b, at)| self.load_asset_bidirectional(a, b, *at)),
            )
            .await?;

            Ok(assets.into_iter().chain(legs).collect())
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private interface
    // ────────────────────────────────────────────────────────────────────────

    /// Build the singleton from the active [`Config`].
    ///
    /// Provider instances are deduplicated — if two asset types share the same
    /// [`Provider`] variant they receive the same [`Arc`].
    fn init() -> DataResult<Self> {
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
                    Provider::Binance => Arc::new(rt.block_on(Binance::new())?),
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

    /// Try to load an asset from symbol format base-quote, else quote-base
    async fn load_asset_bidirectional(&self, a: &str, b: &str, at: AssetType) -> DataResult<Asset> {
        match self.load_asset(&format!("{a}-{b}"), at).await {
            Ok(asset) => Ok(asset),
            Err(_) => self.load_asset(&format!("{b}-{a}"), at).await,
        }
    }
}
