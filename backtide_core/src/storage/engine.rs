//! Implementation of storage related methods for [`Engine`].

use crate::data::models::bar::Bar;
use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;
use crate::engine::Engine;
use crate::storage::errors::StorageResult;

impl Engine {
    pub fn write_bars(
        &self,
        symbol: &str,
        provider: Provider,
        interval: Interval,
        bars: &[Bar],
    ) -> StorageResult<()> {
        self.db.write_bars(symbol, provider, interval, bars)
    }
}
