use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// Built-in technical indicator type.
///
/// Indicators are mathematical functions applied to price and volume
/// data that quantify trends, momentum, volatility and other market
/// characteristics.
///
/// Attributes
/// ----------
/// name : str
///     The human-readable name of the indicator.
///
/// See Also
/// --------
/// - backtide.data:Bar
/// - backtide.backtest:IndicatorExpConfig
/// - backtide.backtest:StrategyType
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
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
#[strum(ascii_case_insensitive)]
#[allow(clippy::upper_case_acronyms)]
pub enum IndicatorType {
    #[default]
    SMA,
    EMA,
    WMA,
    RSI,
    MACD,
    BB,
    ATR,
    OBV,
    VWAP,
    STOCH,
    CCI,
    ADX,
}

#[pymethods]
impl IndicatorType {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown indicator type: {s}")))
    }

    pub fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.to_string(),)))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }

    fn __hash__(&self) -> u64 {
        *self as u64
    }

    pub fn __str__(&self) -> String {
        self.to_string()
    }

    /// The human-readable name of the indicator.
    #[getter]
    pub fn name(&self) -> &'static str {
        match self {
            Self::SMA => "Simple Moving Average",
            Self::EMA => "Exponential Moving Average",
            Self::WMA => "Weighted Moving Average",
            Self::RSI => "Relative Strength Index",
            Self::MACD => "Moving Avg. Convergence Divergence",
            Self::BB => "Bollinger Bands",
            Self::ATR => "Average True Range",
            Self::OBV => "On-Balance Volume",
            Self::VWAP => "Volume-Weighted Average Price",
            Self::STOCH => "Stochastic Oscillator",
            Self::CCI => "Commodity Channel Index",
            Self::ADX => "Average Directional Index",
        }
    }

    /// Return a description of the indicator.
    ///
    /// Returns
    /// -------
    /// str
    ///     A brief explanation of what the indicator measures.
    pub fn description(&self) -> &'static str {
        match self {
            Self::SMA => "Arithmetic mean of the last N closing prices, used to smooth short-term fluctuations and identify trends.",
            Self::EMA => "Weighted moving average that gives more weight to recent prices, reacting faster to price changes than SMA.",
            Self::WMA => "Moving average where each price is multiplied by a linearly decreasing weight, emphasizing recent data.",
            Self::RSI => "Momentum oscillator (0–100) measuring the speed and magnitude of recent price changes to identify overbought/oversold conditions.",
            Self::MACD => "Trend-following momentum indicator showing the relationship between two exponential moving averages of closing prices.",
            Self::BB => "Volatility bands placed above and below a moving average, widening during high volatility and narrowing during low volatility.",
            Self::ATR => "Measures market volatility by calculating the average range between high and low prices over a period.",
            Self::OBV => "Cumulative volume indicator that adds volume on up days and subtracts it on down days to confirm price trends.",
            Self::VWAP => "Average price weighted by volume, used as a benchmark for intraday trading quality.",
            Self::STOCH => "Compares a closing price to a range of prices over a period, generating overbought/oversold signals.",
            Self::CCI => "Measures a price's deviation from its statistical mean, identifying cyclical trends in the data.",
            Self::ADX => "Quantifies trend strength (0–100) regardless of direction, helping distinguish trending from ranging markets.",
        }
    }

    #[staticmethod]
    fn get_default(py: Python<'_>) -> Py<Self> {
        Py::new(py, Self::SMA).unwrap()
    }

    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for IndicatorType {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<IndicatorType>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown indicator type {s:?}.")))
    }
}
