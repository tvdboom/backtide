//! Asset and AssetType definitions.

use pyo3::prelude::*;
use pyo3::types::PyType;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};

/// The broad category an [Asset] belongs to.
#[pyclass(from_py_object)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, EnumIter, Serialize, Deserialize)]
pub enum AssetType {
    /// Individual equity shares.
    #[default]
    Stock,

    /// Exchange-traded funds.
    Etf,

    /// Spot foreign-exchange pairs.
    Forex,

    /// Cryptocurrency spot pairs.
    Crypto,
}

#[pymethods]
impl AssetType {
    pub fn __str__(&self) -> &'static str {
        match self {
            Self::Stock => "Stocks",
            Self::Etf => "ETF",
            Self::Forex => "Forex",
            Self::Crypto => "Crypto",
        }
    }

    /// All known variants as their canonical string values.
    #[classmethod]
    pub fn names(_cls: &Bound<'_, PyType>) -> Vec<&'static str> {
        Self::iter().map(|x| x.__str__()).collect()
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Stock => ":material/candlestick_chart:",
            Self::Etf => ":material/account_balance:",
            Self::Forex => ":material/currency_exchange:",
            Self::Crypto => ":material/currency_bitcoin:",
        }
    }
}

/// A tradeable financial instrument.
///
/// Each asset is uniquely identified by a [symbol] and belongs
/// to exactly one [asset type].
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

    /// Asset type this asset belongs to.
    #[pyo3(get)]
    pub asset_type: AssetType,

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
    fn new(symbol: String, name: String, currency: String, asset_type: AssetType) -> Self {
        Self {
            symbol,
            name,
            currency,
            asset_type,
            volume: None,
            price: None,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Asset(symbol={:?}, name={:?}, currency={:?}, asset_type={:?})",
            self.symbol, self.name, self.currency, self.asset_type
        )
    }
}
