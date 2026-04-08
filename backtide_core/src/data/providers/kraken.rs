//! Kraken data provider.
//!
//! No authentication is required for public market data endpoints.

use crate::constants::Symbol;
use crate::data::errors::{DataError, DataResult};
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::bar::Bar;
use crate::data::models::interval::Interval;
use crate::data::providers::traits::DataProvider;
use crate::data::utils::canonical_symbol;
use crate::utils::http::{HttpClient, HttpError};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, info, instrument};

/// Kraken spot-market data provider.
///
/// Wraps Kraken's public REST API behind the [`DataProvider`] trait.
/// Only [`AssetType::Crypto`] is supported; all other asset types return
/// [`DataError::UnsupportedAssetType`].
pub struct Kraken {
    /// Shared async HTTP client.
    client: HttpClient,
}

impl Kraken {
    /// Returns trading pair metadata.
    const ASSET_PAIRS_URL: &str = "https://api.kraken.com/0/public/AssetPairs";

    /// Returns OHLC candlestick data.
    const OHLC_URL: &str = "https://api.kraken.com/0/public/OHLC";

    /// Mapping from-to kraken specific symbols to canonical symbols.
    const TICKER_MAPPINGS: &[(&str, &str)] = &[("BTC", "XBT"), ("DOGE", "XDG")];

    // ────────────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────────────

    /// Create a new [`Kraken`] provider.
    pub async fn new() -> DataResult<Self> {
        let client = HttpClient::new()?;

        info!("Kraken provider initialised");
        Ok(Self {
            client,
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private API
    // ────────────────────────────────────────────────────────────────────────

    /// Convert a canonical pair (e.g. `"BTC-USD"`) to Kraken format (e.g. `"XBTUSD"`).
    fn parse_canonical_symbol(&self, symbol: &str) -> String {
        symbol
            .split('-')
            .map(|part| {
                Self::TICKER_MAPPINGS
                    .iter()
                    .find(|(canonical, _)| *canonical == part)
                    .map(|(_, kraken)| *kraken)
                    .unwrap_or(part)
            })
            .collect()
    }

    /// Normalize Kraken-specific ticker to their canonical names.
    pub fn normalize_ticker(ticker: &str) -> String {
        Self::TICKER_MAPPINGS
            .iter()
            .find(|(_, kraken)| *kraken == ticker)
            .map(|(canonical, _)| (*canonical).to_string())
            .unwrap_or_else(|| ticker.to_string())
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

    /// Unwrap the standard Kraken response envelope, returning the `result`
    /// field or the first error string.
    fn unwrap_response<T>(resp: KrakenResponse<T>, symbol: &str) -> DataResult<T> {
        if !resp.error.is_empty() {
            if resp.error.iter().any(|e| e.contains("Unknown asset pair")) {
                return Err(DataError::SymbolNotFound(symbol.to_owned()));
            }
            return Err(DataError::UnexpectedResponse(resp.error.join("; ")));
        }

        resp.result.ok_or_else(|| {
            DataError::UnexpectedResponse("Kraken response has no result".to_owned())
        })
    }

    /// Fetch OHLC bars for a pair.
    ///
    /// `since` is an optional Unix-seconds timestamp. When `Some(0)` the
    /// exchange returns the earliest 720 candles.  When `None` it returns
    /// the most recent 720 candles.
    #[instrument(skip(self), fields(%symbol, %interval))]
    async fn get_bars(
        &self,
        symbol: &str,
        interval: Interval,
        since: Option<u64>,
    ) -> DataResult<Vec<KrakenOHLC>> {
        let mut params: Vec<(&str, String)> =
            vec![("pair", symbol.to_owned()), ("interval", interval.minutes().to_string())];

        if let Some(s) = since {
            params.push(("since", s.to_string()));
        }

        let params_ref: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let resp = self.client.get(Self::OHLC_URL, Some(&params_ref)).await?;
        let parsed = HttpClient::json::<KrakenResponse<serde_json::Value>>(resp).await?;
        let result = Self::unwrap_response(parsed, symbol)?;

        let obj = result.as_object().ok_or_else(|| {
            DataError::UnexpectedResponse("OHLC result is not an object".to_owned())
        })?;

        let rows =
            obj.iter().find(|(k, _)| *k != "last").and_then(|(_, v)| v.as_array()).ok_or_else(
                || DataError::UnexpectedResponse("no OHLC data array found".to_owned()),
            )?;

        if rows.is_empty() {
            return Err(HttpError::UnexpectedPayload(format!(
                "empty response for pair: {}",
                symbol
            )))?;
        }

        let bars: Vec<KrakenOHLC> =
            rows.iter().cloned().map(KrakenOHLC::try_from).collect::<DataResult<_>>()?;

        Ok(bars)
    }
}

#[async_trait]
impl DataProvider for Kraken {
    /// Fetch metadata for a single symbol.
    #[instrument(skip(self), fields(%symbol))]
    async fn get_asset(&self, symbol: &Symbol, asset_type: AssetType) -> DataResult<Asset> {
        Self::require_crypto(asset_type)?;

        let pair = self.parse_canonical_symbol(symbol);

        let resp = self
            .client
            .get(Self::ASSET_PAIRS_URL, Some(&[("pair", &pair)]))
            .await
            .map_err(|_| DataError::SymbolNotFound(symbol.to_owned()))?;

        let parsed = HttpClient::json::<KrakenResponse<HashMap<String, PairInfo>>>(resp).await?;
        let map = Self::unwrap_response(parsed, symbol)?;

        let info =
            map.into_values().next().ok_or_else(|| DataError::SymbolNotFound(symbol.to_owned()))?;

        Ok(Asset::try_from(info)?)
    }

    /// Returns the usable download range for an asset at a given interval.
    #[instrument(skip(self), fields(symbol = %asset.symbol, ?interval))]
    async fn get_download_range(&self, asset: Asset, interval: Interval) -> DataResult<(u64, u64)> {
        Self::require_crypto(asset.asset_type)?;

        let symbol = self.parse_canonical_symbol(&asset.symbol);

        let earliest_ts = self
            .get_bars(&symbol, interval, Some(0))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| DataError::SymbolNotFound(asset.symbol.clone()))?
            .time;

        let latest_ts = self
            .get_bars(&symbol, interval, None)
            .await?
            .into_iter()
            .last()
            .ok_or_else(|| DataError::SymbolNotFound(asset.symbol))?
            .time;

        Ok((earliest_ts, latest_ts))
    }

    /// List the spot crypto assets traded on Kraken, capped at `limit`.
    #[instrument(skip(self), fields(?asset_type, limit))]
    async fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>> {
        Self::require_crypto(asset_type)?;

        let resp = self.client.get(Self::ASSET_PAIRS_URL, None).await?;
        let parsed = HttpClient::json::<KrakenResponse<HashMap<String, PairInfo>>>(resp).await?;
        let map = Self::unwrap_response(parsed, "AssetPairs")?;

        let assets: Vec<Asset> = map
            .into_values()
            .filter(|p| p.status == "online")
            .filter_map(|info| {
                Asset::try_from(info)
                    .map_err(|e| {
                        debug!("Kraken list_assets error: {e}");
                        e
                    })
                    .ok()
            })
            .take(limit)
            .collect();

        Ok(assets)
    }

    /// Download OHLCV bars for `symbol` at `interval` from `start` to `end`.
    #[instrument(skip(self), fields(%symbol, ?interval, start, end))]
    async fn download_batch(
        &self,
        symbol: &str,
        _asset_type: AssetType,
        interval: Interval,
        start: u64,
        end: u64,
    ) -> DataResult<Vec<Bar>> {
        let kraken_symbol = self.parse_canonical_symbol(symbol);
        let interval_secs = interval.minutes() * 60;
        let mut all_bars: Vec<Bar> = Vec::new();
        let mut cursor = start;

        loop {
            let bars = self.get_bars(&kraken_symbol, interval, Some(cursor)).await;

            let bars = match bars {
                Ok(b) => b,
                Err(_) => break,
            };

            if bars.is_empty() {
                break;
            }

            let last_time = bars.last().unwrap().time;
            let mut added = false;

            for k in bars {
                let bar = Bar::from(k);
                if bar.open_ts >= start && bar.open_ts < end {
                    all_bars.push(bar);
                    added = true;
                }
            }

            // Kraken returns bars from `since` onwards. Advance cursor.
            let new_cursor = last_time + interval_secs;
            if new_cursor <= cursor || last_time >= end {
                break;
            }
            cursor = new_cursor;

            if !added && cursor >= end {
                break;
            }
        }

        all_bars.sort_by_key(|b| b.open_ts);
        all_bars.dedup_by_key(|b| b.open_ts);

        Ok(all_bars)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Kraken API objects
// ────────────────────────────────────────────────────────────────────────────

/// Standard Kraken REST response envelope.
///
/// Every response is wrapped as `{"error": [...], "result": ...}`.
#[derive(Debug, Deserialize)]
struct KrakenResponse<T> {
    error: Vec<String>,
    result: Option<T>,
}

/// One trading pair entry from `/0/public/AssetPairs`.
#[derive(Debug, Deserialize)]
struct PairInfo {
    /// WebSocket pair name (e.g. `"XBT/USD"`).
    wsname: Option<String>,

    /// Alternative pair name (e.g. `"XBTUSDT"`).
    altname: String,

    /// Base asset identifier (e.g. `"XXBT"`).
    base: String,

    /// Quote asset identifier (e.g. `"ZUSD"`).
    quote: String,

    /// Pair lifecycle status — only `"online"` pairs are usable.
    status: String,
}

impl TryFrom<PairInfo> for Asset {
    type Error = DataError;

    fn try_from(info: PairInfo) -> DataResult<Self> {
        // Prefer the human-readable wsname (e.g. "XBT/USD") for base/quote.
        let (base, quote) = if let Some(ref ws) = info.wsname {
            let mut parts = ws.splitn(2, '/');
            let b = parts.next().unwrap_or(&info.base).to_owned();
            let q = parts.next().unwrap_or(&info.quote).to_owned();
            (b, q)
        } else {
            (info.base.clone(), info.quote.clone())
        };

        // Normalize Kraken-specific tickers (e.g., XBT → BTC).
        let base = Kraken::normalize_ticker(&base);
        let quote = Kraken::normalize_ticker(&quote);

        let symbol = canonical_symbol(&info.altname, &Some(base.clone()), &quote);

        Ok(Asset {
            symbol: symbol.clone(),
            name: symbol,
            base: Some(base),
            quote,
            asset_type: AssetType::Crypto,
            exchange: "KRAKEN".to_owned(),
            volume: None,
            price: None,
        })
    }
}

/// One row from `/0/public/OHLC`.
///
/// Kraken returns: `[time, open, high, low, close, vwap, volume, count]`.
/// Timestamps are Unix **seconds** (not milliseconds).
#[derive(Debug, Copy, Clone)]
struct KrakenOHLC {
    /// Bar open time in Unix seconds.
    time: u64,

    /// Bar open price.
    open: f64,

    /// Highest price in the bar.
    high: f64,

    /// Lowest price in the bar.
    low: f64,

    /// Bar close price.
    close: f64,

    /// Traded volume.
    volume: f64,

    /// Number of trades during the bar.
    count: Option<i32>,
}

impl From<KrakenOHLC> for Bar {
    fn from(k: KrakenOHLC) -> Self {
        Bar {
            open_ts: k.time,
            close_ts: k.time,
            open_ts_exchange: k.time,
            open: k.open,
            high: k.high,
            low: k.low,
            close: k.close,
            adj_close: k.close,
            volume: k.volume,
            n_trades: k.count,
        }
    }
}

impl TryFrom<serde_json::Value> for KrakenOHLC {
    type Error = DataError;

    fn try_from(row: serde_json::Value) -> DataResult<Self> {
        let arr = row
            .as_array()
            .ok_or_else(|| DataError::UnexpectedResponse("OHLC row is not an array".to_owned()))?;

        let time = arr
            .first()
            .and_then(|v| v.as_u64())
            .ok_or_else(|| DataError::UnexpectedResponse("missing OHLC time".to_owned()))?;

        let open = arr
            .get(1)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing OHLC open".to_owned()))?;

        let high = arr
            .get(2)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing OHLC high".to_owned()))?;

        let low = arr
            .get(3)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing OHLC low".to_owned()))?;

        let close = arr
            .get(4)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing OHLC close".to_owned()))?;

        let volume = arr
            .get(6)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing OHLC volume".to_owned()))?;

        let count = arr.get(7).and_then(|v| v.as_i64()).map(|n| n as i32);

        Ok(Self {
            time,
            open,
            high,
            low,
            close,
            volume,
            count,
        })
    }
}
