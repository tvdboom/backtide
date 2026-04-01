use crate::constants::Symbol;
use crate::data::models::asset_type::AssetType;
use crate::data::models::currency::Currency;
use crate::data::models::exchange::Exchange;
use pyo3::prelude::*;
use pyo3::types::PyString;
use serde::Deserialize;

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
/// base : str | [`Currency`] | None
///     The currency of the tradeable asset. Only defined for forex and
///     crypto pairs.
///
/// quote : str | [`Currency`]
///     The currency the asset trades on.
///
/// asset_type : [`AssetType`]
///     Asset type this asset belongs to.
///
/// exchange : str | [`Exchange`]
///     The exchange this asset is listed in.
///
/// exchange_name : str
///     Human-readable exchange name.
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
    pub exchange: String,
    #[pyo3(get)]
    pub earliest_ts: Option<u64>,
    #[pyo3(get)]
    pub latest_ts: Option<u64>,

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
        exchange: String,
        earliest_ts: Option<u64>,
        latest_ts: Option<u64>,
    ) -> Self {
        Self {
            symbol,
            name,
            base,
            quote,
            asset_type,
            exchange,
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
        (Symbol, String, Option<String>, String, AssetType, String, Option<u64>, Option<u64>),
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
                self.exchange.clone(),
                self.earliest_ts,
                self.latest_ts,
            ),
        ))
    }

    fn __repr__(&self) -> String {
        format!(
            "Asset(symbol={:?}, name={:?}, base={}, quote={:?}, asset_type={:?}, exchange={:?}, earliest_ts={}, latest_ts={})",
            self.symbol,
            self.name,
            self.base.as_deref().map_or("None".to_owned(), |s| format!("{s:?}")),
            self.quote,
            self.asset_type.to_string(),
            self.exchange,
            self.earliest_ts.map_or("None".to_owned(), |s| format!("{s:?}")),
            self.latest_ts.map_or("None".to_owned(), |s| format!("{s:?}")),
        )
    }

    #[getter]
    fn base(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        match &self.base {
            None => Ok(None),
            Some(s) => Ok(Some(match s.parse::<Currency>() {
                Ok(c) => Py::new(py, c)?.into_any(),
                Err(_) => PyString::new(py, s).unbind().into_any(),
            })),
        }
    }

    #[getter]
    fn quote(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(match self.quote.parse::<Currency>() {
            Ok(c) => Py::new(py, c)?.into_any(),
            Err(_) => PyString::new(py, &self.quote).unbind().into_any(),
        })
    }

    #[getter]
    fn exchange(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(match self.exchange.parse::<Exchange>() {
            Ok(c) => Py::new(py, c)?.into_any(),
            Err(_) => PyString::new(py, &self.exchange).unbind().into_any(),
        })
    }
}
