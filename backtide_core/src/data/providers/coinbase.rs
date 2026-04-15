//! Coinbase data provider.
//!
//! No authentication is required for public market data endpoints.

use crate::constants::Symbol;
use crate::data::errors::{DataError, DataResult};
use crate::data::models::bar::Bar;
use crate::data::models::bar_download::BarDownload;
use crate::data::models::exchange::Exchange;
use crate::data::models::instrument::Instrument;
use crate::data::models::provider::Provider;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::providers::traits::DataProvider;
use crate::data::utils::canonical_symbol;
use crate::utils::http::{HttpClient, HttpClientConfig, HttpError};
use async_trait::async_trait;
use chrono::DateTime;
use serde::Deserialize;
use tracing::{debug, info, instrument};

/// Coinbase spot-market data provider.
///
/// Wraps Coinbase's public Advanced Trade REST API behind the [`DataProvider`]
/// trait. Only [`InstrumentType::Crypto`] is supported; all other instrument types
/// return [`DataError::UnsupportedInstrumentType`].
pub struct Coinbase {
    /// Shared async HTTP client.
    client: HttpClient,
}

impl Coinbase {
    /// Returns product metadata for a single trading pair.
    const PRODUCTS_URL: &str = "https://api.coinbase.com/api/v3/brokerage/market/products";

    /// Maximum candles returned by the candles endpoint.
    const MAX_CANDLES_PER_REQUEST: u64 = 350;

    // ────────────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────────────

    /// Create a new [`Coinbase`] provider.
    pub async fn new() -> DataResult<Self> {
        let client = HttpClient::with_config(HttpClientConfig {
            max_concurrent_requests: 1,
            ..HttpClientConfig::default()
        })?;

        info!("Coinbase provider initialised");
        Ok(Self {
            client,
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private API
    // ────────────────────────────────────────────────────────────────────────

    /// Convert an [`Interval`] to Coinbase's granularity string.
    fn interval_granularity(interval: Interval) -> DataResult<&'static str> {
        match interval {
            Interval::OneMinute => Ok("ONE_MINUTE"),
            Interval::FiveMinutes => Ok("FIVE_MINUTE"),
            Interval::FifteenMinutes => Ok("FIFTEEN_MINUTE"),
            Interval::ThirtyMinutes => Ok("THIRTY_MINUTE"),
            Interval::OneHour => Ok("ONE_HOUR"),
            Interval::FourHours => Ok("FOUR_HOUR"),
            Interval::OneDay => Ok("ONE_DAY"),
            Interval::OneWeek => Err(DataError::UnsupportedInterval(interval)),
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

    /// Fetch product metadata for a single symbol.
    #[instrument(skip(self), fields(%product_id))]
    async fn get_product_info(&self, product_id: &str) -> DataResult<ProductInfo> {
        let resp = self
            .client
            .get(&format!("{}/{}", Self::PRODUCTS_URL, product_id), None)
            .await
            .map_err(|_| DataError::SymbolNotFound(product_id.to_owned()))?;

        Ok(HttpClient::json::<ProductInfo>(resp).await?)
    }

    /// Fetch candles for a product between Unix timestamps `start` and `end`.
    #[instrument(skip(self), fields(%product_id))]
    async fn get_bars(
        &self,
        product_id: &str,
        interval: Interval,
        start: Option<u64>,
        end: Option<u64>,
    ) -> DataResult<Vec<CoinbaseCandle>> {
        let granularity = Self::interval_granularity(interval)?;
        let url = format!("{}/{}/candles", Self::PRODUCTS_URL, product_id);

        let mut params: Vec<(&str, String)> = vec![("granularity", granularity.to_owned())];

        if let Some(s) = start {
            params.push(("start", s.to_string()));
        }
        if let Some(e) = end {
            params.push(("end", e.to_string()));
        }

        let params_ref: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let resp = self.client.get(&url, Some(&params_ref)).await?;
        let parsed = HttpClient::json::<CoinbaseCandlesResponse>(resp).await?;

        if parsed.candles.is_empty() {
            return Err(HttpError::UnexpectedPayload(format!(
                "empty response for product: {}",
                product_id
            )))?;
        }

        let bars: Vec<CoinbaseCandle> =
            parsed.candles.into_iter().map(CoinbaseCandle::try_from).collect::<DataResult<_>>()?;

        Ok(bars)
    }
}

#[async_trait]
impl DataProvider for Coinbase {
    /// Fetch metadata for a single symbol.
    #[instrument(skip(self), fields(%symbol))]
    async fn fetch_instrument(
        &self,
        symbol: &Symbol,
        instrument_type: InstrumentType,
    ) -> DataResult<Instrument> {
        Self::check_instrument_type(instrument_type)?;

        let info = self.get_product_info(symbol).await?;

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

        let product_id = instrument.symbol.clone();
        let latest_bars = self.get_bars(&product_id, interval, None, None).await?;

        let latest_ts = latest_bars
            .iter()
            .map(|bar| bar.start)
            .max()
            .ok_or_else(|| DataError::SymbolNotFound(product_id.clone()))?;

        let product = self.get_product_info(&product_id).await?;
        let fallback_earliest_ts =
            latest_bars.iter().map(|bar| bar.start).min().unwrap_or(latest_ts);

        let earliest_ts = if let Some(new_at) = product.new_at.as_deref() {
            let start = DateTime::parse_from_rfc3339(new_at)
                .map(|ts| ts.timestamp().max(0) as u64)
                .map_err(|_| {
                    DataError::UnexpectedResponse(format!(
                        "invalid Coinbase new_at timestamp: {new_at}"
                    ))
                })?;

            let span = (interval.minutes() * 60)
                .saturating_mul(Self::MAX_CANDLES_PER_REQUEST.saturating_sub(1));

            match self
                .get_bars(&product_id, interval, Some(start), Some(start.saturating_add(span)))
                .await
            {
                Ok(bars) => bars.iter().map(|bar| bar.start).min().unwrap_or(fallback_earliest_ts),
                Err(e) => {
                    debug!(%product_id, ?interval, "Coinbase earliest window probe failed: {e}");
                    fallback_earliest_ts
                },
            }
        } else {
            fallback_earliest_ts
        };

        Ok((earliest_ts, latest_ts))
    }

    /// List the spot crypto instruments traded on Coinbase, capped at `limit`.
    #[instrument(skip(self), fields(?instrument_type, limit))]
    async fn list_instruments(
        &self,
        instrument_type: InstrumentType,
        _: Option<Vec<Exchange>>,
        limit: usize,
    ) -> DataResult<Vec<Instrument>> {
        Self::check_instrument_type(instrument_type)?;

        let resp = self
            .client
            .get(
                Self::PRODUCTS_URL,
                Some(&[("product_type", "SPOT"), ("limit", &limit.to_string())]),
            )
            .await?;

        let parsed = HttpClient::json::<ProductsListResponse>(resp).await?;

        let instruments: Vec<Instrument> = parsed
            .products
            .into_iter()
            .filter(|p| p.status.as_deref() == Some("online"))
            .filter_map(|info| {
                Instrument::try_from(info)
                    .map_err(|e| {
                        debug!("Coinbase list_instruments error: {e}");
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
        let interval_secs = interval.minutes() * 60;
        let window = interval_secs * (Self::MAX_CANDLES_PER_REQUEST - 1);
        let mut all_bars: Vec<Bar> = Vec::new();
        let mut cursor = start;

        while cursor < end {
            let batch_end = (cursor + window).min(end);

            let bars = self.get_bars(symbol, interval, Some(cursor), Some(batch_end)).await;

            let bars = match bars {
                Ok(b) => b,
                Err(_) => break,
            };

            if bars.is_empty() {
                break;
            }

            for c in &bars {
                let bar = Bar::from(*c);
                if bar.open_ts >= start && bar.open_ts < end {
                    all_bars.push(bar);
                }
            }

            cursor = batch_end;
            if cursor <= start && !bars.is_empty() {
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
// Coinbase API objects
// ────────────────────────────────────────────────────────────────────────────

/// Response envelope for the product list endpoint.
#[derive(Debug, Deserialize)]
struct ProductsListResponse {
    products: Vec<ProductInfo>,
}

/// One product entry from `/api/v3/brokerage/market/products`.
#[derive(Debug, Deserialize)]
struct ProductInfo {
    /// Coinbase product id.
    product_id: String,

    /// Base currency id.
    base_currency_id: String,

    /// Quote currency id.
    quote_currency_id: String,

    /// Product lifecycle status — only `"online"` products are usable.
    status: Option<String>,

    /// Product launch timestamp used to anchor historical candle requests.
    new_at: Option<String>,
}

impl TryFrom<ProductInfo> for Instrument {
    type Error = DataError;

    fn try_from(info: ProductInfo) -> DataResult<Self> {
        let base = info.base_currency_id;
        let quote = info.quote_currency_id;

        let symbol = canonical_symbol(&info.product_id, &Some(base.clone()), &quote);

        Ok(Instrument {
            symbol: symbol.clone(),
            name: symbol,
            base: Some(base),
            quote,
            instrument_type: InstrumentType::Crypto,
            exchange: "COINBASE".to_owned(),
            provider: Provider::Coinbase,
        })
    }
}

/// Response envelope for the candles endpoint.
#[derive(Debug, Deserialize)]
struct CoinbaseCandlesResponse {
    candles: Vec<CoinbaseCandleRaw>,
}

/// Raw candle row from `/api/v3/brokerage/market/products/{product_id}/candles`.
///
/// Coinbase returns: `{"start", "low", "high", "open", "close", "volume"}`.
/// Timestamps are Unix **seconds** as string.
#[derive(Debug, Deserialize)]
struct CoinbaseCandleRaw {
    /// Bar open time as a string of Unix seconds.
    start: String,

    /// Open price as a string.
    open: String,

    /// Highest price as a string.
    high: String,

    /// Lowest price as a string.
    low: String,

    /// Close price as a string.
    close: String,

    /// Volume as a string.
    volume: String,
}

/// Parsed candle.
#[derive(Debug, Copy, Clone)]
struct CoinbaseCandle {
    /// Bar open time in Unix seconds.
    start: u64,

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
}

impl From<CoinbaseCandle> for Bar {
    fn from(c: CoinbaseCandle) -> Self {
        Bar {
            open_ts: c.start,
            close_ts: c.start, // Coinbase only gives open ts
            open_ts_exchange: c.start,
            open: c.open,
            high: c.high,
            low: c.low,
            close: c.close,
            adj_close: c.close,
            volume: c.volume,
            n_trades: None,
        }
    }
}

impl TryFrom<CoinbaseCandleRaw> for CoinbaseCandle {
    type Error = DataError;

    fn try_from(raw: CoinbaseCandleRaw) -> DataResult<Self> {
        let start = raw.start.parse::<u64>().map_err(|_| {
            DataError::UnexpectedResponse("invalid candle start timestamp".to_owned())
        })?;

        let open = raw
            .open
            .parse::<f64>()
            .map_err(|_| DataError::UnexpectedResponse("invalid candle open price".to_owned()))?;

        let high = raw
            .high
            .parse::<f64>()
            .map_err(|_| DataError::UnexpectedResponse("invalid candle high price".to_owned()))?;

        let low = raw
            .low
            .parse::<f64>()
            .map_err(|_| DataError::UnexpectedResponse("invalid candle low price".to_owned()))?;

        let close = raw
            .close
            .parse::<f64>()
            .map_err(|_| DataError::UnexpectedResponse("invalid candle close price".to_owned()))?;

        let volume = raw
            .volume
            .parse::<f64>()
            .map_err(|_| DataError::UnexpectedResponse("invalid candle volume".to_owned()))?;

        Ok(Self {
            start,
            open,
            high,
            low,
            close,
            volume,
        })
    }
}
