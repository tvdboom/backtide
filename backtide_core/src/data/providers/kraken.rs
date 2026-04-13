//! Kraken data provider.
//!
//! No authentication is required for public market data endpoints.

use crate::constants::Symbol;
use crate::data::errors::{DataError, DataResult};
use crate::data::models::bar::Bar;
use crate::data::models::bar_download::BarDownload;
use crate::data::models::currency::Currency;
use crate::data::models::exchange::Exchange;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::providers::traits::DataProvider;
use crate::data::utils::canonical_symbol;
use crate::utils::http::{HttpClient, HttpClientConfig, HttpError};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, instrument};

/// Kraken spot-market data provider.
///
/// Wraps Kraken's public REST API behind the [`DataProvider`] trait.
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
        // Kraken public API: 1 req/s for unauthenticated users (counter
        // decrements by 1/s, max burst of 15). Keep it conservative.
        let client = HttpClient::with_config(HttpClientConfig {
            max_concurrent_requests: 8,
            min_request_gap: Duration::from_millis(1000),
            ..HttpClientConfig::default()
        })?;

        info!("Kraken provider initialised");
        Ok(Self {
            client,
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private API
    // ────────────────────────────────────────────────────────────────────────

    /// Convert a canonical pair to Kraken format.
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

    /// Checks whether the instrument type is supported by the provider.
    fn check_instrument_type(instrument_type: InstrumentType) -> DataResult<()> {
        match instrument_type {
            InstrumentType::Crypto | InstrumentType::Forex => Ok(()),
            _ => Err(DataError::UnsupportedInstrumentType(instrument_type)),
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
    async fn get_instrument(
        &self,
        symbol: &Symbol,
        instrument_type: InstrumentType,
    ) -> DataResult<Instrument> {
        Self::check_instrument_type(instrument_type)?;

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

        Ok(Instrument::try_from(info)?)
    }

    /// Returns the usable download range for an instrument at a given interval.
    #[instrument(skip(self), fields(symbol = %instrument.symbol, ?interval))]
    async fn get_download_range(
        &self,
        instrument: Instrument,
        interval: Interval,
    ) -> DataResult<(u64, u64)> {
        Self::check_instrument_type(instrument.instrument_type)?;

        let symbol = self.parse_canonical_symbol(&instrument.symbol);

        let earliest_ts = self
            .get_bars(&symbol, interval, Some(0))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| DataError::SymbolNotFound(instrument.symbol.clone()))?
            .time;

        let latest_ts = self
            .get_bars(&symbol, interval, None)
            .await?
            .into_iter()
            .last()
            .ok_or_else(|| DataError::SymbolNotFound(instrument.symbol))?
            .time;

        Ok((earliest_ts, latest_ts))
    }

    /// List instruments traded on Kraken, filtered by `instrument_type` and capped at `limit`.
    ///
    /// The instrument type (Forex vs Crypto) is determined during [`PairInfo`]
    /// conversion — pairs where both sides are fiat [`Currency`] variants are
    /// classified as Forex, everything else as Crypto.
    #[instrument(skip(self), fields(?instrument_type, limit))]
    async fn list_instruments(
        &self,
        instrument_type: InstrumentType,
        _: Option<Vec<Exchange>>,
        limit: usize,
    ) -> DataResult<Vec<Instrument>> {
        Self::check_instrument_type(instrument_type)?;

        let resp = self.client.get(Self::ASSET_PAIRS_URL, None).await?;
        let parsed = HttpClient::json::<KrakenResponse<HashMap<String, PairInfo>>>(resp).await?;
        let map = Self::unwrap_response(parsed, "AssetPairs")?;

        let instruments: Vec<Instrument> = map
            .into_values()
            .filter(|p| p.status == "online")
            .filter_map(|info| {
                Instrument::try_from(info)
                    .map_err(|e| {
                        debug!("Kraken list_instruments error: {e}");
                        e
                    })
                    .ok()
            })
            .filter(|a| a.instrument_type == instrument_type)
            .take(limit)
            .collect();

        Ok(instruments)
    }

    /// Download OHLCV bars for `symbol` at `interval` from `start` to `end`.
    #[instrument(skip(self), fields(%symbol, ?interval, start, end))]
    async fn download_bars(
        &self,
        symbol: &str,
        _instrument_type: InstrumentType,
        interval: Interval,
        start: u64,
        end: u64,
    ) -> DataResult<BarDownload> {
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

        Ok(BarDownload {
            bars: all_bars,
            dividends: vec![],
        })
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
    /// WebSocket pair name.
    wsname: Option<String>,

    /// Alternative pair name.
    altname: String,

    /// Base asset identifier.
    base: String,

    /// Quote asset identifier.
    quote: String,

    /// Pair lifecycle status — only `"online"` pairs are usable.
    status: String,
}

impl TryFrom<PairInfo> for Instrument {
    type Error = DataError;

    fn try_from(info: PairInfo) -> DataResult<Self> {
        // Prefer the human-readable wsname (e.g., "XBT/USD") for base/quote.
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

        // Classify as Forex when both sides are fiat currencies, Crypto otherwise.
        let instrument_type =
            if base.parse::<Currency>().is_ok() && quote.parse::<Currency>().is_ok() {
                InstrumentType::Forex
            } else {
                InstrumentType::Crypto
            };

        Ok(Instrument {
            symbol: symbol.clone(),
            name: symbol,
            base: Some(base),
            quote,
            instrument_type,
            exchange: "KRAKEN".to_owned(),
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
