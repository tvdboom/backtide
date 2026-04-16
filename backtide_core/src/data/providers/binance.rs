//! Binance data provider.
//!
//! No authentication is required for public market data endpoints.

use crate::constants::Symbol;
use crate::data::errors::{DataError, DataResult};
use crate::data::models::bar::Bar;
use crate::data::models::bar_download::BarDownload;
use crate::data::models::exchange::Exchange;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::data::providers::traits::DataProvider;
use crate::data::utils::canonical_symbol;
use crate::utils::http::{HttpClient, HttpClientConfig, HttpError};
use async_trait::async_trait;
use serde::Deserialize;
use tracing::{debug, info, instrument};

/// Binance spot-market data provider.
///
/// Wraps Binance's public REST API behind the [`DataProvider`] trait.
/// Only [`InstrumentType::Crypto`] is supported; all other instrument types return
/// [`DataError::UnsupportedInstrumentType`].
pub struct Binance {
    /// Shared async HTTP client.
    client: HttpClient,
}

impl Binance {
    /// Returns exchange metadata for trading pairs.
    const EXCHANGE_INFO_URL: &str = "https://api.binance.com/api/v3/exchangeInfo";

    const KLINES_URL: &str = "https://api.binance.com/api/v3/klines";

    /// Maximum klines returned per request by the Binance API.
    const MAX_KLINES_PER_REQUEST: usize = 1000;

    // ────────────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────────────

    /// Create a new [`Binance`] provider.
    pub async fn new() -> DataResult<Self> {
        let client = HttpClient::with_config(HttpClientConfig {
            max_concurrent_requests: 12,
            ..HttpClientConfig::default()
        })?;

        info!("Binance provider initialized.");
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

    /// Unwrap the Binance response envelope, returning the inner data
    /// or converting a Binance error into [`DataError`].
    fn unwrap_response<T>(resp: BinanceResponse<T>) -> DataResult<T> {
        match resp {
            BinanceResponse::Ok(data) => Ok(data),
            BinanceResponse::Err {
                code,
                msg,
            } => Err(DataError::UnexpectedResponse(format!("Binance error {code}: {msg}"))),
        }
    }

    /// Checks whether the instrument type is supported by the provider.
    fn check_instrument_type(instrument_type: InstrumentType) -> DataResult<()> {
        if instrument_type == InstrumentType::Crypto {
            Ok(())
        } else {
            Err(DataError::UnsupportedInstrumentType(instrument_type))
        }
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
        let parsed = HttpClient::json::<BinanceResponse<Vec<serde_json::Value>>>(resp).await?;
        let rows = Self::unwrap_response(parsed)?;

        if rows.is_empty() {
            return Err(HttpError::UnexpectedPayload(format!(
                "empty response for symbol: {}",
                symbol.to_owned()
            )))?;
        }

        rows.into_iter().map(BinanceKline::try_from).collect()
    }
}

#[async_trait]
impl DataProvider for Binance {
    /// Fetch metadata for a single symbol.
    #[instrument(skip(self), fields(%symbol))]
    async fn fetch_instrument(
        &self,
        symbol: &Symbol,
        instrument_type: InstrumentType,
    ) -> DataResult<Instrument> {
        Self::check_instrument_type(instrument_type)?;

        let binance_symbol = Self::parse_canonical_symbol(symbol);

        let resp = self
            .client
            .get(Self::EXCHANGE_INFO_URL, Some(&[("symbol", &binance_symbol)]))
            .await
            .map_err(|_| DataError::SymbolNotFound(symbol.to_owned()))?;

        let parsed = HttpClient::json::<BinanceResponse<ExchangeInfo>>(resp).await?;
        let info = Self::unwrap_response(parsed)?;

        let info = info
            .symbols
            .into_iter()
            .next()
            .ok_or_else(|| DataError::SymbolNotFound(symbol.to_owned()))?;

        Ok(Instrument::try_from(info)?)
    }

    /// Returns the usable download range for an instrument at a given interval.
    #[instrument(skip(self, instrument), fields(symbol = %instrument.symbol, ?interval))]
    async fn fetch_range(
        &self,
        instrument: Instrument,
        interval: Interval,
    ) -> DataResult<(u64, u64)> {
        Self::check_instrument_type(instrument.instrument_type)?;

        let symbol = Self::parse_canonical_symbol(&instrument.symbol);

        let (first, last) = tokio::try_join!(
            self.get_bars(&symbol, interval, Some(0), None, 1),
            self.get_bars(&symbol, interval, None, None, 1),
        )?;

        let earliest_ts = first
            .into_iter()
            .next()
            .ok_or_else(|| DataError::SymbolNotFound(instrument.symbol.clone()))?
            .open_time;

        let latest_ts = last
            .into_iter()
            .next()
            .ok_or_else(|| DataError::SymbolNotFound(instrument.symbol))?
            .close_time;

        Ok((earliest_ts, latest_ts))
    }

    /// List the spot crypto instruments traded on Binance, capped at `limit`.
    #[instrument(skip(self), fields(?instrument_type, limit))]
    async fn list_instruments(
        &self,
        instrument_type: InstrumentType,
        _: Option<Vec<Exchange>>,
        limit: usize,
    ) -> DataResult<Vec<Instrument>> {
        Self::check_instrument_type(instrument_type)?;

        let resp =
            self.client.get(Self::EXCHANGE_INFO_URL, Some(&[("permissions", "SPOT")])).await?;
        let parsed = HttpClient::json::<BinanceResponse<ExchangeInfo>>(resp).await?;
        let info = Self::unwrap_response(parsed)?;

        let instruments: Vec<Instrument> = info
            .symbols
            .into_iter()
            .filter(|s| s.status == "TRADING")
            .filter_map(|info| {
                Instrument::try_from(info)
                    .map_err(|e| {
                        debug!("Binance list_instruments error: {e}");
                        e
                    })
                    .ok()
            })
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
        let binance_symbol = Self::parse_canonical_symbol(symbol);
        let interval_secs = interval.minutes() * 60;
        let mut all_bars: Vec<Bar> = Vec::new();
        let mut cursor = start;

        while cursor < end {
            let bars = self
                .get_bars(
                    &binance_symbol,
                    interval,
                    Some(cursor as i64),
                    Some(end as i64),
                    Self::MAX_KLINES_PER_REQUEST,
                )
                .await;

            let bars = match bars {
                Ok(b) => b,
                Err(_) => break, // no more data
            };

            if bars.is_empty() {
                break;
            }

            let last_open_ts = bars.last().unwrap().open_time;

            for k in bars {
                let bar = Bar::from(k);
                if bar.open_ts >= start && bar.open_ts < end {
                    all_bars.push(bar);
                }
            }

            // Advance cursor past the last bar
            cursor = last_open_ts + interval_secs;
            if cursor <= last_open_ts {
                break; // safety: avoid infinite loop
            }
        }

        // Sort by open_ts and deduplicate
        all_bars.sort_by_key(|b| b.open_ts);
        all_bars.dedup_by_key(|b| b.open_ts);

        Ok(BarDownload {
            bars: all_bars,
            dividends: vec![],
        })
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Binance API objects
// ────────────────────────────────────────────────────────────────────────────

/// Standard Binance REST response envelope.
///
/// On success the payload is returned directly; on error Binance responds
/// with `{"code": <i32>, "msg": "<string>"}`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BinanceResponse<T> {
    Err {
        code: i32,
        msg: String,
    },
    Ok(T),
}

/// Exchange-info payload from `/api/v3/exchangeInfo`.
#[derive(Debug, Deserialize)]
struct ExchangeInfo {
    symbols: Vec<SymbolInfo>,
}

/// One trading pair entry from `/api/v3/exchangeInfo`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymbolInfo {
    /// Binance composite symbol.
    symbol: String,

    /// Lifecycle state — only `"TRADING"` pairs are usable.
    status: String,

    /// Base asset ticker.
    base_asset: String,

    /// Quote asset ticker.
    quote_asset: String,
}

impl TryFrom<SymbolInfo> for Instrument {
    type Error = DataError;

    fn try_from(info: SymbolInfo) -> DataResult<Self> {
        let base = info.base_asset;
        let quote = info.quote_asset;

        let symbol = canonical_symbol(&info.symbol, &Some(base.clone()), &quote);

        Ok(Instrument {
            symbol: symbol.clone(),
            name: symbol,
            base: Some(base),
            quote,
            instrument_type: InstrumentType::Crypto,
            exchange: "BINANCE".to_owned(), // Binance has no MIC code.
            provider: Provider::Binance,
        })
    }
}

/// One row from `/api/v3/klines`.
#[derive(Debug, Copy, Clone)]
struct BinanceKline {
    /// Bar open time in Unix seconds.
    open_time: u64,

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

    /// Bar close time in Unix seconds.
    close_time: u64,

    /// Number of trades during the bar.
    n_trades: Option<i32>,
}

impl From<BinanceKline> for Bar {
    fn from(k: BinanceKline) -> Self {
        Bar {
            open_ts: k.open_time,
            close_ts: k.close_time,
            open_ts_exchange: k.open_time,
            open: k.open,
            high: k.high,
            low: k.low,
            close: k.close,
            adj_close: k.close,
            volume: k.volume,
            n_trades: k.n_trades,
        }
    }
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

        let open = arr
            .get(1)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing kline open".to_owned()))?;

        let high = arr
            .get(2)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing kline high".to_owned()))?;

        let low = arr
            .get(3)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing kline low".to_owned()))?;

        let close = arr
            .get(4)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing kline close".to_owned()))?;

        let volume = arr
            .get(5)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| DataError::UnexpectedResponse("missing kline volume".to_owned()))?;

        let close_time =
            arr.get(6).and_then(|v| v.as_i64()).map(|ms| (ms / 1_000).max(0) as u64).ok_or_else(
                || DataError::UnexpectedResponse("missing kline close_time".to_owned()),
            )?;

        let n_trades = arr.get(8).and_then(|v| v.as_i64()).map(|n| n as i32);

        Ok(Self {
            open_time,
            open,
            high,
            low,
            close,
            volume,
            close_time,
            n_trades,
        })
    }
}
