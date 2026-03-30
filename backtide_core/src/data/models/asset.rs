//! Asset and AssetType definitions.

use crate::data::provider::provider::Provider;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde::Deserialize;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// Canonical (provider-independent) symbol name.
pub type Symbol = String;

/// The broad category an [`Asset`] belongs to.
///
/// See Also
/// --------
/// - backtide.data:Asset
/// - backtide.data:Bar
/// - backtide.data:Interval
#[pyclass(from_py_object, module = "backtide.data")]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    Display,
    EnumIter,
    EnumString,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum AssetType {
    #[default]
    Stocks,
    Etf,
    Forex,
    Crypto,
}

impl AssetType {
    pub fn default(&self) -> Provider {
        match self {
            Self::Stocks => Provider::Yahoo,
            Self::Etf => Provider::Yahoo,
            Self::Forex => Provider::Yahoo,
            Self::Crypto => Provider::Binance,
        }
    }
}

#[pymethods]
impl AssetType {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("invalid AssetType: {s}")))
    }

    /// Make the class pickable (required by streamlit).
    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<AssetType>().into_any();
        Ok((cls, (self.to_string(),)))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }

    fn __hash__(&self) -> u64 {
        *self as u64
    }

    pub fn __str__(&self) -> &'static str {
        match self {
            Self::Stocks => "Stocks",
            Self::Etf => "ETF",
            Self::Forex => "Forex",
            Self::Crypto => "Crypto",
        }
    }

    /// Return the default variant.
    #[staticmethod]
    fn get_default(py: Python<'_>) -> Py<Self> {
        Py::new(py, Self::Stocks).unwrap()
    }

    /// Return all variants.
    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }

    /// Material icon to visually represent this asset type.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Stocks => ":material/candlestick_chart:",
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
/// base : str | None
///     The currency of the tradeable asset. Only defined for forex and
///     crypto pairs.
///
/// quote : str
///     The currency the asset trades on.
///
/// asset_type : [`AssetType`]
///     Asset type this asset belongs to.
///
/// earliest_ts : int | None
///     Earliest timestamp for which there is data in UNIX timestamp.
///
/// latest_ts : int | None
///     Most recent timestamp for which there is data in UNIX timestamp.
///
/// See Also
/// --------
/// - backtide.data:AssetType
/// - backtide.data:Bar
/// - backtide.data:Interval
#[pyclass(skip_from_py_object, frozen, module = "backtide.data")]
#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    #[pyo3(get)]
    pub symbol: Symbol,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub base: Option<String>,
    #[pyo3(get)]
    pub quote: String,
    #[pyo3(get)]
    pub asset_type: AssetType,
    #[pyo3(get)]
    pub earliest_ts: Option<i64>,
    #[pyo3(get)]
    pub latest_ts: Option<i64>,

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
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    fn new(
        symbol: Symbol,
        name: String,
        base: Option<String>,
        quote: String,
        asset_type: AssetType,
        earliest_ts: Option<i64>,
        latest_ts: Option<i64>,
    ) -> Self {
        Self {
            symbol,
            name,
            base,
            quote,
            asset_type,
            earliest_ts,
            latest_ts,
            volume: None,
            price: None,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(
        Bound<'py, PyAny>,
        (Symbol, String, Option<String>, String, AssetType, Option<i64>, Option<i64>),
    )> {
        let cls = py.get_type::<Asset>().into_any();
        Ok((
            cls,
            (
                self.symbol.clone(),
                self.name.clone(),
                self.base.clone(),
                self.quote.clone(),
                self.asset_type,
                self.earliest_ts,
                self.latest_ts,
            ),
        ))
    }

    fn __repr__(&self) -> String {
        format!(
            "Asset(symbol={:?}, name={:?}, base={:?}, quote={:?}, asset_type={:?}, earliest_ts={:?}, latest_ts={:?})",
            self.symbol, self.name, self.base, self.quote, self.asset_type, self.earliest_ts, self.latest_ts
        )
    }
}
