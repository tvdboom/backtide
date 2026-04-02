use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Bound, Py, PyAny, PyResult, Python};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

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
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    OneHour,
    FourHours,
    #[default]
    OneDay,
    OneWeek,
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
            Interval::FiveMinutes => "5m".to_string(),
            Interval::FifteenMinutes => "15m".to_string(),
            Interval::ThirtyMinutes => "30m".to_string(),
            Interval::OneHour => "1h".to_string(),
            Interval::FourHours => "4h".to_string(),
            Interval::OneDay => "1d".to_string(),
            Interval::OneWeek => "1w".to_string(),
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
        self.minutes() < Interval::OneDay.minutes()
    }

    /// Minutes in this interval.
    ///
    /// Returns
    /// -------
    /// int
    ///     Number of minutes.
    fn minutes(&self) -> u32 {
        match self {
            Interval::OneMinute => 1,
            Interval::FiveMinutes => 5,
            Interval::FifteenMinutes => 15,
            Interval::ThirtyMinutes => 30,
            Interval::OneHour => 60,
            Interval::FourHours => 4 * 60,
            Interval::OneDay => 24 * 60,
            Interval::OneWeek => 7 * 24 * 60,
        }
    }
}
