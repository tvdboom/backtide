//! Data models shared across all market data providers.

use pyo3::prelude::*;
use serde::Deserialize;

/// A single financial asset returned by a market data provider.
///
/// All fields that Yahoo Finance may omit (e.g. `market_cap` for crypto) are
/// `Option` so callers can handle missing data gracefully.
#[pyclass(skip_from_py_object)]
#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    /// Ticker symbol as used on the exchange.
    #[pyo3(get)]
    pub symbol: String,

    /// Human-readable name of the asset.
    #[pyo3(get)]
    pub name: String,

    /// Currency the asset trades on. Quote for forex and crypto.
    #[pyo3(get)]
    pub currency: String,

    /// Traded volume during the most recent regular market session.
    pub volume: Option<u64>,

    /// The most recent traded price during the regular market session.
    pub price: Option<f64>,
}

impl Asset {
    pub fn volume_price(&self) -> f64 {
        match (self.volume, self.price) {
            (Some(v), Some(p)) => v as f64 * p,
            _ => 0.,
        }
    }
}

#[pymethods]
impl Asset {
    #[new]
    fn new(symbol: String, name: String, currency: String) -> Self {
        Self {
            symbol,
            name,
            currency,
            volume: None,
            price: None,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Asset(symbol={:?}, name={:?}, currency={:?})",
            self.symbol, self.name, self.currency
        )
    }
}
