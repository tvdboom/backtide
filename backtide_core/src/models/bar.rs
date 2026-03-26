//! Interval and Bar definitions.

use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};
use strum::IntoEnumIterator;

/// The time resolution of a single [`Bar`].
///
/// Variants map to the canonical durations supported across providers.
///
/// See Also
/// --------
/// - backtide.models:Asset
/// - backtide.models:AssetType
/// - backtide.models:Bar
#[pyclass(from_py_object, module = "backtide.models")]
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
    Serialize,
    Deserialize,
)]
pub enum Interval {
    /// 1-minute bars.
    OneMinute,

    /// 5-minute bars.
    FiveMinutes,

    /// 15-minute bars.
    FifteenMinutes,

    /// 30-minute bars.
    ThirtyMinutes,

    /// 1-hour bars.
    OneHour,

    /// 4-hour bars.
    FourHours,

    #[default]
    /// Daily bars (calendar day, not session).
    OneDay,

    /// Weekly bars (Monday open → Friday close, UTC).
    OneWeek,
}

#[pymethods]
impl Interval {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    fn __repr__(&self) -> String {
        match self {
            Interval::OneMinute => "1m".to_string(),
            Interval::FiveMinutes => "5m".to_string(),
            Interval::FifteenMinutes => "15m".to_string(),
            Interval::ThirtyMinutes => "30m".to_string(),
            Interval::OneHour => "1h".to_string(),
            Interval::FourHours => "4h".to_string(),
            Interval::OneDay => "1d".to_string(),
            Interval::OneWeek => "1w".to_string(),
        }
    }

    /// Return all variants.
    #[staticmethod]
    fn variants() -> Vec<Self> {
        Self::iter().collect()
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
            Interval::FiveMinutes => 5,
            Interval::FifteenMinutes => 15,
            Interval::ThirtyMinutes => 30,
            Interval::OneHour => 60,
            Interval::FourHours => 4 * 60,
            Interval::OneDay => 24 * 60 * 60,
            Interval::OneWeek => 7 * 24 * 60 * 60,
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
/// See Also
/// --------
/// - backtide.models:Asset
/// - backtide.models:AssetType
/// - backtide.models:Interval
#[pyclass(from_py_object, get_all, frozen, module = "backtide.models")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bar {
    /// Bar open time in UTC (Unix seconds).
    pub open_ts: i64,

    /// Bar open time in the exchange's local timezone (Unix seconds).
    pub open_ts_exchange: i64,

    /// Price at bar open.
    pub open: f64,

    /// Highest price seen in the interval.
    pub high: f64,

    /// Lowest price seen in the interval.
    pub low: f64,

    /// Price at bar close.
    pub close: f64,

    /// Split- and dividend-adjusted close. Equal to `close` when adjustment
    /// is not applicable.
    pub adj_close: f64,

    /// Traded volume in the asset's native units.
    pub volume: f64,
}

#[pymethods]
impl Bar {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;
}
