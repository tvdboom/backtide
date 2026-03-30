//! Implementation of the [`DataDownload`].

use crate::config::Config;
use crate::data::errors::DataResult;
use crate::data::models::asset::{Asset, AssetType};
use crate::data::models::bar::Interval;
use crate::data::models::currency::Currency;
use crate::data::models::download::DownloadValidation;
use crate::data::provider::provider::Provider;
use crate::data::provider::traits::DataProvider;
use crate::data::provider::yahoo::YahooFinance;
use futures::future::join_all;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use strum::IntoEnumIterator;
use tokio::runtime::Runtime;

/// Process-wide data download singleton.
static DATA_DOWNLOAD: OnceLock<DataDownload> = OnceLock::new();

/// Singleton-like data download struct.
pub struct DataDownload {
    /// Mapping of each asset type to its provider.
    providers: HashMap<AssetType, Arc<dyn DataProvider>>,

    /// Tokio runtime.
    rt: Runtime,
}

impl DataDownload {
    // ────────────────────────────────────────────────────────────────────────
    // Public interface
    // ────────────────────────────────────────────────────────────────────────
    //
    // /// Validate a set of parameters for data download.
    // async fn validate_download(
    //     &self,
    //     asset_type: AssetType,
    //     symbols: Vec<String>,
    //     base_currency: &str,
    //     intervals: &[Interval],
    // ) -> DataResult<DownloadValidation> {
    //     let cfg = Config::get()?;
    //
    //     // Fetch metadata for all requested symbols concurrently
    //     let assets = self.get_assets(asset_type, symbols)?;
    //
    //     // Resolve forex dependencies
    //     let forex_deps = self.resolve_forex_dependencies(&assets, cfg.base_currency);
    //
    //     // Fetch metadata for forex symbols concurrently too
    //     let forex_infos: Vec<anyhow::Result<(AssetInfo, String)>> = stream::iter(&forex_deps)
    //         .map(|dep| {
    //             let sym = dep.symbol.clone();
    //             let req_by = dep.required_by.clone();
    //             async move {
    //                 self.get_asset_by_symbol(&sym)
    //                     .await
    //                     .map(|info| (info, req_by))
    //                     .with_context(|| format!("get_asset failed for forex {sym}"))
    //             }
    //         })
    //         .buffer_unordered(concurrency)
    //         .collect()
    //         .await;
    //
    //     // ── 3. Build AssetValidationInfo rows ─────────────────────────────────
    //     let mut all_rows: Vec<AssetValidationInfo> = Vec::new();
    //     let mut global_start: Option<NaiveDate> = None;
    //     let mut global_end: Option<NaiveDate> = None;
    //
    //     let update_range = |gs: &mut Option<NaiveDate>,
    //                         ge: &mut Option<NaiveDate>,
    //                         start: NaiveDate,
    //                         end: NaiveDate| {
    //         *gs = Some(gs.map_or(start, |g: NaiveDate| g.min(start)));
    //         *ge = Some(ge.map_or(end, |g: NaiveDate| g.max(end)));
    //     };
    //
    //     // Original assets
    //     for asset in &resolved_assets {
    //         let start = asset
    //             .first_trade_date
    //             .unwrap_or_else(|| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
    //         let end = asset.last_trade_date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    //         update_range(&mut global_start, &mut global_end, start, end);
    //
    //         let bars = estimate_bars(&asset.asset_type.to_string(), start, end, interval);
    //         let bytes = bars * BYTES_PER_BAR;
    //
    //         all_rows.push(AssetValidationInfo {
    //             symbol: asset.symbol.clone(),
    //             name: asset.name.clone(),
    //             asset_type: asset.asset_type.to_string(),
    //             exchange: asset.exchange.to_string(),
    //             currency: asset.currency.code().to_string(),
    //             earliest_date: start.to_string(),
    //             latest_date: end.to_string(),
    //             est_bars: bars,
    //             est_bytes: bytes,
    //             is_forex_dependency: false,
    //             required_by: String::new(),
    //         });
    //     }
    //
    //     // Forex dependencies
    //     for res in forex_infos {
    //         match res {
    //             Ok((asset, required_by)) => {
    //                 let start = asset
    //                     .first_trade_date
    //                     .unwrap_or_else(|| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
    //                 let end =
    //                     asset.last_trade_date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    //                 update_range(&mut global_start, &mut global_end, start, end);
    //
    //                 let bars = estimate_bars("Forex", start, end, interval);
    //                 let bytes = bars * BYTES_PER_BAR;
    //
    //                 all_rows.push(AssetValidationInfo {
    //                     symbol: asset.symbol.clone(),
    //                     name: asset.name.clone(),
    //                     asset_type: "Forex".to_string(),
    //                     exchange: "FX".to_string(),
    //                     currency: asset.currency.code().to_string(),
    //                     earliest_date: start.to_string(),
    //                     latest_date: end.to_string(),
    //                     est_bars: bars,
    //                     est_bytes: bytes,
    //                     is_forex_dependency: true,
    //                     required_by,
    //                 });
    //             },
    //             Err(e) => {
    //                 // Forex fetch failure is non-fatal — log and skip
    //                 tracing::warn!("Could not fetch forex asset metadata: {e:#}");
    //             },
    //         }
    //     }
    //
    //     // ── 4. Aggregate stats ────────────────────────────────────────────────
    //     let g_start = global_start.unwrap_or_else(|| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
    //     let g_end = global_end.unwrap_or_else(|| chrono::Utc::now().date_naive());
    //
    //     let total_bars: u64 = all_rows.iter().map(|r| r.est_bars).sum();
    //     let total_bytes: u64 = all_rows.iter().map(|r| r.est_bytes).sum();
    //     let months = calendar_months(g_start, g_end);
    //     let est_secs = estimate_download_secs(all_rows.len(), months, 3.0);
    //
    //     let original_count = resolved_assets.len();
    //     let forex_dependency_count = forex_deps.len();
    //
    //     Ok(DownloadValidation {
    //         assets: all_rows,
    //         original_count,
    //         forex_dependency_count,
    //         global_start: g_start.to_string(),
    //         global_end: g_end.to_string(),
    //         total_bars,
    //         total_bytes,
    //         est_seconds: est_secs,
    //         calendar_months: months,
    //     })
    // }

    /// Return a `&'static` reference to the global ingester.
    pub fn get() -> DataResult<&'static DataDownload> {
        // Replace block with get_or_try_init when it becomes stable
        if let Some(cfg) = DATA_DOWNLOAD.get() {
            Ok(cfg)
        } else {
            let _ = DATA_DOWNLOAD.set(DataDownload::init()?);
            Ok(DATA_DOWNLOAD.get().unwrap())
        }
    }

    /// Get a list of assets given their symbols.
    pub fn get_assets(
        &self,
        symbols: Vec<String>,
        asset_type: AssetType,
    ) -> DataResult<Vec<Asset>> {
        self.rt.block_on(async {
            let provider = self.providers.get(&asset_type).unwrap();
            let tasks: Vec<_> =
                symbols.iter().map(|symbol| provider.get_asset(symbol, asset_type)).collect();

            let results = join_all(tasks).await;

            // Collect to surface errors
            let assets = results.into_iter().collect::<Result<Vec<_>, _>>()?;

            Ok(assets)
        })
    }

    /// List available assets for a given asset type.
    pub fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>> {
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
    fn init() -> DataResult<Self> {
        let rt = Runtime::new()?;
        let pc = &Config::get()?.data.providers;

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
        })
    }

    // /// Given a list of assets, return which forex assets are required to
    // /// convert them to the base currency.
    // fn resolve_forex_dependencies(
    //     &self,
    //     assets: &Vec<Asset>,
    //     base_currency: Currency,
    // ) -> Vec<Asset> {
    //     let base = base_currency.to_string();
    //     let mut seen: HashMap<(String, String), Asset> = HashMap::new();
    //
    //     for asset in assets {
    //         let key = (asset.quote, base);
    //
    //         if asset.quote == base || seen.contains_key(&key) {
    //             continue;
    //         }
    //
    //         if asset.asset_type == AssetType::Crypto {
    //             // Cryptos are mapped to USDT, which is pegged to USD
    //             if asset.quote != "USDT" {
    //                 let key = seen.insert((asset.quote.clone(), "USDT".to_owned()));
    //             }
    //         }
    //
    //         // Check if currency -> base exists
    //         if let Ok() =
    //             self.get_assets(AssetType::Forex, vec![format!("{}{}", asset.quote, base)])
    //         {
    //             seen.insert(
    //                 key.clone(),
    //                 ForexDependency {
    //                     symbol: format!("{}{}=X", asset.quote, base),
    //                     base: asset.quote.to_string(),
    //                     quote: base.clone(),
    //                     required_by: asset.symbol.clone(),
    //                 },
    //             );
    //             continue;
    //         }
    //
    //         if base_currency == "USD" {
    //             // Direct: e.g. JPY/USD
    //             seen.insert(
    //                 key,
    //                 ForexDependency {
    //                     symbol: format!("{}USD=X", ccy),
    //                     base: ccy.to_string(),
    //                     quote: "USD".to_string(),
    //                     required_by: asset.symbol.clone(),
    //                 },
    //             );
    //         } else if ccy == "USD" {
    //             // Inverse: e.g. USD/GBP
    //             let k = ("USD".to_string(), base_currency.to_string());
    //             seen.entry(k).or_insert_with(|| ForexDependency {
    //                 symbol: format!("USD{}=X", base_currency),
    //                 base: "USD".to_string(),
    //                 quote: base_currency.to_string(),
    //                 required_by: asset.symbol.clone(),
    //             });
    //         } else {
    //             // Triangulate: ccy→USD, then USD→base
    //             let k1 = (ccy.to_string(), "USD".to_string());
    //             seen.entry(k1).or_insert_with(|| ForexDependency {
    //                 symbol: format!("{}USD=X", ccy),
    //                 base: ccy.to_string(),
    //                 quote: "USD".to_string(),
    //                 required_by: asset.symbol.clone(),
    //             });
    //             let k2 = ("USD".to_string(), base_currency.to_string());
    //             seen.entry(k2).or_insert_with(|| ForexDependency {
    //                 symbol: format!("USD{}=X", base_currency),
    //                 base: "USD".to_string(),
    //                 quote: base_currency.to_string(),
    //                 required_by: asset.symbol.clone(),
    //             });
    //         }
    //     }
    //
    //     seen.into_values().collect()
    // }
}
