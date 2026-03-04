//! Yahoo Finance implementation of [`MarketDataProvider`].

pub mod auth;
pub mod http;

use async_trait::async_trait;
use reqwest::Client;

use crate::error::MarketDataError;
use crate::models::Asset;
use crate::provider::MarketDataProvider;
use crate::yahoo::auth::YahooAuth;
use crate::yahoo::http::{fetch_by_exchanges, fetch_by_quote_type, fetch_predefined};

// ─── Exchange groups ──────────────────────────────────────────────────────────

/// Yahoo Finance exchange codes for US markets.
const US_EXCHANGES: &[&str] = &["NMS", "NYQ", "NGM", "NCM", "ASE"];

/// Yahoo Finance exchange codes for major European exchanges.
const EU_EXCHANGES: &[&str] = &[
    "AMS", // Amsterdam (Euronext)
    "PAR", // Paris (Euronext)
    "GER", // Frankfurt (XETRA)
    "LSE", // London
    "MCE", // Madrid
    "STO", // Stockholm
    "OSL", // Oslo
    "CPH", // Copenhagen
    "HEL", // Helsinki
    "VIE", // Vienna
    "BRU", // Brussels
    "LIS", // Lisbon
    "MIL", // Milan
    "SWX", // Swiss Exchange
];

/// Yahoo Finance exchange codes for major Asian/Pacific exchanges.
const ASIA_EXCHANGES: &[&str] = &[
    "JPX", // Tokyo
    "HKG", // Hong Kong
    "SHH", // Shanghai
    "SHZ", // Shenzhen
    "KSC", // Korea
    "TAI", // Taiwan
    "NSI", // National Stock Exchange of India
    "BSE", // Bombay
    "ASX", // Australia
    "SGX", // Singapore
    "NZE", // New Zealand
];

// ─── Provider ────────────────────────────────────────────────────────────────

/// Yahoo Finance implementation of [`MarketDataProvider`].
///
/// Authenticates once on construction and reuses the crumb and cookie jar for
/// all subsequent requests.  Create a new instance if the crumb expires.
///
/// # Example
///
/// ```rust,no_run
/// use market_data::yahoo::YahooFinance;
/// use market_data::provider::MarketDataProvider;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let yf = YahooFinance::new().await?;
///     let stocks = yf.get_stocks(300).await?;
///     println!("{} stocks returned", stocks.len());
///     Ok(())
/// }
/// ```
pub struct YahooFinance {
    client: Client,
    crumb: String,
}

impl YahooFinance {
    /// Create a new [`YahooFinance`] instance, fetching a fresh crumb.
    ///
    /// # Errors
    ///
    /// Returns [`MarketDataError::Auth`] if authentication fails.
    pub async fn new() -> Result<Self, MarketDataError> {
        let auth = YahooAuth::fetch().await?;

        let client = Client::builder()
            .cookie_provider(auth.jar)
            .user_agent("Mozilla/5.0")
            .build()
            .map_err(|e| MarketDataError::Auth(e.to_string()))?;

        Ok(Self {
            client,
            crumb: auth.crumb,
        })
    }
}

#[async_trait]
impl MarketDataProvider for YahooFinance {
    /// Return the top `limit` most active equities across US, European and
    /// Asian exchanges.
    ///
    /// Fetches the three regions **concurrently**, merges the results, and
    /// returns them sorted by descending volume.
    async fn get_stocks(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError> {
        let per_region = (limit / 3).max(1);

        // Fetch all three regions concurrently
        let (us, eu, asia) = tokio::try_join!(
            fetch_by_exchanges(&self.client, &self.crumb, US_EXCHANGES, per_region),
            fetch_by_exchanges(&self.client, &self.crumb, EU_EXCHANGES, per_region),
            fetch_by_exchanges(&self.client, &self.crumb, ASIA_EXCHANGES, per_region),
        )?;

        let mut assets: Vec<Asset> =
            us.into_iter().chain(eu).chain(asia).map(Asset::from).collect();

        // Sort by volume descending
        assets.sort_by(|a, b| b.volume.unwrap_or(0).cmp(&a.volume.unwrap_or(0)));
        assets.truncate(limit);

        Ok(assets)
    }

    /// Return the top `limit` most active forex pairs.
    ///
    /// Uses the Yahoo Finance screener with `quoteType = CURRENCY`.
    async fn get_forex(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError> {
        let quotes = fetch_by_quote_type(&self.client, &self.crumb, "CURRENCY", limit).await?;
        Ok(quotes.into_iter().map(Asset::from).collect())
    }

    /// Return the top `limit` most active ETFs.
    ///
    /// Uses the Yahoo Finance screener with `quoteType = ETF`.
    async fn get_etf(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError> {
        let quotes = fetch_by_quote_type(&self.client, &self.crumb, "ETF", limit).await?;
        Ok(quotes.into_iter().map(Asset::from).collect())
    }

    /// Return the top `limit` most active cryptocurrencies.
    ///
    /// Uses the `most_actives_crypto` predefined Yahoo Finance screener.
    async fn get_crypto(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError> {
        let quotes =
            fetch_predefined(&self.client, &self.crumb, "most_actives_crypto", limit).await?;
        Ok(quotes.into_iter().map(Asset::from).collect())
    }
}
