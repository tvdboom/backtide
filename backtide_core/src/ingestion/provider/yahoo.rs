//! Implementation of the `yahoo` provider.

use crate::ingestion::errors::{IngestionError, IngestionResult};
use crate::ingestion::provider::traits::DataProvider;
use crate::models::asset::{Asset, AssetType};
use crate::models::bar::Interval;
use crate::models::currency::Currency;
use crate::models::exchange::Exchange;
use crate::models::forex::ForexPair;
use crate::utils::http::{paginate, HttpClient};
use async_trait::async_trait;
use chrono::Utc;
use futures::future::join_all;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cmp::Ordering;
use strum::IntoEnumIterator;
use tokio::try_join;
use tracing::warn;

/// Provider to ingest data from Yahoo Finance.
pub struct YahooFinance {
    /// An async client to make requests with.
    client: HttpClient,

    /// CSRF protection token that verifies that a request was legitimate.
    crumb: String,
}

impl YahooFinance {
    /// Endpoint that seeds the session cookie required for authenticated requests.
    const COOKIE_SEED_URL: &str = "https://fc.yahoo.com";

    /// Endpoint that returns a one-time CSRF crumb token tied to the active session.
    const CRUMB_URL: &str = "https://query2.finance.yahoo.com/v1/test/getcrumb";

    /// Endpoint for the custom POST screener.
    const SCREENER_URL: &str = "https://query2.finance.yahoo.com/v1/finance/screener";

    /// Endpoint for Yahoo's predefined screeners.
    const PREDEFINED_SCREENER_URL: &str =
        "https://query2.finance.yahoo.com/v1/finance/screener/predefined/saved";

    /// Endpoint for per-symbol chart data (bars, metadata, and exchange timestamps).
    const CHART_URL: &str = "https://query2.finance.yahoo.com/v8/finance/chart";

    /// Maximum results returned per screener page.
    const PAGE_SIZE: usize = 100;

    // ────────────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────────────

    /// Create a new [`YahooFinance`] instance by opening an authenticated session.
    ///
    /// This performs two HTTP requests:
    /// 1. `GET` [`Self::COOKIE_SEED_URL`] — populates the cookie jar.
    /// 2. `GET` [`Self::CRUMB_URL`]       — retrieves the CSRF crumb.
    pub async fn new() -> IngestionResult<Self> {
        let client = HttpClient::new().map_err(|e| IngestionError::Http(e))?;

        let crumb = Self::fetch_crumb(&client).await?;

        Ok(Self {
            client,
            crumb,
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private API
    // ────────────────────────────────────────────────────────────────────────

    /// Seed the cookie jar and then return a fresh crumb string.
    ///
    /// Both requests reuse `client` so the same cookie jar is populated and
    /// read within the same call.
    async fn fetch_crumb(client: &HttpClient) -> IngestionResult<String> {
        // Ignore the response status entirely — we only care about Set-Cookie headers.
        // fc.yahoo.com commonly returns 404 but still seeds the session cookie.
        let _ = client.inner.get(Self::COOKIE_SEED_URL).send().await;

        let crumb_resp = client
            .get(Self::CRUMB_URL, None)
            .await
            .map_err(|e| IngestionError::Auth(format!("Crumb request failed: {e}")))?;

        let crumb = crumb_resp
            .text()
            .await
            .map_err(|e| IngestionError::Auth(format!("Failed to read crumb: {e}")))?;

        if crumb.is_empty() {
            return Err(IngestionError::Auth(
                "Yahoo returned an empty crumb — session cookie may be missing".to_owned(),
            ));
        }

        Ok(crumb)
    }

    /// Paginate through a predefined Yahoo screener by its screener ID.
    async fn fetch_predefined_screener(
        &self,
        scr_id: &str,
        limit: usize,
    ) -> IngestionResult<Vec<Asset>> {
        let quotes = paginate(limit, Self::PAGE_SIZE, |batch, offset| async move {
            let resp = self
                .client
                .get(
                    Self::PREDEFINED_SCREENER_URL,
                    Some(&[
                        ("scrIds", scr_id),
                        ("count ", &batch.to_string()),
                        ("start", &offset.to_string()),
                        ("crumb", &self.crumb),
                        ("lang", "en-US"),
                        ("region", "US"),
                    ]),
                )
                .await?;

            Self::parse_quotes(resp).await
        })
        .await?;

        Ok(quotes.into_iter().map(Asset::from).collect())
    }

    /// Paginate through the POST screener endpoint using a custom query body.
    async fn paginate_screener(
        &self,
        quote_type: &str,
        query: &Value,
        limit: usize,
    ) -> IngestionResult<Vec<Asset>> {
        let quotes = paginate(limit, Self::PAGE_SIZE, |batch, offset| {
            let payload = json!({
                "size": batch,
                "offset": offset,
                "sortField": "dayvolume",
                "sortType": "DESC",
                "quoteType": quote_type,
                "query": query,
                "userId": "",
                "userIdType": "guid",
            });

            async move {
                let resp = self
                    .client
                    .post(
                        Self::SCREENER_URL,
                        &[("crumb", &self.crumb), ("lang", "en-US"), ("region", "US")],
                        &payload,
                    )
                    .await?;

                Self::parse_quotes(resp).await
            }
        })
        .await?;

        Ok(quotes.into_iter().map(Asset::from).collect())
    }

    /// Validate HTTP status then deserialize screener JSON into a list of quotes.
    async fn parse_quotes(resp: reqwest::Response) -> IngestionResult<Vec<YahooQuote>> {
        let parsed = HttpClient::json::<ScreenerResponse>(resp).await?;

        parsed.finance.result.into_iter().next().map(|r| r.quotes).ok_or_else(|| {
            IngestionError::UnexpectedResponse("empty screener result array".to_owned())
        })
    }
}

#[async_trait]
impl DataProvider for YahooFinance {
    /// Get an asset given a symbol. This method returns the asset with all metadata.
    async fn get_asset(&self, _: AssetType, symbol: &str) -> IngestionResult<Asset> {
        let symbol = symbol.to_owned();

        let resp = self
            .client
            .get(
                &format!("{}/{symbol}", Self::CHART_URL),
                Some(&[("range", "1d"), ("interval", "1d"), ("crumb", &self.crumb)]),
            )
            .await?;

        let parsed = HttpClient::json::<ChartResponse>(resp).await?;

        let meta = parsed.chart.result.into_iter().next().map(|r| r.meta).ok_or_else(|| {
            IngestionError::NotFound(symbol.clone())
        })?;

        let currency = meta.currency.ok_or_else(|| {
            IngestionError::UnexpectedResponse(format!("no currency for symbol: {symbol}"))
        })?;

        let asset_type = match meta.instrument_type.as_deref() {
            Some("EQUITY") => AssetType::Stocks,
            Some("ETF") => AssetType::Etf,
            Some("CURRENCY") => AssetType::Forex,
            Some("CRYPTOCURRENCY") => AssetType::Crypto,
            s => {
                return Err(IngestionError::UnexpectedResponse(format!(
                    "unknown instrument type: {s:?}"
                )))
            },
        };

        Ok(Asset {
            symbol: symbol.to_owned(),
            name: meta.short_name.or(meta.long_name).unwrap_or_else(|| symbol.to_owned()),
            currency,
            asset_type,
            volume: meta.regular_market_volume,
            price: meta.regular_market_price,
            start_date: Some(meta.first_trade_date.unwrap_or(Utc::now().timestamp())),
            end_date: Some(meta.regular_market_time.unwrap_or(0)),
        })
    }

    async fn list_assets(
        &self,
        asset_type: AssetType,
        limit: usize,
    ) -> IngestionResult<Vec<Asset>> {
        match asset_type {
            AssetType::Stocks | AssetType::Etf => {
                let (quote_type, operands) = match asset_type {
                    AssetType::Stocks => {
                        (
                            "equity",
                            vec![
                                json!({ "operator": "EQ", "operands": ["quoteType", "equity"] }),
                                // Only select large companies
                                json!({ "operator": "GT", "operands": ["intradaymarketcap", 1000000000] }),
                                // Remove penny stocks
                                json!({ "operator": "GT", "operands": ["regularMarketPrice", 5.0] }),
                            ],
                        )
                    },
                    AssetType::Etf => {
                        ("etf", vec![json!({ "operator": "EQ", "operands": ["quoteType", "etf"] })])
                    },
                    _ => unreachable!(),
                };

                // Fan out across all exchanges concurrently.
                // Log exchange-level failures rather than aborting the whole call.
                let tasks: Vec<_> = Exchange::iter()
                    .map(|ex| {
                        let mut operands = operands.clone();
                        operands.push(
                            json!({ "operator": "EQ", "operands": ["exchange", ex.yahoo_code()] }),
                        );
                        let query = json!({ "operator": "AND", "operands": operands });
                        async move { self.paginate_screener(quote_type, &query, 100).await }
                    })
                    .collect();

                let mut assets = Vec::new();
                for result in join_all(tasks).await {
                    match result {
                        Ok(batch) => assets.extend(batch),
                        Err(e) => warn!("Yahoo list_assets exchange error: {e}"),
                    }
                }

                // Select only the top `limit` assets by volume x price
                assets.sort_by(|a, b| {
                    b.volume_price().partial_cmp(&a.volume_price()).unwrap_or(Ordering::Equal)
                });
                assets.truncate(limit);

                Ok(assets)
            },

            AssetType::Forex => Ok(ForexPair::iter()
                .map(|f| Asset {
                    symbol: if f.base() != Currency::USD {
                        format!("{f:?}=X")
                    } else {
                        format!("{}=X", f.quote())
                    },
                    name: f.to_string(),
                    currency: f.quote().to_string(),
                    asset_type: AssetType::Forex,
                    start_date: Some(0),
                    end_date: Some(Utc::now().timestamp()),
                    volume: None,
                    price: None,
                })
                .collect()),

            AssetType::Crypto => {
                // Fetch a large pool from multiple regional screeners concurrently
                let (us, eu, gb) = try_join!(
                    self.fetch_predefined_screener("all_cryptocurrencies_us", 100),
                    self.fetch_predefined_screener("all_cryptocurrencies_eu", 100),
                    self.fetch_predefined_screener("all_cryptocurrencies_gb", 100),
                )?;

                let mut assets: Vec<Asset> = us.into_iter().chain(eu).chain(gb).collect();

                assets.sort_by(|a, b| {
                    b.volume_price().partial_cmp(&a.volume_price()).unwrap_or(Ordering::Equal)
                });
                assets.truncate(limit);

                Ok(assets)
            },
        }
    }

    fn list_intervals(&self) -> Vec<Interval> {
        vec![
            Interval::OneMinute,
            Interval::TwoMinutes,
            Interval::FiveMinutes,
            Interval::FifteenMinutes,
            Interval::ThirtyMinutes,
            Interval::OneHour,
            Interval::OneDay,
            Interval::FiveDays,
            Interval::OneWeek,
            Interval::OneMonth,
            Interval::ThreeMonths,
        ]
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Yahoo API objects
// ────────────────────────────────────────────────────────────────────────────

/// Raw quote shape returned by the Yahoo Finance endpoint.
/// Fields are `Option` because Yahoo omits them inconsistently.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YahooQuote {
    pub symbol: String,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub currency: String,
    pub quote_type: String,
    pub regular_market_volume: Option<u64>,
    pub regular_market_price: Option<f64>,
}

impl From<YahooQuote> for Asset {
    fn from(q: YahooQuote) -> Self {
        let name = q.short_name.or(q.long_name).unwrap_or_else(|| q.symbol.clone());

        Self {
            symbol: q.symbol,
            name,
            currency: q.currency,
            asset_type: match q.quote_type.as_str() {
                "EQUITY" => AssetType::Stocks,
                "ETF" => AssetType::Etf,
                "CURRENCY" => AssetType::Forex,
                "CRYPTOCURRENCY" => AssetType::Crypto,
                _ => unreachable!(),
            },
            start_date: None,
            end_date: None,
            volume: q.regular_market_volume,
            price: q.regular_market_price,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ScreenerResponse {
    finance: ScreenerFinance,
}

#[derive(Debug, Deserialize)]
struct ScreenerFinance {
    result: Vec<ScreenerResult>,
}

#[derive(Debug, Deserialize)]
struct ScreenerResult {
    quotes: Vec<YahooQuote>,
}

#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: ChartBody,
}

#[derive(Debug, Deserialize)]
struct ChartBody {
    result: Vec<ChartResult>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    meta: ChartMeta,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChartMeta {
    short_name: Option<String>,
    long_name: Option<String>,
    currency: Option<String>,
    instrument_type: Option<String>,
    first_trade_date: Option<i64>,
    regular_market_time: Option<i64>,
    regular_market_price: Option<f64>,
    regular_market_volume: Option<u64>,
}
