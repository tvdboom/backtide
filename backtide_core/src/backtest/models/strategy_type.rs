use pyo3::exceptions::PyValueError;
use pyo3::{pyclass, pymethods, Borrowed, Bound, FromPyObject, Py, PyAny, PyErr, PyResult, Python};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// A predefined (built-in) strategy type.
///
/// Each variant represents a complete trading strategy shipped with
/// backtide. Predefined strategies can be selected alongside custom
/// user-defined strategies for performance comparison.
///
/// Attributes
/// ----------
/// name : str
///     The human-readable display name of the strategy.
///
/// is_rotation : bool
///     Whether this is a portfolio rotation (multi-asset) strategy.
///
/// See Also
/// --------
/// - backtide.backtest:IndicatorType
/// - backtide.backtest:OrderType
/// - backtide.backtest:StrategyConfig
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
pub enum StrategyType {
    #[default]
    BuyAndHold,
    SmaNaive,
    SmaCrossover,
    Macd,
    BollingerBands,
    Momentum,
    Rsi,
    Rsrs,
    Roc,
    DoubleTop,
    RiskAverse,
    TurtleTrading,
    Vcp,
    AlphaRsiPro,
    AdaptiveRsi,
    HybridAlphaRsi,
    RocRotation,
    RsrsRotation,
    TripleRsiRotation,
    MultiBbRotation,
}

#[pymethods]
impl StrategyType {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    #[new]
    pub fn new(s: &str) -> PyResult<Self> {
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown strategy type: {s}")))
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

    /// The human-readable display name of the strategy.
    #[getter]
    pub fn name(&self) -> &'static str {
        match self {
            Self::BuyAndHold => "Buy & Hold",
            Self::SmaNaive => "SMA (Naive)",
            Self::SmaCrossover => "SMA (Crossover)",
            Self::Macd => "MACD",
            Self::BollingerBands => "Bollinger Bands",
            Self::Momentum => "Momentum",
            Self::Rsi => "RSI",
            Self::Rsrs => "RSRS",
            Self::Roc => "ROC",
            Self::DoubleTop => "Double Top",
            Self::RiskAverse => "Risk Averse",
            Self::TurtleTrading => "Turtle Trading",
            Self::Vcp => "VCP",
            Self::AlphaRsiPro => "AlphaRSI Pro",
            Self::AdaptiveRsi => "Adaptive RSI",
            Self::HybridAlphaRsi => "Hybrid AlphaRSI",
            Self::RocRotation => "ROC Rotation",
            Self::RsrsRotation => "RSRS Rotation",
            Self::TripleRsiRotation => "Triple RSI Rotation",
            Self::MultiBbRotation => "Multi BB Rotation",
        }
    }

    /// Whether this is a portfolio rotation strategy (multi-asset).
    #[getter]
    pub fn is_rotation(&self) -> bool {
        matches!(
            self,
            Self::RocRotation
                | Self::RsrsRotation
                | Self::TripleRsiRotation
                | Self::MultiBbRotation
        )
    }

    /// Return the description of the strategy.
    ///
    /// Returns
    /// -------
    /// str
    ///     A human-readable summary of the strategy's logic.
    pub fn description(&self) -> &'static str {
        match self {
            Self::BuyAndHold => "Buys on the first day and holds to the end. A baseline for performance comparison.",
            Self::SmaNaive => "Buys when price is above a moving average, sells when below.",
            Self::SmaCrossover => "Buys on a golden cross (fast MA over slow MA), sells on a death cross.",
            Self::Macd => "Buys on a MACD golden cross and sells on a death cross.",
            Self::BollingerBands => "A mean-reversion strategy that buys at the lower band and sells at the upper band.",
            Self::Momentum => "Buys when momentum turns positive, sells when price falls below a trend-filtering MA.",
            Self::Rsi => "Combines RSI and Bollinger Bands. Buys when RSI is oversold and price is below the lower band.",
            Self::Rsrs => "Uses linear regression of high/low prices to buy on signals of strengthening support.",
            Self::Roc => "A simple momentum strategy that buys on a high Rate of Change and sells on a low one.",
            Self::DoubleTop => "Buys on a breakout after a double top pattern, with trend and volume confirmation.",
            Self::RiskAverse => "Buys low-volatility stocks making new highs on high volume.",
            Self::TurtleTrading => "A classic trend-following strategy that buys on breakouts and sells on breakdowns, using ATR for position sizing.",
            Self::Vcp => "Buys on breakouts after price and volume volatility have contracted (Volatility Contraction Pattern).",
            Self::AlphaRsiPro => "Advanced RSI with adaptive overbought/oversold levels based on volatility and trend bias filtering.",
            Self::AdaptiveRsi => "RSI with dynamic period (8-28) that adapts to market volatility and cycles.",
            Self::HybridAlphaRsi => "Most sophisticated RSI variant combining adaptive period, adaptive levels, and trend confirmation.",
            Self::RocRotation => "Periodically rotates into the top K stocks with the highest Rate of Change (momentum).",
            Self::RsrsRotation => "Periodically rotates into stocks with high RSRS indicator values (strong support).",
            Self::TripleRsiRotation => "Rotates stocks based on a combination of long, medium, and short-term RSI signals.",
            Self::MultiBbRotation => "A breakout rotation strategy that buys stocks crossing above their upper Bollinger Band.",
        }
    }

    #[staticmethod]
    fn get_default(py: Python<'_>) -> Py<Self> {
        Py::new(py, Self::BuyAndHold).unwrap()
    }

    #[staticmethod]
    fn variants(py: Python<'_>) -> Vec<Py<Self>> {
        Self::iter().map(|v| Py::new(py, v).unwrap()).collect()
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for StrategyType {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<StrategyType>() {
            return Ok(*bound.borrow());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown strategy type {s:?}.")))
    }
}
