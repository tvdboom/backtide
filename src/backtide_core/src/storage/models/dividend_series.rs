use crate::constants::Symbol;
use crate::data::models::{Dividend, Provider};

/// One batch of dividends sharing the same keys.
pub struct DividendSeries {
    /// Canonical ticker symbol.
    pub symbol: Symbol,

    /// Data provider that sourced the dividends.
    pub provider: Provider,

    /// Dividend events to persist. May be empty, in which case the series is skipped.
    pub dividends: Vec<Dividend>,
}
