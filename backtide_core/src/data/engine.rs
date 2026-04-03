//! Implementation of data related methods for [`Engine`].

use crate::constants::Symbol;
use crate::data::errors::DataResult;
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::currency::Currency;
use crate::engine::Engine;
use futures::future::{join_all, try_join_all};
use indexmap::IndexMap;
use std::sync::Arc;
use tracing::{debug, instrument};

impl Engine {
    // ────────────────────────────────────────────────────────────────────────
    // Public interface
    // ────────────────────────────────────────────────────────────────────────

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
        let base_currency = &self.config.general.base_currency.to_string();
        let tri_fiat = &self.config.general.triangulation_fiat.to_string();
        let tri_crypto = &self.config.general.triangulation_crypto;
        let tri_crypto_pegged = &self.config.general.triangulation_crypto_pegged.to_string();

        self.rt.block_on(async {
            // Resolve all primary assets concurrently.
            let assets: Vec<Asset> =
                try_join_all(symbols.iter().map(|sym| self.load_asset(sym, asset_type))).await?;

            // Compute which triangulation legs are needed.
            // Use IndexMap to preserve insertion order while deduplicating by symbol.
            let mut leg_symbols: IndexMap<String, (String, String, AssetType)> = IndexMap::new();

            for asset in &assets {
                let base = &asset.base;
                let quote = &asset.quote;
                let is_fiat = quote.parse::<Currency>().is_ok();

                // Skip if already denominated in base — no extra legs needed.
                if base.as_ref().is_some_and(|b| b == base_currency) || quote == base_currency {
                    continue;
                }

                let at = if is_fiat {
                    AssetType::Forex
                } else {
                    AssetType::Crypto
                };

                // Try direct conversion first.
                if self.load_asset_bidirectional(&quote, &base_currency, at).await.is_ok() {
                    leg_symbols
                        .entry(format!("{quote}-{base_currency}"))
                        .or_insert_with(|| (quote.clone(), base_currency.clone(), at));
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
                    insert_leg(&quote, mid1);
                }
                if mid2 != base_currency {
                    insert_leg(mid2, &base_currency);
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

    /// Resolve an asset, returning the cached value if still live.
    ///
    /// On a cache miss the provider is queried and the result is inserted
    /// before returning.
    async fn load_asset(&self, symbol: &Symbol, asset_type: AssetType) -> DataResult<Asset> {
        if let Some(asset) = self.asset_cache.get(symbol).await {
            debug!(%symbol, "Asset cache hit");
            return Ok(asset.as_ref().clone());
        }

        let provider = self.providers.get(&asset_type).unwrap();
        let asset = provider.get_asset(symbol, asset_type).await?;
        self.asset_cache.insert(symbol.clone(), Arc::new(asset.clone())).await;
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
