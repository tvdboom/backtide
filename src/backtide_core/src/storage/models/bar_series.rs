use crate::data::models::{Bar, Interval, Provider};

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
