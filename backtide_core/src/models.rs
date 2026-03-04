//! Data models shared across all market data providers.

use pyo3::prelude::*;
use serde::Deserialize;

/// A single financial asset returned by a market data provider.
///
/// All fields that Yahoo Finance may omit (e.g. `market_cap` for crypto) are
/// `Option` so callers can handle missing data gracefully.
#[pyclass(from_py_object)]
#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    /// Ticker symbol as used on the exchange (e.g. `"ASML.AS"`, `"BTC-USD"`).
    #[pyo3(get)]
    pub symbol: String,

    /// Human-readable name of the asset (e.g. `"ASML Holding N.V."`).
    #[pyo3(get)]
    pub name: String,

    /// Latest market price in the asset's native currency.
    #[pyo3(get)]
    pub price: Option<f64>,

    /// Absolute price change since the previous close.
    #[pyo3(get)]
    pub change: Option<f64>,

    /// Percentage price change since the previous close.
    #[pyo3(get)]
    pub change_pct: Option<f64>,

    /// Trading volume for the current session.
    #[pyo3(get)]
    pub volume: Option<u64>,

    /// Market capitalisation in USD.
    #[pyo3(get)]
    pub market_cap: Option<f64>,

    /// Exchange the asset trades on (e.g. `"AMS"`, `"NMS"`).
    #[pyo3(get)]
    pub exchange: Option<String>,
}

#[pymethods]
impl Asset {
    fn __repr__(&self) -> String {
        format!("Asset(symbol={:?}, name={:?}, price={:?})", self.symbol, self.name, self.price)
    }
}

/// Raw quote shape returned by the Yahoo Finance screener endpoint.
/// Fields are `Option` because Yahoo omits them inconsistently.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YahooQuote {
    pub symbol: String,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub regular_market_price: Option<f64>,
    pub regular_market_change: Option<f64>,
    pub regular_market_change_percent: Option<f64>,
    pub regular_market_volume: Option<u64>,
    pub market_cap: Option<f64>,
    pub full_exchange_name: Option<String>,
}

impl From<YahooQuote> for Asset {
    fn from(q: YahooQuote) -> Self {
        let name = q.short_name.or(q.long_name).unwrap_or_else(|| q.symbol.clone());

        Self {
            symbol: q.symbol,
            name,
            price: q.regular_market_price,
            change: q.regular_market_change,
            change_pct: q.regular_market_change_percent,
            volume: q.regular_market_volume,
            market_cap: q.market_cap,
            exchange: q.full_exchange_name,
        }
    }
}
