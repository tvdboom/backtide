//! Trait that storage solutions must implement.

use crate::data::models::bar::Bar;
use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;
use crate::storage::errors::StorageResult;
use async_trait::async_trait;

/// Abstraction over a storage solution.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Initialize all tables in the database.
    fn init(&self) -> StorageResult<()>;

    /// Store OHLC data.
    fn write_bars(
        &self,
        symbol: &str,
        provider: Provider,
        interval: Interval,
        bars: &[Bar],
    ) -> StorageResult<()>;

    // /// Load OHLC data.
    // fn load_bars(
    //     &self,
    //     symbol: &str,
    //     start: Option<DateTime<Utc>>,
    //     end: Option<DateTime<Utc>>,
    // ) -> StorageResult<Vec<Bar>>;
    //
    // /// Delete bars from the database.
    // fn drop_bars(&self);
}
