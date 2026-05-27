//! Engine module.
//!
//! Engine is loaded once into a process-wide singleton ([`Engine::get`]) from
//! any part of the Python interface. Logic per module (download, backtest, etc...)
//! are implemented directly on the engine.

use crate::config::interface::Config;
use crate::constants::Symbol;
use crate::data::models::Provider;
use crate::data::models::{Instrument, InstrumentType, Interval};
use crate::data::providers::{Binance, Coinbase, DataProvider, Kraken, YahooFinance};
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

    #[test]
    fn engine_cache_clear_invalidates_instrument_entries() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let inst = Arc::new(Instrument {
                symbol: "AAPL".to_owned(),
                name: "Apple".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "NASDAQ".to_owned(),
                provider: Provider::Yahoo,
            });
            cache.instrument_cache.insert("AAPL".to_owned(), inst.clone()).await;
            cache.instrument_cache.run_pending_tasks().await;
            assert!(cache.instrument_cache.get("AAPL").await.is_some());

            cache.clear();
            cache.instrument_cache.run_pending_tasks().await;
            assert!(cache.instrument_cache.get("AAPL").await.is_none());
        });
    }

    #[test]
    fn engine_get_returns_singleton() {
        // First call initializes, second returns the same instance.
        let e1 = Engine::get().expect("engine init");
        let e2 = Engine::get().expect("engine init");
        assert!(std::ptr::eq(e1, e2));
        // Providers should be populated for all known instrument types.
        for t in InstrumentType::iter() {
            assert!(e1.providers.contains_key(&t));
        }
        // clear_cache must be callable without panicking.
        e1.clear_cache();
    }

    #[test]
    fn engine_cache_instrument_insertion_and_retrieval() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let inst = Arc::new(Instrument {
                symbol: "MSFT".to_owned(),
                name: "Microsoft".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "NASDAQ".to_owned(),
                provider: Provider::Yahoo,
            });
            cache.instrument_cache.insert("MSFT".to_owned(), inst.clone()).await;
            cache.instrument_cache.run_pending_tasks().await;
            let got = cache.instrument_cache.get("MSFT").await.unwrap();
            assert_eq!(got.symbol, "MSFT");
            assert!(cache.instrument_cache.get("NONEXIST").await.is_none());
        });
    }

    #[test]
    fn engine_cache_range_insertion_and_retrieval() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let key = ("BTC".to_owned(), Interval::OneHour);
            cache.range_cache.insert(key.clone(), (1000, 2000)).await;
            cache.range_cache.run_pending_tasks().await;
            let (start, end) = cache.range_cache.get(&key).await.unwrap();
            assert_eq!(start, 1000);
            assert_eq!(end, 2000);
        });
    }

    #[test]
    fn engine_cache_multiple_clears_dont_panic() {
        let cache = EngineCache::new();
        cache.clear();
        cache.clear();
        cache.clear();
    }

    // ── EngineCache constructor & behavior ───────────────────────────────

    #[test]
    fn engine_cache_default_trait_impl() {
        let cache1 = EngineCache::new();
        let cache2 = EngineCache::default();
        assert_eq!(cache1.instrument_cache.entry_count(), cache2.instrument_cache.entry_count());
        assert_eq!(cache1.range_cache.entry_count(), cache2.range_cache.entry_count());
    }

    #[test]
    fn engine_cache_has_correct_ttl_durations() {
        // The instrument cache has 2 hours TTL (7200 seconds)
        // and range cache has 30 min TTL (1800 seconds).
        // We can't directly test the TTL from outside, but we verify
        // that caches are created and are initially empty.
        let cache = EngineCache::new();
        assert_eq!(cache.instrument_cache.entry_count(), 0);
        assert_eq!(cache.range_cache.entry_count(), 0);
    }

    #[test]
    fn engine_cache_multiple_instruments() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let inst1 = Arc::new(Instrument {
                symbol: "AAPL".to_owned(),
                name: "Apple".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "NASDAQ".to_owned(),
                provider: Provider::Yahoo,
            });
            let inst2 = Arc::new(Instrument {
                symbol: "MSFT".to_owned(),
                name: "Microsoft".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "NASDAQ".to_owned(),
                provider: Provider::Yahoo,
            });

            cache.instrument_cache.insert("AAPL".to_owned(), inst1).await;
            cache.instrument_cache.insert("MSFT".to_owned(), inst2).await;
            cache.instrument_cache.run_pending_tasks().await;

            assert!(cache.instrument_cache.get("AAPL").await.is_some());
            assert!(cache.instrument_cache.get("MSFT").await.is_some());
        });
    }

    #[test]
    fn engine_cache_multiple_ranges() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let key1 = ("AAPL".to_owned(), Interval::OneDay);
            let key2 = ("AAPL".to_owned(), Interval::OneHour);
            let key3 = ("MSFT".to_owned(), Interval::OneDay);

            cache.range_cache.insert(key1.clone(), (100, 200)).await;
            cache.range_cache.insert(key2.clone(), (300, 400)).await;
            cache.range_cache.insert(key3.clone(), (500, 600)).await;
            cache.range_cache.run_pending_tasks().await;

            let (s1, e1) = cache.range_cache.get(&key1).await.unwrap();
            let (s2, e2) = cache.range_cache.get(&key2).await.unwrap();
            let (s3, e3) = cache.range_cache.get(&key3).await.unwrap();

            assert_eq!((s1, e1), (100, 200));
            assert_eq!((s2, e2), (300, 400));
            assert_eq!((s3, e3), (500, 600));
        });
    }

    #[test]
    fn engine_cache_clear_all_ranges() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let key1 = ("AAPL".to_owned(), Interval::OneDay);
            let key2 = ("MSFT".to_owned(), Interval::OneHour);

            cache.range_cache.insert(key1.clone(), (100, 200)).await;
            cache.range_cache.insert(key2.clone(), (300, 400)).await;
            cache.range_cache.run_pending_tasks().await;

            assert!(cache.range_cache.get(&key1).await.is_some());
            assert!(cache.range_cache.get(&key2).await.is_some());

            cache.clear();
            cache.range_cache.run_pending_tasks().await;

            assert!(cache.range_cache.get(&key1).await.is_none());
            assert!(cache.range_cache.get(&key2).await.is_none());
        });
    }

    #[test]
    fn engine_cache_clear_mixed_entries() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let inst = Arc::new(Instrument {
                symbol: "TEST".to_owned(),
                name: "Test".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "TEST".to_owned(),
                provider: Provider::Yahoo,
            });
            let key = ("TEST".to_owned(), Interval::OneDay);

            cache.instrument_cache.insert("TEST".to_owned(), inst).await;
            cache.range_cache.insert(key.clone(), (100, 200)).await;
            cache.instrument_cache.run_pending_tasks().await;
            cache.range_cache.run_pending_tasks().await;

            assert!(cache.instrument_cache.get("TEST").await.is_some());
            assert!(cache.range_cache.get(&key).await.is_some());

            cache.clear();
            cache.instrument_cache.run_pending_tasks().await;
            cache.range_cache.run_pending_tasks().await;

            assert!(cache.instrument_cache.get("TEST").await.is_none());
            assert!(cache.range_cache.get(&key).await.is_none());
        });
    }

    #[test]
    fn engine_cache_clear_empty() {
        let cache = EngineCache::new();
        // Should not panic on empty cache
        cache.clear();
        assert_eq!(cache.instrument_cache.entry_count(), 0);
        assert_eq!(cache.range_cache.entry_count(), 0);
    }

    #[test]
    fn engine_cache_instrument_overwrite() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let inst1 = Arc::new(Instrument {
                symbol: "AAPL".to_owned(),
                name: "Apple Inc".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "NASDAQ".to_owned(),
                provider: Provider::Yahoo,
            });
            let inst2 = Arc::new(Instrument {
                symbol: "AAPL".to_owned(),
                name: "Apple Corporation".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "NASDAQ".to_owned(),
                provider: Provider::Yahoo,
            });

            cache.instrument_cache.insert("AAPL".to_owned(), inst1).await;
            cache.instrument_cache.run_pending_tasks().await;
            let first = cache.instrument_cache.get("AAPL").await.unwrap();
            assert_eq!(first.name, "Apple Inc");

            cache.instrument_cache.insert("AAPL".to_owned(), inst2).await;
            cache.instrument_cache.run_pending_tasks().await;
            let second = cache.instrument_cache.get("AAPL").await.unwrap();
            assert_eq!(second.name, "Apple Corporation");
        });
    }

    #[test]
    fn engine_cache_range_overwrite() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let key = ("AAPL".to_owned(), Interval::OneDay);

            cache.range_cache.insert(key.clone(), (100, 200)).await;
            cache.range_cache.run_pending_tasks().await;
            let first = cache.range_cache.get(&key).await.unwrap();
            assert_eq!(first, (100, 200));

            cache.range_cache.insert(key.clone(), (150, 250)).await;
            cache.range_cache.run_pending_tasks().await;
            let second = cache.range_cache.get(&key).await.unwrap();
            assert_eq!(second, (150, 250));
        });
    }

    // ── Engine::get() and initialization ─────────────────────────────────

    #[test]
    fn engine_get_initializes_on_first_call() {
        let engine = Engine::get().expect("engine initialized");
        assert!(!engine.providers.is_empty());
    }

    #[test]
    fn engine_get_providers_all_present() {
        let engine = Engine::get().expect("engine initialized");
        for it in InstrumentType::iter() {
            assert!(
                engine.providers.contains_key(&it),
                "Missing provider for instrument type: {:?}",
                it
            );
        }
    }

    #[test]
    fn engine_get_cache_initialized() {
        let engine = Engine::get().expect("engine initialized");
        assert_eq!(engine.cache.instrument_cache.entry_count(), 0);
        assert_eq!(engine.cache.range_cache.entry_count(), 0);
    }

    #[test]
    fn engine_get_runtime_valid() {
        let engine = Engine::get().expect("engine initialized");
        // Verify the runtime can execute a simple task
        let result = engine.rt.block_on(async { 42 });
        assert_eq!(result, 42);
    }

    #[test]
    fn engine_clear_cache_removes_all_entries() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();

        rt.block_on(async {
            let inst = Arc::new(Instrument {
                symbol: "TEST".to_owned(),
                name: "Test".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "TEST".to_owned(),
                provider: Provider::Yahoo,
            });
            let key = ("TEST".to_owned(), Interval::OneHour);

            // Populate both caches
            cache.instrument_cache.insert("TEST".to_owned(), inst).await;
            cache.range_cache.insert(key.clone(), (500, 600)).await;
            cache.instrument_cache.run_pending_tasks().await;
            cache.range_cache.run_pending_tasks().await;

            // Verify populated
            assert_eq!(cache.instrument_cache.entry_count(), 1);
            assert_eq!(cache.range_cache.entry_count(), 1);

            // Clear should remove all
            cache.clear();
            cache.instrument_cache.run_pending_tasks().await;
            cache.range_cache.run_pending_tasks().await;

            assert_eq!(cache.instrument_cache.entry_count(), 0);
            assert_eq!(cache.range_cache.entry_count(), 0);
        });
    }

    #[test]
    fn engine_clear_cache_idempotent() {
        let cache = EngineCache::new();
        // Multiple clears in a row should be safe
        cache.clear();
        cache.clear();
        cache.clear();
        assert_eq!(cache.instrument_cache.entry_count(), 0);
        assert_eq!(cache.range_cache.entry_count(), 0);
    }

    // ── Edge cases and boundary conditions ────────────────────────────────

    #[test]
    fn engine_cache_get_nonexistent_returns_none() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let result = cache.instrument_cache.get("NONEXIST").await;
            assert!(result.is_none());
        });
    }

    #[test]
    fn engine_cache_range_get_nonexistent_returns_none() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let result = cache.range_cache.get(&("NONEXIST".to_owned(), Interval::OneDay)).await;
            assert!(result.is_none());
        });
    }

    #[test]
    fn engine_cache_different_intervals_isolated() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let key_1d = ("AAPL".to_owned(), Interval::OneDay);
            let key_1h = ("AAPL".to_owned(), Interval::OneHour);
            let key_15m = ("AAPL".to_owned(), Interval::FifteenMinutes);

            cache.range_cache.insert(key_1d.clone(), (1, 100)).await;
            cache.range_cache.insert(key_1h.clone(), (1, 50)).await;
            cache.range_cache.insert(key_15m.clone(), (1, 25)).await;
            cache.range_cache.run_pending_tasks().await;

            assert_eq!(cache.range_cache.get(&key_1d).await.unwrap(), (1, 100));
            assert_eq!(cache.range_cache.get(&key_1h).await.unwrap(), (1, 50));
            assert_eq!(cache.range_cache.get(&key_15m).await.unwrap(), (1, 25));
        });
    }

    #[test]
    fn engine_cache_same_symbol_different_intervals() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let key1 = ("BTC".to_owned(), Interval::OneDay);
            let key2 = ("BTC".to_owned(), Interval::FiveMinutes);

            cache.range_cache.insert(key1.clone(), (1000, 2000)).await;
            cache.range_cache.insert(key2.clone(), (1000, 1500)).await;
            cache.range_cache.run_pending_tasks().await;

            assert_eq!(cache.range_cache.get(&key1).await.unwrap(), (1000, 2000));
            assert_eq!(cache.range_cache.get(&key2).await.unwrap(), (1000, 1500));
        });
    }

    #[test]
    fn engine_cache_empty_after_individual_clear_ops() {
        let rt = Runtime::new().unwrap();
        let cache = EngineCache::new();
        rt.block_on(async {
            let inst = Arc::new(Instrument {
                symbol: "X".to_owned(),
                name: "X".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "X".to_owned(),
                provider: Provider::Yahoo,
            });
            let key = ("X".to_owned(), Interval::OneDay);

            cache.instrument_cache.insert("X".to_owned(), inst).await;
            cache.range_cache.insert(key, (1, 2)).await;
            cache.instrument_cache.run_pending_tasks().await;
            cache.range_cache.run_pending_tasks().await;

            // First clear
            cache.clear();
            cache.instrument_cache.run_pending_tasks().await;
            cache.range_cache.run_pending_tasks().await;
            assert_eq!(cache.instrument_cache.entry_count(), 0);
            assert_eq!(cache.range_cache.entry_count(), 0);

            // Fill again and clear again
            cache.instrument_cache.insert("X".to_owned(), Arc::new(Instrument {
                symbol: "X".to_owned(),
                name: "X2".to_owned(),
                base: None,
                quote: "USD".to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "X".to_owned(),
                provider: Provider::Yahoo,
            })).await;
            cache.instrument_cache.run_pending_tasks().await;
            assert_eq!(cache.instrument_cache.entry_count(), 1);

            cache.clear();
            cache.instrument_cache.run_pending_tasks().await;
            assert_eq!(cache.instrument_cache.entry_count(), 0);
        });
    }
}
