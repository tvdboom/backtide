//! Asset and AssetType definitions.

use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};

/// The broad category an [`Asset`] belongs to.
///
/// See Also
/// --------
/// - backtide.models:Asset
/// - backtide.models:Bar
/// - backtide.models:Interval
#[pyclass(from_py_object, module = "backtide.models")]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, EnumIter, Serialize, Deserialize)]
pub enum AssetType {
    #[default]
    Stock,
    Etf,
    Forex,
    Crypto,
}

#[pymethods]
impl AssetType {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    pub fn __str__(&self) -> &'static str {
        match self {
            Self::Stock => "Stocks",
            Self::Etf => "ETF",
            Self::Forex => "Forex",
            Self::Crypto => "Crypto",
        }
    }

    /// Return all variants.
    #[staticmethod]
    fn variants() -> Vec<Self> {
        Self::iter().collect()
    }

    /// Material icon to visually represent this asset type.
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
/// Each asset is uniquely identified by a [symbol][nom-symbol] and
/// belongs to exactly one [asset type].
///
/// Attributes
/// ----------
/// symbol : str
///     Ticker symbol as used on the exchange.
///
/// name : str
///     Human-readable name of the asset.
///
/// currency : str
///     Currency the asset trades on. Quote for forex and crypto.
///
/// asset_type : [`AssetType`]
///     Asset type this asset belongs to.
///
/// volume : int or None
///     Traded volume during the most recent regular market session.
///
/// price : float or None
///     The most recent traded price during the regular market session.
///
/// See Also
/// --------
/// - backtide.models:AssetType
/// - backtide.models:Bar
/// - backtide.models:Interval
#[pyclass(skip_from_py_object, get_all, frozen, module = "backtide.models")]
#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub symbol: String,
    pub name: String,
    pub currency: String,
    pub asset_type: AssetType,
    pub volume: Option<u64>,
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
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

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
