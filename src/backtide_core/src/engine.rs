//! Engine module.
//!
//! Engine is loaded once into a process-wide singleton ([`Engine::get`]) from
//! any part of the Python interface. Logic per module (download, backtest, etc...)
//! are implemented directly on the engine.

use crate::config::interface::Config;
use crate::constants::Symbol;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::data::providers::binance::Binance;
use crate::data::providers::coinbase::Coinbase;
use crate::data::providers::kraken::Kraken;
use crate::data::providers::traits::DataProvider;
use crate::data::providers::yahoo::YahooFinance;
use crate::errors::EngineResult;
use crate::storage::duckdb::DuckDb;
use crate::storage::traits::Storage;
use crate::utils::interface::init_logging_with_level;
use moka::future::Cache;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use strum::IntoEnumIterator;
use tokio::runtime::Runtime;
use tracing::{debug, info};

/// Process-wide configuration singleton.
static ENGINE: OnceLock<Engine> = OnceLock::new();

/// Cache storage for the engine.
pub struct EngineCache {
    /// TTL instrument cache.
    pub instrument_cache: Cache<Symbol, Arc<Instrument>>,

    /// TTL instrument range cache.
    pub range_cache: Cache<(Symbol, Interval), (u64, u64)>,
}

impl EngineCache {
    pub fn new() -> Self {
        Self {
            instrument_cache: Cache::builder().time_to_live(Duration::from_secs(2 * 3600)).build(),
            range_cache: Cache::builder().time_to_live(Duration::from_secs(1800)).build(),
        }
    }

    /// Invalidate all cache.
    pub fn clear(&self) {
        self.instrument_cache.invalidate_all();
        self.range_cache.invalidate_all();
    }
}

impl Default for EngineCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Backtide core engine.
pub struct Engine {
    /// Global configuration.
    pub config: &'static Config,

    /// Dedicated runtime for blocking async calls from sync contexts.
    pub rt: Runtime,

    /// One provider arc per instrument type, potentially shared across types.
    pub providers: HashMap<InstrumentType, Arc<dyn DataProvider>>,

    /// Database which stores all data.
    pub db: Box<dyn Storage>,

    /// In-memory cache.
    pub cache: EngineCache,
}

impl Engine {
    // ────────────────────────────────────────────────────────────────────────
    // Public interface
    // ────────────────────────────────────────────────────────────────────────

    /// Return a `&'static` reference to the global [`Engine`].
    ///
    /// Initializes the singleton on first call; subsequent calls are free.
    /// Returns an error if config loading or any provider handshake fails.
    pub fn get() -> EngineResult<&'static Self> {
        // Replace block with get_or_try_init when it becomes stable
        if let Some(cfg) = ENGINE.get() {
            Ok(cfg)
        } else {
            let _ = ENGINE.set(Self::init()?);
            info!("Engine initialized.");
            Ok(ENGINE.get().unwrap())
        }
    }

    /// Invalidate all cache in the engine.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private interface
    // ────────────────────────────────────────────────────────────────────────

    /// Build the singleton from the active [`Config`].
    ///
    /// Provider instances are deduplicated — if two instrument types share the
    /// same [`Provider`] variant they receive the same [`Arc`].
    fn init() -> EngineResult<Self> {
        // Load the configuration from the file or use default.
        let config = Config::get()?;
        init_logging_with_level(config.general.log_level);

        let rt = Runtime::new()?;
        let pc = &config.data.providers;

        // Provider instances are deduplicated — if two instrument types share the
        // same variant, they receive the same Arc.
        let mut cache: HashMap<Provider, Arc<dyn DataProvider>> = HashMap::new();
        let mut providers: HashMap<InstrumentType, Arc<dyn DataProvider>> = HashMap::new();

        for instrument_type in InstrumentType::iter() {
            let default = instrument_type.default_provider();
            let provider = pc.get(&instrument_type).unwrap_or(&default);
            let p = if let Some(p) = cache.get(provider) {
                debug!(?instrument_type, ?provider, "Reusing existing provider instance");
                p.clone()
            } else {
                debug!(?instrument_type, ?provider, "Creating new provider instance");
                let p: Arc<dyn DataProvider> = match provider {
                    Provider::Yahoo => Arc::new(rt.block_on(YahooFinance::new())?),
                    Provider::Binance => Arc::new(rt.block_on(Binance::new())?),
                    Provider::Coinbase => Arc::new(rt.block_on(Coinbase::new())?),
                    Provider::Kraken => Arc::new(rt.block_on(Kraken::new())?),
                };
                cache.insert(*provider, p.clone());
                p
            };

            providers.insert(instrument_type, p);
        }

        // Initialize the database and create all required tables
        let db = DuckDb::new(&config.data.storage_path)?;
        db.init()?;

        Ok(Self {
            config,
            providers,
            rt,
            db: Box::new(db),
            cache: EngineCache::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn engine_cache_new_creates_empty_caches() {
        let cache = EngineCache::new();
        assert_eq!(cache.instrument_cache.entry_count(), 0);
        assert_eq!(cache.range_cache.entry_count(), 0);
    }

    #[test]
    fn engine_cache_default_matches_new() {
        let cache = EngineCache::default();
        assert_eq!(cache.instrument_cache.entry_count(), 0);
        assert_eq!(cache.range_cache.entry_count(), 0);
    }

    #[test]
    fn engine_cache_clear_invalidates_range_entries() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        let key = ("AAPL".to_owned(), Interval::OneDay);

        rt.block_on(async {
            cache.range_cache.insert(key.clone(), (100, 200)).await;
            cache.range_cache.run_pending_tasks().await;
            assert!(cache.range_cache.get(&key).await.is_some());

            cache.clear();
            cache.range_cache.run_pending_tasks().await;
            assert!(cache.range_cache.get(&key).await.is_none());
        });
    }
}
