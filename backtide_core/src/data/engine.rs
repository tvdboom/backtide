//! Implementation of data related methods for [`Engine`].

use crate::config::models::triangulation_strategy::TriangulationStrategy;
use crate::constants::Symbol;
use crate::data::errors::{DataError, DataResult};
use crate::data::models::asset::Asset;
use crate::data::models::asset_meta::AssetMeta;
use crate::data::models::asset_type::AssetType;
use crate::data::models::currency::Currency;
use crate::data::models::download_info::DownloadInfo;
use crate::data::models::download_result::DownloadResult;
use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;
use crate::engine::Engine;
use crate::errors::EngineResult;
use crate::storage::models::bars_group::BarsGroup;
use futures::future::{join_all, try_join_all};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

impl Engine {
    // ────────────────────────────────────────────────────────────────────────
    // Public interface
    // ────────────────────────────────────────────────────────────────────────

    /// Fetch assets concurrently, using the cache where possible.
    #[instrument(skip(self), fields(?asset_type))]
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

    /// Resolves all assets required to price the given symbols in the
    /// portfolio base currency, including any triangulation intermediaries.
    #[instrument(skip(self), fields(?asset_type, ?intervals))]
    pub fn get_download_info(
        &self,
        symbols: Vec<Symbol>,
        asset_type: AssetType,
        intervals: Vec<Interval>,
    ) -> DataResult<DownloadInfo> {
        let base_currency = &self.config.general.base_currency.to_string();

        let tri_strategy = self.config.general.triangulation_strategy;
        let tri_fiat = &self.config.general.triangulation_fiat.to_string();
        let tri_crypto = &self.config.general.triangulation_crypto;
        let tri_crypto_pegged = &self.config.general.triangulation_crypto_pegged.to_string();

        self.rt.block_on(async {
            // Resolve the primary assets.
            let tasks: Vec<_> = symbols.iter().map(|s| self.load_asset(s, asset_type)).collect();
            let assets = join_all(tasks).await.into_iter().collect::<Result<Vec<_>, _>>()?;

            let mut leg_map: IndexMap<String, Asset> = IndexMap::new();
            let mut asset_leg_symbols: Vec<Vec<Symbol>> = Vec::new();

            for asset in &assets {
                let base = &asset.base;
                let quote = &asset.quote;

                // Skip if already denominated in base — no extra legs needed.
                if base.as_ref().is_some_and(|b| b == base_currency) || quote == base_currency {
                    asset_leg_symbols.push(vec![]);
                    continue;
                }

                let is_fiat = quote.parse::<Currency>().is_ok();
                let at = if is_fiat {
                    AssetType::Forex
                } else {
                    AssetType::Crypto
                };

                let (mid, mid_pegged) = if is_fiat {
                    (tri_fiat, tri_fiat)
                } else {
                    (tri_crypto, tri_crypto_pegged)
                };

                // Fetch the legs for this asset
                let resolved = self
                    .resolve_legs(
                        quote,
                        &base_currency,
                        mid,
                        mid_pegged,
                        at,
                        &intervals,
                        tri_strategy,
                    )
                    .await?;

                // Add the leg symbols to the asset's meta
                asset_leg_symbols.push(resolved.iter().map(|l| l.symbol.clone()).collect());

                for leg in resolved {
                    leg_map.entry(leg.symbol.clone()).or_insert(leg);
                }
            }

            let asset_metas = try_join_all(
                assets.into_iter().zip(asset_leg_symbols.into_iter()).map(|(asset, legs)| async {
                    let (earliest_ts, latest_ts) = self.load_range(&asset, &intervals).await?;
                    Ok::<_, DataError>(AssetMeta {
                        asset,
                        earliest_ts,
                        latest_ts,
                        legs,
                    })
                }),
            )
            .await?;

            let leg_metas = try_join_all(leg_map.into_values().map(|asset| async {
                let (earliest_ts, latest_ts) = self.load_range(&asset, &intervals).await?;
                Ok::<_, DataError>(AssetMeta {
                    asset,
                    earliest_ts,
                    latest_ts,
                    legs: vec![],
                })
            }))
            .await?;

            Ok(DownloadInfo {
                assets: asset_metas,
                legs: leg_metas,
            })
        })
    }

    /// List the most important assets for a given asset type, capped at `limit`.
    ///
    /// Delegates directly to the provider — callers should cache the result
    /// as this may trigger multiple network requests.
    #[instrument(skip(self), fields(?asset_type))]
    pub fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>> {
        self.rt.block_on(self.providers.get(&asset_type).unwrap().list_assets(asset_type, limit))
    }

    /// Download assets from a [`DownloadInfo`] and stores the results in the database.
    ///
    /// * Checks what is already in storage — skips completed ranges and only downloads
    ///   the missing head/tail (or both) for partial ones.
    /// * Downloads concurrently across symbols and intervals.
    /// * Writes **all** downloaded data in a single bulk transaction.
    /// * Only writes contiguous (gap-free) bars starting from the beginning of the range.
    /// * Idempotent, i.e., re-calling with the same `DownloadInfo` is a no-op.
    ///
    /// When `start` or `end` is provided, the per-asset range is clamped so that
    /// no data before `start` or after `end` is requested from the provider.
    #[instrument(skip(self))]
    pub fn download_symbols(
        &self,
        download_info: &DownloadInfo,
        start: Option<u64>,
        end: Option<u64>,
    ) -> EngineResult<DownloadResult> {
        // Collect all assets and legs pairs to treat them uniformly.
        let all_metas: Vec<&AssetMeta> =
            download_info.assets.iter().chain(download_info.legs.iter()).collect();

        self.rt.block_on(async {
            // Build a list of (symbol, asset_type, interval, start, end) tasks
            let mut tasks: Vec<(String, AssetType, Interval, u64, u64)> = Vec::new();

            for meta in &all_metas {
                let symbol = &meta.asset.symbol;
                let asset_type = meta.asset.asset_type;
                let provider = self.provider(asset_type);

                for (interval, meta_start) in &meta.earliest_ts {
                    let meta_end = meta.latest_ts.get(interval).unwrap();

                    // Clamp to the user-requested range when provided.
                    let start = start.map_or(*meta_start, |s| s.max(*meta_start));
                    let end = end.map_or(*meta_end, |e| e.min(*meta_end));

                    if start >= end {
                        debug!(%symbol, ?interval, "User range does not overlap provider range, skipping.");
                        continue;
                    }

                    // Check what's in storage and only download the missing portions.
                    if let Some((db_min, db_max)) =
                        self.get_stored_range(symbol, *interval, provider)?
                    {
                        if db_min <= start && db_max >= end {
                            debug!(%symbol, ?interval, "Already in database, skipping download.");
                            continue;
                        }

                        // Missing head: requested start is before what the database has.
                        if start < db_min {
                            debug!(%symbol, ?interval, head_end = db_min, "Downloading missing head.");
                            tasks.push((symbol.clone(), asset_type, *interval, start, db_min));
                        }

                        // Missing tail: requested end is beyond what the database has.
                        // Always start from db_max (not from `start`) so the new data connects
                        // to what is already stored, no gap is left even when the requested
                        // range begins after the stored range.
                        if end > db_max {
                            debug!(%symbol, ?interval, tail_start = db_max, "Downloading missing tail.");
                            tasks.push((symbol.clone(), asset_type, *interval, db_max, end));
                        }
                    } else {
                        tasks.push((symbol.clone(), asset_type, *interval, start, end));
                    };
                }
            }

            let total_tasks = tasks.len();
            info!("Download plan: {total_tasks} symbol x interval tasks");

            // ── Phase 1: download all tasks concurrently ─────────────────
            let downloaded: Vec<_> = join_all(tasks.into_iter().enumerate().map(
                |(idx, (symbol, at, interval, start, end))| async move {
                    let provider = self.providers.get(&at).unwrap();
                    info!(%symbol, ?interval, start, end, "Downloading...");
                    let result = provider.download_batch(&symbol, at, interval, start, end).await;
                    (idx, symbol, at, interval, result)
                },
            ))
            .await;

            // ── Phase 2: collect results and build one bulk write ────────
            // Separate successes from failures so we can write all successes
            // in a single transaction and still report individual errors.
            let mut groups: Vec<BarsGroup> = Vec::new();
            let mut outcomes: Vec<(usize, String, Interval, Result<usize, String>)> = Vec::new();

            for (idx, symbol, at, interval, result) in downloaded {
                let provider_enum = self.provider(at);
                match result {
                    Ok(bars) => {
                        let n_bars = bars.len();
                        info!(%symbol, ?interval, bars = n_bars, "Downloaded.");
                        groups.push(BarsGroup {
                            symbol: symbol.clone(),
                            asset_type: at,
                            interval,
                            provider: provider_enum,
                            bars,
                        });
                        outcomes.push((idx, symbol, interval, Ok(n_bars)));
                    },
                    Err(e) => {
                        warn!(%symbol, ?interval, "Download failed: {e}");
                        outcomes.push((idx, symbol, interval, Err(e.to_string())));
                    },
                }
            }

            // Single bulk write — one BEGIN / COMMIT for every downloaded group.
            if !groups.is_empty() {
                info!(groups = groups.len(), "Writing all downloaded data to database...");
                self.write_bars_bulk(&groups)?;
                info!("Bulk write complete.");
            }

            // ── Phase 3: build result summary ────────────────────────────
            let mut n_succeeded = 0usize;
            let mut warnings = Vec::new();

            for (_idx, symbol, interval, outcome) in outcomes {
                match outcome {
                    Ok(_) => n_succeeded += 1,
                    Err(msg) => warnings.push(format!("{symbol} ({interval}): {msg}")),
                }
            }

            let n_failed = warnings.len();
            Ok(DownloadResult {
                n_succeeded,
                n_failed,
                warnings,
            })
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private interface
    // ────────────────────────────────────────────────────────────────────────

    /// Resolve the [`Provider`] for a given asset type from config.
    fn provider(&self, asset_type: AssetType) -> Provider {
        *self.config.data.providers.get(&asset_type).unwrap()
    }

    /// Resolve an asset using the engine's cache.
    async fn load_asset(&self, symbol: &Symbol, asset_type: AssetType) -> DataResult<Asset> {
        if let Some(asset) = self.cache.asset_cache.get(symbol).await {
            debug!(%symbol, "Asset cache hit.");
            return Ok(asset.as_ref().clone());
        }

        let provider = self.providers.get(&asset_type).unwrap();
        let asset = provider.get_asset(symbol, asset_type).await?;
        self.cache.asset_cache.insert(symbol.clone(), Arc::new(asset.clone())).await;
        debug!(%symbol, "Asset cached");
        Ok(asset)
    }

    /// Resolve an asset range for one or multiple intervals using the engine's cache.
    async fn load_range(
        &self,
        asset: &Asset,
        intervals: &[Interval],
    ) -> DataResult<(HashMap<Interval, u64>, HashMap<Interval, u64>)> {
        let provider = self.providers.get(&asset.asset_type).unwrap();

        let ranges = try_join_all(intervals.iter().map(|&iv| async move {
            let key = (asset.symbol.clone(), iv);

            if let Some(range) = self.cache.range_cache.get(&key).await {
                debug!(symbol = %asset.symbol, ?iv, "Range cache hit.");
                return Ok::<_, DataError>((iv, range.0, range.1));
            }

            let (start, end) = provider.get_download_range(asset.clone(), iv).await?;
            self.cache.range_cache.insert(key, (start, end)).await;
            Ok::<_, DataError>((iv, start, end))
        }))
        .await?;

        let mut earliest = HashMap::new();
        let mut latest = HashMap::new();
        for (iv, start, end) in ranges {
            earliest.insert(iv, start);
            latest.insert(iv, end);
            debug!(symbol = %asset.symbol, ?iv, "Range cached.");
        }

        Ok((earliest, latest))
    }

    /// Try to load an asset from symbol format base-quote or quote-base.
    ///
    /// If both symbols exist, return the one with the longest history.
    async fn load_asset_bidirectional(
        &self,
        base: &str,
        quote: &str,
        at: AssetType,
        intervals: &[Interval],
    ) -> DataResult<Asset> {
        let base_quote = format!("{base}-{quote}");
        let quote_base = format!("{quote}-{base}");

        let (direct, inverse) =
            tokio::join!(self.load_asset(&base_quote, at), self.load_asset(&quote_base, at),);

        match (direct, inverse) {
            (Ok(d), Ok(i)) => {
                let d_start =
                    self.load_range(&d, intervals).await?.0.into_values().min().unwrap_or(u64::MAX);
                let i_start =
                    self.load_range(&i, intervals).await?.0.into_values().min().unwrap_or(u64::MAX);
                Ok(if d_start <= i_start {
                    d
                } else {
                    i
                })
            },
            (Ok(d), Err(_)) => Ok(d),
            (Err(_), Ok(i)) => Ok(i),
            (Err(e), Err(_)) => Err(e),
        }
    }

    /// Resolve a two-leg triangulation path: `quote → mid` and `mid_pegged → base`.
    ///
    /// Legs that are identical to their target currency are omitted.
    async fn triangulate(
        &self,
        quote: &str,
        mid: &str,
        mid_pegged: &str,
        base: &str,
        at: AssetType,
        intervals: &[Interval],
    ) -> DataResult<Vec<Asset>> {
        let mut legs = Vec::new();

        if quote != mid {
            legs.push(self.load_asset_bidirectional(quote, mid, at, intervals).await?);
        }

        if mid_pegged != base {
            legs.push(self.load_asset_bidirectional(mid_pegged, base, at, intervals).await?);
        }

        if legs.is_empty() {
            return Err(DataError::NoConversionPath {
                from: quote.to_string(),
                to: base.to_string(),
            });
        }

        Ok(legs)
    }

    /// Resolve the conversion legs needed to bring `quote` to `base`.
    async fn resolve_legs(
        &self,
        quote: &str,
        base: &str,
        mid: &str,
        mid_pegged: &str,
        at: AssetType,
        intervals: &[Interval],
        strategy: TriangulationStrategy,
    ) -> DataResult<Vec<Asset>> {
        let direct = self.load_asset_bidirectional(quote, base, at, intervals).await;

        match strategy {
            TriangulationStrategy::Direct => match direct {
                Ok(leg) => Ok(vec![leg]),
                Err(_) => self.triangulate(quote, mid, mid_pegged, base, at, intervals).await,
            },
            TriangulationStrategy::Earliest => {
                let tri = self.triangulate(quote, mid, mid_pegged, base, at, intervals).await;
                match (direct, tri) {
                    (Ok(d), Ok(t)) => {
                        // Check the history of all legs
                        let d_start = self
                            .load_range(&d, intervals)
                            .await?
                            .0
                            .into_values()
                            .min()
                            .unwrap_or(u64::MAX);

                        let t_start = try_join_all(t.iter().map(|l| self.load_range(l, intervals)))
                            .await?
                            .into_iter()
                            .flat_map(|(e, _)| e.into_values())
                            .max()
                            .unwrap_or(u64::MAX);

                        Ok(if d_start <= t_start {
                            vec![d]
                        } else {
                            t
                        })
                    },
                    (Ok(d), Err(_)) => Ok(vec![d]),
                    (Err(_), Ok(t)) => Ok(t),
                    (Err(e), Err(_)) => Err(e),
                }
            },
        }
    }
}
