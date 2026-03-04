//! Trait that all market data providers must implement.
//!
//! Adding a new provider (e.g. Bloomberg, Alpaca) only requires implementing
//! this trait — no changes to the Python bindings or callers are needed.

use async_trait::async_trait;

use crate::error::MarketDataError;
use crate::models::Asset;

/// A source of financial market data.
///
/// All methods are async and must be safe to call concurrently from a
/// multi-threaded Tokio runtime.
///
/// # Example
///
/// ```rust,no_run
/// use market_data::provider::MarketDataProvider;
/// use market_data::yahoo::YahooFinance;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let provider = YahooFinance::new()?;
///     let stocks = provider.get_stocks(100).await?;
///     println!("Got {} stocks", stocks.len());
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    /// Return the top `limit` most active equities across US, European and
    /// Asian exchanges, sorted by descending volume.
    async fn get_stocks(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError>;

    /// Return the top `limit` most active forex pairs sorted by volume.
    async fn get_forex(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError>;

    /// Return the top `limit` most active ETFs sorted by volume.
    async fn get_etf(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError>;

    /// Return the top `limit` most active cryptocurrencies sorted by volume.
    async fn get_crypto(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError>;
}
