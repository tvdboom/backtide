use crate::data::models::asset_type::AssetType;
use crate::data::models::bar::Bar;
use crate::data::models::interval::Interval;
use crate::data::providers::provider::Provider;

/// One batch of bars sharing the same keys.
///
/// Used by `write_bars_bulk` to write many groups in a single database transaction.
pub struct BarsGroup {
    /// Canonical ticker symbol (e.g. `"AAPL"`, `"BTC-USD"`).
    pub symbol: String,

    /// The asset class this group belongs to (stocks, crypto, forex, …).
    pub asset_type: AssetType,

    /// Bar frequency / time-frame (e.g. `1m`, `1h`, `1d`).
    pub interval: Interval,

    /// Data provider that sourced the bars (e.g. Yahoo, Binance).
    pub provider: Provider,

    /// OHLCV bars to persist. May be empty, in which case the group is skipped.
    pub bars: Vec<Bar>,
}
