use crate::data::models::bar::Bar;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;

/// One batch of bars sharing the same keys.
///
/// Used by `write_bars_bulk` to write many series in a single database transaction.
pub struct BarSeries {
    /// Canonical ticker symbol (e.g. `"AAPL"`, `"BTC-USD"`).
    pub symbol: String,

    /// The instrument class this series belongs to (stocks, crypto, forex, …).
    pub instrument_type: InstrumentType,

    /// Bar frequency / time-frame (e.g. `1m`, `1h`, `1d`).
    pub interval: Interval,

    /// Data provider that sourced the bars (e.g. Yahoo, Binance).
    pub provider: Provider,

    /// OHLCV bars to persist. May be empty, in which case the series is skipped.
    pub bars: Vec<Bar>,
}
