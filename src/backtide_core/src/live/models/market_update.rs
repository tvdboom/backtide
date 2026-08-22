//! Provider-normalized live market updates.

use crate::data::models::Bar;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

/// A candle received from a live market-data connection.
///
/// `is_final` is `true` only when the provider has closed the candle, or when
/// Backtide observed the next candle and can therefore finalize the prior one.
///
/// Attributes
/// ----------
/// provider : str
///     Lowercase provider identifier, or `"mock"` for replay data.
///
/// symbol : str
///     Canonical provider-independent symbol.
///
/// quote_currency : str | None
///     Currency in which OHLC prices are denominated. Live providers populate
///     this value; `None` preserves base-currency accounting for synthetic and
///     backwards-compatible manually constructed updates.
///
/// interval : str
///     Canonical interval string.
///
/// open_ts : int
///     Candle-open Unix timestamp in seconds.
///
/// close_ts : int
///     Candle-close Unix timestamp in seconds.
///
/// open : float
///     Opening price in quote-currency units.
///
/// high : float
///     Highest price in quote-currency units.
///
/// low : float
///     Lowest price in quote-currency units.
///
/// close : float
///     Latest or final closing price in quote-currency units.
///
/// volume : float
///     Traded volume in base-asset units.
///
/// n_trades : int | None
///     Provider-reported trade count when available.
///
/// is_final : bool
///     Whether no further updates are expected for this candle.
///
/// received_ts : int
///     Local receipt Unix timestamp in seconds.
///
/// See Also
/// --------
/// - backtide.live:PaperTradingSession
///
/// Examples
/// --------
/// ```pycon
/// from backtide.live import MarketUpdate
///
/// market = MarketUpdate(
///     symbol="BTC-USD",
///     interval="1m",
///     open_ts=1_700_000_000,
///     close_ts=1_700_000_060,
///     open=100.0,
///     high=102.0,
///     low=99.0,
///     close=101.0,
///     volume=5.0,
/// )
/// print(market.close)
/// ```
#[pyclass(get_all, frozen, from_py_object, module = "backtide.live")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketUpdate {
    /// Lowercase provider identifier, or `"mock"` for replay data.
    pub provider: String,

    /// Canonical provider-independent symbol (for example, `"BTC-USD"`).
    pub symbol: String,

    /// Currency in which OHLC prices are denominated.
    pub quote_currency: Option<String>,

    /// Canonical interval string (for example, `"1m"`).
    pub interval: String,

    /// Candle-open Unix timestamp in seconds.
    pub open_ts: u64,

    /// Candle-close Unix timestamp in seconds.
    pub close_ts: u64,

    /// Opening price in quote-currency units.
    pub open: f64,

    /// Highest price in quote-currency units.
    pub high: f64,

    /// Lowest price in quote-currency units.
    pub low: f64,

    /// Latest or final closing price in quote-currency units.
    pub close: f64,

    /// Traded volume in base-asset units.
    pub volume: f64,

    /// Provider-reported trade count when available.
    pub n_trades: Option<i32>,

    /// Whether no further updates are expected for this candle.
    pub is_final: bool,

    /// Local receipt Unix timestamp in seconds.
    pub received_ts: i64,
}

impl MarketUpdate {
    /// Whether the update contains a usable positive OHLC candle.
    pub fn is_valid_bar(&self) -> bool {
        self.close_ts > self.open_ts
            && [self.open, self.high, self.low, self.close]
                .iter()
                .all(|price| price.is_finite() && *price > 0.0)
            && self.high >= self.open.max(self.close)
            && self.low <= self.open.min(self.close)
            && self.high >= self.low
            && self.volume.is_finite()
            && self.volume >= 0.0
    }

    /// Convert the transport model to the engine's canonical bar model.
    pub fn bar(&self) -> Bar {
        Bar {
            open_ts: self.open_ts,
            close_ts: self.close_ts,
            open_ts_exchange: self.open_ts,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            adj_close: self.close,
            volume: self.volume,
            n_trades: self.n_trades,
        }
    }
}

#[pymethods]
impl MarketUpdate {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        symbol,
        interval,
        open_ts,
        close_ts,
        open,
        high,
        low,
        close,
        volume=0.0,
        n_trades=None,
        is_final=true,
        provider: "str"="mock",
        received_ts=0,
        quote_currency=None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        symbol: String,
        interval: String,
        open_ts: u64,
        close_ts: u64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        n_trades: Option<i32>,
        is_final: bool,
        provider: &str,
        received_ts: i64,
        quote_currency: Option<String>,
    ) -> Self {
        Self {
            provider: provider.to_owned(),
            symbol,
            quote_currency,
            interval,
            open_ts,
            close_ts,
            open,
            high,
            low,
            close,
            volume,
            n_trades,
            is_final,
            received_ts,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "MarketUpdate(provider={:?}, symbol={:?}, interval={:?}, close={}, is_final={})",
            self.provider, self.symbol, self.interval, self.close, self.is_final,
        )
    }
}
