//! Coinbase data provider.
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

/// Coinbase spot-market data provider.
///
/// Wraps Coinbase's public Advanced Trade REST API behind the [`DataProvider`]
/// trait.  Only [`AssetType::Crypto`] is supported; all other asset types
/// return [`DataError::UnsupportedAssetType`].
pub struct Coinbase {
    /// Shared async HTTP client.
    client: HttpClient,
}

impl Coinbase {
    /// Returns product metadata for a single trading pair.
    const PRODUCTS_URL: &str = "https://api.coinbase.com/api/v3/brokerage/market/products";

    /// Returns candle (OHLCV) data for a product.
    /// Usage: `{PRODUCTS_URL}/{product_id}/candles`
    const CANDLES_SUFFIX: &str = "candles";

    // ────────────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────────────

    /// Create a new [`Coinbase`] provider.
    pub async fn new() -> DataResult<Self> {
        let client = HttpClient::new()?;

        info!("Coinbase provider initialised");
        Ok(Self {
            client,
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private API
    // ────────────────────────────────────────────────────────────────────────

    /// Convert a canonical symbol (`BTC-USD`) to the Coinbase product id
    /// format, which is the same (`BTC-USD`).
    fn parse_canonical_symbol(symbol: &str) -> String {
        symbol.to_owned()
    }

    /// Convert an [`Interval`] to Coinbase's granularity string.
    ///
    /// Coinbase Advanced Trade accepts:
    /// `ONE_MINUTE`, `FIVE_MINUTE`, `FIFTEEN_MINUTE`, `THIRTY_MINUTE`,
    /// `ONE_HOUR`, `TWO_HOUR`, `SIX_HOUR`, `ONE_DAY`.
    fn interval_granularity(interval: Interval) -> &'static str {
        match interval {
            Interval::OneMinute => "ONE_MINUTE",
            Interval::FiveMinutes => "FIVE_MINUTE",
            Interval::FifteenMinutes => "FIFTEEN_MINUTE",
            Interval::ThirtyMinutes => "THIRTY_MINUTE",
            Interval::OneHour => "ONE_HOUR",
            Interval::FourHours => "SIX_HOUR", // closest available
            Interval::OneDay => "ONE_DAY",
            Interval::OneWeek => "ONE_DAY", // fetch daily, caller aggregates
        }
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

    /// Build the candles URL for a given product id.
    fn candles_url(product_id: &str) -> String {
        format!("{}/{}/{}", Self::PRODUCTS_URL, product_id, Self::CANDLES_SUFFIX)
    }

    /// Build the single-product URL for a given product id.
    fn product_url(product_id: &str) -> String {
        format!("{}/{}", Self::PRODUCTS_URL, product_id)
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
        let granularity = Self::interval_granularity(interval);
        let url = Self::candles_url(product_id);

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
    async fn get_asset(&self, symbol: &Symbol, asset_type: AssetType) -> DataResult<Asset> {
        Self::require_crypto(asset_type)?;

        let product_id = Self::parse_canonical_symbol(symbol);
        let url = Self::product_url(&product_id);

        let resp = self
            .client
            .get(&url, None)
            .await
            .map_err(|_| DataError::SymbolNotFound(symbol.to_owned()))?;

        let info = HttpClient::json::<ProductInfo>(resp).await?;

        Ok(Asset::try_from(info)?)
    }

    /// Returns the usable download range for an asset at a given interval.
    #[instrument(skip(self), fields(symbol = %asset.symbol, ?interval))]
    async fn get_download_range(&self, asset: Asset, interval: Interval) -> DataResult<(u64, u64)> {
        Self::require_crypto(asset.asset_type)?;

        let product_id = Self::parse_canonical_symbol(&asset.symbol);

        // Earliest: request from epoch 0
        // Latest: request without bounds (returns most recent candles)
        let (first, last) = tokio::try_join!(
            self.get_bars(&product_id, interval, Some(0), Some(1)),
            self.get_bars(&product_id, interval, None, None),
        )?;

        // Coinbase returns candles in descending order, so the earliest is
        // the last element of the "from epoch" request and the latest is the
        // first element of the unbounded request.
        let earliest_ts = first
            .into_iter()
            .last()
            .ok_or_else(|| DataError::SymbolNotFound(asset.symbol.clone()))?
            .start;

        let latest_ts =
            last.into_iter().next().ok_or_else(|| DataError::SymbolNotFound(asset.symbol))?.start;

        Ok((earliest_ts, latest_ts))
    }

    /// List the spot crypto assets traded on Coinbase, capped at `limit`.
    #[instrument(skip(self), fields(?asset_type, limit))]
    async fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>> {
        Self::require_crypto(asset_type)?;

        let resp = self
            .client
            .get(
                Self::PRODUCTS_URL,
                Some(&[("product_type", "SPOT"), ("limit", &limit.to_string())]),
            )
            .await?;

        let parsed = HttpClient::json::<ProductsListResponse>(resp).await?;

        let assets: Vec<Asset> = parsed
            .products
            .into_iter()
            .filter(|p| p.status.as_deref() == Some("online"))
            .filter_map(|info| {
                Asset::try_from(info)
                    .map_err(|e| {
                        debug!("Coinbase list_assets error: {e}");
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
    /// Coinbase product id (e.g. `"BTC-USD"`).
    product_id: String,

    /// Base currency id (e.g. `"BTC"`).
    base_currency_id: String,

    /// Quote currency id (e.g. `"USD"`).
    quote_currency_id: String,

    /// Product lifecycle status — only `"online"` products are usable.
    status: Option<String>,
}

impl TryFrom<ProductInfo> for Asset {
    type Error = DataError;

    fn try_from(info: ProductInfo) -> DataResult<Self> {
        let base = info.base_currency_id;
        let quote = info.quote_currency_id;

        let symbol = canonical_symbol(&info.product_id, &Some(base.clone()), &quote);

        Ok(Asset {
            symbol: symbol.clone(),
            name: symbol,
            base: Some(base),
            quote,
            asset_type: AssetType::Crypto,
            exchange: "COINBASE".to_owned(),
            volume: None,
            price: None,
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

    /// Close price as a string.
    close: String,
}

/// Parsed candle.
#[derive(Debug, Copy, Clone)]
struct CoinbaseCandle {
    /// Bar open time in Unix seconds.
    start: u64,

    /// Bar close price.
    close: f64,
}

impl TryFrom<CoinbaseCandleRaw> for CoinbaseCandle {
    type Error = DataError;

    fn try_from(raw: CoinbaseCandleRaw) -> DataResult<Self> {
        let start = raw.start.parse::<u64>().map_err(|_| {
            DataError::UnexpectedResponse("invalid candle start timestamp".to_owned())
        })?;

        let close = raw
            .close
            .parse::<f64>()
            .map_err(|_| DataError::UnexpectedResponse("invalid candle close price".to_owned()))?;

        Ok(Self {
            start,
            close,
        })
    }
}
