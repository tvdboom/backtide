//! Trait that all market data providers must implement.
//!
//! Adding a new provider (e.g. Bloomberg, Alpaca) only requires implementing
//! this trait — no changes to the Python bindings or callers are needed.

use crate::data::asset::Asset;
use crate::data::utils::MarketDataError;
use async_trait::async_trait;

/// A source of financial market data.
///
/// All methods are async and must be safe to call concurrently from a
/// multithreaded Tokio runtime.
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    /// Return the top `limit` most active equities across US, European and
    /// Asian exchanges.
    async fn list_stocks(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError>;

    /// Return the top `limit` most active forex pairs.
    async fn list_forex(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError>;

    /// Return the top `limit` most active ETFs.
    async fn list_etf(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError>;

    /// Return the top `limit` most active cryptocurrencies.
    async fn list_crypto(&self, limit: usize) -> Result<Vec<Asset>, MarketDataError>;
}
