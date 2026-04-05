//! Binance data provider.
//!
//! No authentication is required for public market data endpoints.

use crate::constants::Symbol;
use crate::data::errors::{DataError, DataResult};
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::interval::Interval;
use crate::data::providers::traits::DataProvider;
use crate::data::utils::canonical_symbol;
use crate::utils::http::{HttpClient, HttpError};
use async_trait::async_trait;
use serde::Deserialize;
use tracing::{debug, info, instrument};

/// Binance spot-market data provider.
///
/// Wraps Binance's public REST API behind the [`DataProvider`] trait.
/// Only [`AssetType::Crypto`] is supported; all other asset types return
/// [`DataError::UnsupportedAssetType`].
pub struct Binance {
    /// Shared async HTTP client.
    client: HttpClient,
}

impl Binance {
    /// Returns exchange metadata for trading pairs.
    const EXCHANGE_INFO_URL: &str = "https://api.binance.com/api/v3/exchangeInfo";

    const KLINES_URL: &str = "https://api.binance.com/api/v3/klines";

    // ────────────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────────────

    /// Create a new [`Binance`] provider.
    pub async fn new() -> DataResult<Self> {
        let client = HttpClient::new()?;

        info!("Binance provider initialised");
        Ok(Self {
            client,
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private API
    // ────────────────────────────────────────────────────────────────────────

    /// Convert a canonical symbol to Binance format.
    fn parse_canonical_symbol(symbol: &str) -> String {
        symbol.replace('-', "")
    }

    /// Guard: return [`DataError::UnsupportedAssetType`] for anything except
    /// [`AssetType::Crypto`].
    fn require_crypto(asset_type: AssetType) -> DataResult<()> {
        if asset_type == AssetType::Crypto {
            Ok(())
        } else {
            Err(DataError::UnsupportedAssetType(asset_type))
        }
    }

    /// Fetch exchange info for a single symbol.
    async fn fetch_symbol_info(&self, symbol: &str) -> DataResult<SymbolInfo> {
        let resp = self
            .client
            .get(Self::EXCHANGE_INFO_URL, Some(&[("symbol", symbol)]))
            .await
            .map_err(|_| DataError::SymbolNotFound(symbol.to_owned()))?;

        let parsed = HttpClient::json::<ExchangeInfoResponse>(resp).await?;

        parsed
            .symbols
            .into_iter()
            .next()
            .ok_or_else(|| DataError::SymbolNotFound(symbol.to_owned()))
    }

    /// Fetch klines for a symbol between Unix timestamps `start_time` and `end_time`.
    #[instrument(skip(self), fields(%symbol, limit))]
    async fn get_bars(
        &self,
        symbol: &str,
        interval: Interval,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: usize,
    ) -> DataResult<Vec<BinanceKline>> {
        // Build query params dynamically so absent bounds are simply omitted.
        let mut params: Vec<(&str, String)> = vec![
            ("symbol", symbol.to_owned()),
            ("interval", interval.to_string()),
            ("limit", limit.to_string()),
        ];

        if let Some(s) = start_time {
            params.push(("startTime", (s * 1_000).to_string()));
        }
        if let Some(e) = end_time {
            params.push(("endTime", (e * 1_000).to_string()));
        }

        let params_ref: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let resp = self.client.get(Self::KLINES_URL, Some(&params_ref)).await?;
        let rows = HttpClient::json::<Vec<serde_json::Value>>(resp).await?;

        if rows.is_empty() {
            return Err(HttpError::UnexpectedPayload(format!(
                "empty response for symbol: {}",
                symbol.to_owned()
            )))?;
        }

        let bars: Vec<BinanceKline> =
            rows.into_iter().map(|row| BinanceKline::try_from(row)).collect::<DataResult<_>>()?;

        Ok(bars)
    }
}

#[async_trait]
impl DataProvider for Binance {
    /// Fetch metadata for a single symbol.
    #[instrument(skip(self), fields(%symbol))]
    async fn get_asset(&self, symbol: &Symbol, asset_type: AssetType) -> DataResult<Asset> {
        Self::require_crypto(asset_type)?;

        let symbol = Self::parse_canonical_symbol(symbol);
        let info = self.fetch_symbol_info(&symbol).await?;

        Ok(Asset::try_from(info)?)
    }

    /// Returns the usable download range for an asset at a given interval.
    #[instrument(skip(self), fields(symbol = %asset.symbol, ?interval))]
    async fn get_download_range(&self, asset: Asset, interval: Interval) -> DataResult<(u64, u64)> {
        Self::require_crypto(asset.asset_type)?;

        let symbol = Self::parse_canonical_symbol(&asset.symbol);

        let (first, last) = tokio::try_join!(
            self.get_bars(&symbol, interval, Some(0), None, 1),
            self.get_bars(&symbol, interval, None, None, 1),
        )?;

        let earliest_ts = first
            .into_iter()
            .next()
            .ok_or_else(|| DataError::SymbolNotFound(symbol.clone()))?
            .open_time;

        let latest_ts = last
            .into_iter()
            .next()
            .ok_or_else(|| DataError::SymbolNotFound(symbol.clone()))?
            .close_time;

        Ok((earliest_ts, latest_ts))
    }

    /// List the spot crypto assets traded on Binance, capped at `limit`.
    #[instrument(skip(self), fields(?asset_type, limit))]
    async fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>> {
        Self::require_crypto(asset_type)?;

        let resp =
            self.client.get(Self::EXCHANGE_INFO_URL, Some(&[("permissions", "SPOT")])).await?;
        let info = HttpClient::json::<ExchangeInfoResponse>(resp).await?;

        let assets: Vec<Asset> = info
            .symbols
            .into_iter()
            .filter(|s| s.status == "TRADING")
            .filter_map(|info| {
                Asset::try_from(info)
                    .map_err(|e| {
                        debug!("Binance list_assets error: {e}");
                        e
                    })
                    .ok()
            })
            .take(limit)
            .collect();

        Ok(assets)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Binance API objects
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ExchangeInfoResponse {
    symbols: Vec<SymbolInfo>,
}

/// One trading pair entry from `/api/v3/exchangeInfo`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymbolInfo {
    /// Binance composite symbol (e.g. `"BTCUSDT"`).
    symbol: String,

    /// Lifecycle state — only `"TRADING"` pairs are usable.
    status: String,

    /// Base asset ticker (e.g. `"BTC"`).
    base_asset: String,

    /// Quote asset ticker (e.g. `"USDT"`).
    quote_asset: String,
}

impl TryFrom<SymbolInfo> for Asset {
    type Error = DataError;

    fn try_from(info: SymbolInfo) -> DataResult<Self> {
        let base = info.base_asset;
        let quote = info.quote_asset;

        Ok(Asset {
            symbol: canonical_symbol(&info.symbol, &Some(base.clone()), &quote),
            name: format!("{base}-{quote}"),
            base: Some(base),
            quote,
            asset_type: AssetType::Crypto,
            exchange: "BINANCE".to_owned(), // Binance has no MIC code.
            volume: None,
            price: None,
        })
    }
}

/// One row from `/api/v3/klines`.
#[derive(Debug, Copy, Clone)]
struct BinanceKline {
    /// Bar open time in Unix milliseconds.
    open_time: u64,

    /// Bar close price.
    close: f64,

    /// Bar close time in Unix milliseconds.
    close_time: u64,
}

impl TryFrom<serde_json::Value> for BinanceKline {
    type Error = DataError;

    fn try_from(row: serde_json::Value) -> DataResult<Self> {
        let arr = row
            .as_array()
            .ok_or_else(|| DataError::UnexpectedResponse("kline row is not an array".to_owned()))?;

        let open_time =
            arr.first().and_then(|v| v.as_i64()).map(|ms| (ms / 1_000).max(0) as u64).ok_or_else(
                || DataError::UnexpectedResponse("missing kline open_time".to_owned()),
            )?;

        let close = arr
            .get(4)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing kline close".to_owned()))?;

        let close_time =
            arr.get(6).and_then(|v| v.as_i64()).map(|ms| (ms / 1_000).max(0) as u64).ok_or_else(
                || DataError::UnexpectedResponse("missing kline close_time".to_owned()),
            )?;

        Ok(Self {
            open_time,
            close,
            close_time,
        })
    }
}
