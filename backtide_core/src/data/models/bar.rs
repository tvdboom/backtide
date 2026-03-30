//! Interval and Bar definitions.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};
use strum::{EnumString, IntoEnumIterator};

/// The time resolution of a single [`Bar`].
///
/// Variants map to the canonical durations supported across providers.
///
/// See Also
/// --------
/// - backtide.data:Asset
/// - backtide.data:AssetType
/// - backtide.data:Bar
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
    Serialize,
    Deserialize,
)]
pub enum Interval {
    OneMinute,
    TwoMinutes,
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    OneHour,
    FourHours,
    #[default]
    OneDay,
    FiveDays,
    OneWeek,
    OneMonth,
    ThreeMonths,
}

#[pymethods]
impl Interval {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("invalid Interval: {s}")))
    }

    /// Make the class pickable (required by streamlit).
    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Interval>().into_any();
        Ok((cls, (self.to_string(),)))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }

    fn __hash__(&self) -> u64 {
        *self as u64
    }

    fn __repr__(&self) -> String {
        match self {
            Interval::OneMinute => "1m".to_string(),
            Interval::TwoMinutes => "2m".to_string(),
            Interval::FiveMinutes => "5m".to_string(),
            Interval::FifteenMinutes => "15m".to_string(),
            Interval::ThirtyMinutes => "30m".to_string(),
            Interval::OneHour => "1h".to_string(),
            Interval::FourHours => "4h".to_string(),
            Interval::OneDay => "1d".to_string(),
            Interval::FiveDays => "5d".to_string(),
            Interval::OneWeek => "1w".to_string(),
            Interval::OneMonth => "1mo".to_string(),
            Interval::ThreeMonths => "3mo".to_string(),
        }
    }

    /// Return the default variant.
    #[staticmethod]
    fn get_default(py: Python<'_>) -> Py<Self> {
        Py::new(py, Self::OneDay).unwrap()
    }

    /// Return all variants.
    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }

    /// Whether the interval is smaller than one day.
    ///
    /// Returns
    /// -------
    /// bool
    ///     If interval is intraday.
    fn is_intraday(&self) -> bool {
        self.to_minutes() < Interval::OneDay.to_minutes()
    }
    
    /// Minutes in this interval.
    ///
    /// Returns
    /// -------
    /// int
    ///     Number of minutes.
    fn to_minutes(&self) -> u32 {
        match self {
            Interval::OneMinute => 1,
            Interval::TwoMinutes => 2,
            Interval::FiveMinutes => 5,
            Interval::FifteenMinutes => 15,
            Interval::ThirtyMinutes => 30,
            Interval::OneHour => 60,
            Interval::FourHours => 4 * 60,
            Interval::OneDay => 24 * 60 * 60,
            Interval::FiveDays => 5 * 24 * 60 * 60,
            Interval::OneWeek => 7 * 24 * 60 * 60,
            Interval::OneMonth => 30 * 24 * 60 * 60,
            Interval::ThreeMonths => 3 * 30 * 24 * 60 * 60,
        }
    }
}

/// A single OHLCV candle for one symbol at one interval.
///
/// Two timestamps are carried per bar:
/// - `ts_utc` — the bar's open time in UTC; use this as the join key across
///   exchanges when aligning a multi-asset universe.
/// - `ts_exchange` — the open time in the exchange's local timezone; use this
///   for session-relative filtering (e.g. "first 30 minutes of the session").
///
/// `adj_close` is always populated. For assets where price adjustment is
/// meaningless (crypto, forex) it is set equal to `close`.
///
/// Attributes
/// ----------
/// open_ts : int
///     Bar open time in UTC (Unix seconds).
///
/// open_ts_exchange : float
///     Bar open time in the exchange's local timezone (Unix seconds).
///
/// open : float
///     Price at bar open.
///
/// high : float
///     Highest price seen in the interval.
///
/// low : float
///     Lowest price seen in the interval.
///
/// close : float
///     Price at bar close.
///
/// adj_close : float
///     Split- and dividend-adjusted close. Equal to `close` when adjustment
///     is not applicable.
///
/// volume : float
///     Traded volume in the asset's native units.
///
/// See Also
/// --------
/// - backtide.data:Asset
/// - backtide.data:AssetType
/// - backtide.data:Interval
#[pyclass(from_py_object, get_all, frozen, module = "backtide.data")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bar {
    pub open_ts: i64,
    pub open_ts_exchange: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub adj_close: f64,
    pub volume: f64,
}

#[pymethods]
impl Bar {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;
}
