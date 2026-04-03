use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

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
/// close_ts : int
///     Bar close time in UTC (Unix seconds).
///
/// open_ts_exchange : int
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
/// n_trades: int | None
///     Number of trades that occurred this bar.
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
    pub close_ts: i64,
    pub open_ts_exchange: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub adj_close: f64,
    pub volume: f64,
    pub n_trades: Option<i32>,
}

#[pymethods]
impl Bar {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;
}
