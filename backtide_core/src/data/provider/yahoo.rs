use crate::data::asset::Asset;
use crate::data::utils::MarketDataError;
use crate::data::MarketDataProvider;
use crate::utils::http::{paginate, HttpClient};
use async_trait::async_trait;
use futures::future::join_all;
use reqwest::cookie::Jar;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::sync::Arc;
use tokio::try_join;

pub struct YahooFinance {
    /// An async client to make requests with
    client: HttpClient,

    /// CSRF protection token that verifies that a request was legitimate.
    crumb: String,
}

impl YahooFinance {
    /// Endpoint that seeds the session cookie required for authenticated requests.
    const COOKIE_SEED_URL: &str = "https://fc.yahoo.com";

    /// Endpoint that returns a one-time CSRF crumb token tied to the active session.
    const CRUMB_URL: &str = "https://query2.finance.yahoo.com/v1/test/getcrumb";

    const SCREENER_URL: &str = "https://query2.finance.yahoo.com/v1/finance/screener";
    const PREDEFINED_URL: &str =
        "https://query2.finance.yahoo.com/v1/finance/screener/predefined/saved";

    /// Maximum results returned per screener page.
    const PAGE_SIZE: usize = 100;

    /// User-agent sent with every request. A generic browser string is required;
    /// Yahoo rejects requests that use the default `reqwest` agent.
    const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

    /// Exchange codes for major markets
    const EXCHANGES: &[&str] = &[
        // United States
        "NMS", // NASDAQ
        "NYQ", // New York Stock Exchange
        "NGM", // NASDAQ Global Market
        "NCM", // NASDAQ Capital Market
        "ASE", // NYSE American (AMEX)
        // Europe
        "AMS", // Euronext Amsterdam
        "PAR", // Euronext Paris
        "GER", // XETRA Frankfurt
        "LSE", // London Stock Exchange
        "MCE", // Bolsa de Madrid
        "STO", // Nasdaq Stockholm
        "OSL", // Oslo Børs
        "CPH", // Nasdaq Copenhagen
        "HEL", // Nasdaq Helsinki
        "VIE", // Vienna Stock Exchange
        "BRU", // Euronext Brussels
        "LIS", // Euronext Lisbon
        "MIL", // Borsa Italiana Milan
        "SWX", // SIX Swiss Exchange
        // Asia & Pacific
        "JPX", // Japan Exchange Group (Tokyo)
        "HKG", // Hong Kong Stock Exchange
        "SHH", // Shanghai Stock Exchange
        "SHZ", // Shenzhen Stock Exchange
        "KSC", // Korea Exchange (Seoul)
        "TAI", // Taiwan Stock Exchange
        "NSI", // National Stock Exchange of India
        "BSE", // Bombay Stock Exchange
        "ASX", // Australian Securities Exchange
        "SGX", // Singapore Exchange
        "NZE", // New Zealand Exchange
    ];

    // ── Public API ──────────────────────────────────────────────────────────

    /// Create a new [`YahooFinance`] instance by opening an authenticated session.
    ///
    /// This performs two HTTP requests:
    /// 1. `GET` [`Self::COOKIE_SEED_URL`] — populates the cookie jar.
    /// 2. `GET` [`Self::CRUMB_URL`]       — retrieves the CSRF crumb.
    ///
    /// # Errors
    ///
    /// Returns [`MarketDataError::Auth`] if the HTTP client cannot be built,
    /// either request fails, or the crumb response is empty.
    pub async fn new() -> Result<Self, MarketDataError> {
        let jar = Arc::new(Jar::default());

        let client = HttpClient::new(Self::USER_AGENT, jar)
            .map_err(|e| MarketDataError::Auth(format!("Failed to build HTTP client: {e}")))?;

        let crumb = Self::fetch_crumb(&client).await?;
        Ok(Self {
            client,
            crumb,
        })
    }

    // ── Private API ─────────────────────────────────────────────────────────

    /// Seed the cookie jar and then return a fresh crumb string.
    ///
    /// Both requests reuse `client` so the same cookie jar is populated and
    /// read within the same call.
    async fn fetch_crumb(client: &HttpClient) -> Result<String, MarketDataError> {
        // Ignore the response status entirely — we only care about Set-Cookie headers.
        // fc.yahoo.com commonly returns 404 but still seeds the session cookie.
        let _ = client.inner.get(Self::COOKIE_SEED_URL).send().await;

        let crumb_resp = client
            .get(Self::CRUMB_URL, None)
            .await
            .map_err(|e| MarketDataError::Auth(format!("Crumb request failed: {e}")))?;

        let crumb = crumb_resp
            .text()
            .await
            .map_err(|e| MarketDataError::Auth(format!("Failed to read crumb: {e}")))?;

        if crumb.is_empty() {
            return Err(MarketDataError::Auth(
                "Yahoo returned an empty crumb — session cookie may be missing".to_string(),
            ));
        }

        Ok(crumb)
    }

    /// Fetch equities matching a set of exchange codes.
    async fn fetch_by_exchange(
        &self,
        quote_type: &str,
        exchange: &str,
        limit: usize,
    ) -> Result<Vec<Asset>, MarketDataError> {
        let mut operands = vec![
            json!({ "operator": "EQ", "operands": ["quoteType", quote_type] }),
            json!({ "operator": "EQ", "operands": ["exchange", exchange] }),
        ];

        if quote_type != "etf" {
            operands
                .push(json!({ "operator": "GT", "operands": ["intradaymarketcap", 1000000000] }));
            operands.push(json!({ "operator": "GT", "operands": ["regularMarketPrice", 5.0] }));
        }

        let query = json!({
            "operator": "AND",
            "operands": operands
        });

        self.paginate_screener(quote_type, &query, limit).await
    }

    /// Paginate through a predefined Yahoo screener by its screener ID.
    async fn fetch_predefined(
        &self,
        scr_id: &str,
        limit: usize,
    ) -> Result<Vec<Asset>, MarketDataError> {
        let quotes = paginate(limit, Self::PAGE_SIZE, |batch, offset| async move {
            let resp = self
                .client
                .get(
                    Self::PREDEFINED_URL,
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
    ) -> Result<Vec<Asset>, MarketDataError> {
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

        Ok(quotes
            .into_iter()
            .filter_map(|quote| quote.currency.is_some().then(|| Asset::from(quote)))
            .collect())
    }

    /// Validate HTTP status then deserialize screener JSON into a list of quotes.
    async fn parse_quotes(resp: reqwest::Response) -> Result<Vec<YahooQuote>, MarketDataError> {
        let parsed = HttpClient::json::<ScreenerResponse>(resp).await?;

        parsed.finance.result.into_iter().next().map(|r| r.quotes).ok_or_else(|| {
            MarketDataError::UnexpectedResponse("Empty screener result array".to_string())
        })
    }
}

#[async_trait]
impl MarketDataProvider for YahooFinance {
    /// Return the top `limit` most active equities and returns the merged results.
    async fn list_stocks(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError> {
        let futures: Vec<_> =
            Self::EXCHANGES.iter().map(|ex| self.fetch_by_exchange("equity", ex, 50)).collect();

        let results = join_all(futures).await;

        let mut assets: Vec<Asset> = results.into_iter().filter_map(|r| r.ok()).flatten().collect();
        assets.sort_by(|a, b| {
            b.volume_price().partial_cmp(&a.volume_price()).unwrap_or(Ordering::Equal)
        });
        assets.truncate(limit);

        Ok(assets)
    }

    /// Return the top `_limit` most active forex pairs.
    async fn list_forex(&self, _limit: usize) -> Result<Vec<Asset>, MarketDataError> {
        // Yahoo doesn't have a standard way to retrieve forex pairs
        Ok(vec![])
    }

    /// Return the top `limit` most active ETFs.
    async fn list_etf(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError> {
        let futures: Vec<_> =
            Self::EXCHANGES.iter().map(|ex| self.fetch_by_exchange("etf", ex, 30)).collect();

        let results = join_all(futures).await;

        let mut assets: Vec<Asset> = results.into_iter().filter_map(|r| r.ok()).flatten().collect();

        assets.sort_by(|a, b| {
            b.volume_price().partial_cmp(&a.volume_price()).unwrap_or(Ordering::Equal)
        });
        assets.truncate(limit);

        Ok(assets)
    }

    /// Return the top `limit` most active cryptocurrencies.
    async fn list_crypto(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError> {
        // Fetch a large pool from multiple regional screeners concurrently
        let (us, eu, gb) = try_join!(
            self.fetch_predefined("all_cryptocurrencies_us", 50),
            self.fetch_predefined("all_cryptocurrencies_eu", 50),
            self.fetch_predefined("all_cryptocurrencies_gb", 50),
        )?;

        let mut assets: Vec<Asset> = us.into_iter().chain(eu).chain(gb).collect();

        assets.sort_by(|a, b| {
            b.volume_price().partial_cmp(&a.volume_price()).unwrap_or(Ordering::Equal)
        });
        assets.truncate(limit);

        Ok(assets)
    }
}

/// Raw quote shape returned by the Yahoo Finance screener endpoint.
/// Fields are `Option` because Yahoo omits them inconsistently.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YahooQuote {
    pub symbol: String,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub currency: Option<String>,
    pub regular_market_volume: Option<u64>,
    pub regular_market_price: Option<f64>,
}

impl From<YahooQuote> for Asset {
    fn from(q: YahooQuote) -> Self {
        let name = q.short_name.or(q.long_name).unwrap_or_else(|| q.symbol.clone());

        Self {
            symbol: q.symbol,
            name,
            currency: q.currency.unwrap(),
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
