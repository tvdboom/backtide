use crate::data::models::bar::Bar;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;

/// One batch of bars sharing the same keys.
pub struct BarSeries {
    /// Canonical ticker symbol.
    pub symbol: String,

    /// Bar frequency / time-frame.
    pub interval: Interval,

    /// Data provider that sourced the bars.
    pub provider: Provider,

    /// OHLCV bars to persist.
    pub bars: Vec<Bar>,
}
