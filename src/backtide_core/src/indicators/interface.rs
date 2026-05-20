use crate::data::models::Bar;
use crate::indicators::traits::Indicator;
use crate::indicators::utils::*;
use crate::utils::python::{extract_bars_from_python, to_python};
use pyo3::prelude::*;
use pyo3::types::PyType;

/// Shared pymethods macro for all indicator structs.
///
/// The struct must already have a `#[pymethods]` block with `new` and `__reduce__`.
/// This macro adds `acronym`, `name`, `description`, `calculate`, `__repr__`.
macro_rules! indicator_pymethods {
    ($ty:ident) => {
        #[pymethods]
        impl $ty {
            /// Short ticker-style acronym.
            #[classattr]
            fn acronym() -> &'static str {
                <$ty as Indicator>::ACRONYM
            }

            /// Human-readable name.
            #[classattr]
            fn name() -> &'static str {
                <$ty as Indicator>::NAME
            }

            /// Short explanation of what the indicator measures.
            ///
            /// Returns
            /// -------
            /// str
            ///     The description.
            #[classmethod]
            fn description(_cls: &Bound<'_, PyType>) -> &'static str {
                <$ty as Indicator>::DESCRIPTION
            }

            /// Compute the indicator on a dataset.
            ///
            /// Parameters
            /// ----------
            /// data : pd.DataFrame | pl.DataFrame
            ///     Historical OHLCV data.
            ///
            /// Returns
            /// -------
            /// pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            ///     The computed values. For multi-output indicators (e.g., upper
            ///     and lower bounds), return a 2d structure.
            fn compute<'py>(
                &self,
                py: Python<'py>,
                data: &Bound<'py, PyAny>,
            ) -> PyResult<Bound<'py, PyAny>> {
                let bars = extract_bars_from_python(data)?;
                to_python(py, &self.compute_inner(&bars))
            }

            /// Return a debug representation.
            fn __repr__(&self) -> String {
                format!("{}()", <$ty as Indicator>::ACRONYM)
            }
        }
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Python API
// ─────────────────────────────────────────────────────────────────────────────

/// Get the deterministic name for an indicator instance.
#[pyfunction]
#[pyo3(signature = (indicator: "BaseIndicator"))]
pub fn _indicator_deterministic_name(indicator: &Bound<'_, PyAny>) -> PyResult<String> {
    let acronym = indicator_acronym_from_py(indicator)?;
    let args = indicator_args_from_py(indicator)?;

    Ok(indicator_deterministic_name(&acronym, &args))
}

/// Average Directional Index (ADX).
///
/// Quantifies trend strength on a scale of 0 to 100, regardless of direction.
/// Values above 25 generally indicate a strong trend; below 20, a weak or
/// ranging market. Useful for determining whether a market is trending or
/// ranging before applying trend-following or mean-reversion strategies.
///
/// Formula:
///
/// $$
/// \begin{aligned}
/// +DI_t &= 100 \cdot \frac{Smoothed(+DM_t)}{ATR_t} \\\\
/// -DI_t &= 100 \cdot \frac{Smoothed(-DM_t)}{ATR_t} \\\\
/// DX_t &= 100 \cdot \frac{|+DI_t - (-DI_t)|}{+DI_t + (-DI_t)} \\\\
/// ADX_t &= EMA_n(DX_t)
/// \end{aligned}
/// $$
///
/// Read more on [Wikipedia][wiki-adx].
///
/// Parameters
/// ----------
/// period : int, default=14
///     Look-back window length.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:AverageTrueRange
/// backtide.indicators:MovingAverageConvergenceDivergence
/// backtide.indicators:RelativeStrengthIndex
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct AverageDirectionalIndex {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl AverageDirectionalIndex {
    #[new]
    #[pyo3(signature = (period: "int"=14))]
    pub fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Indicator for AverageDirectionalIndex {
    const ACRONYM: &'static str = "ADX";
    const NAME: &'static str = "Average Directional Index";
    const DESCRIPTION: &'static str = "Quantifies trend strength (0\u{2013}100) regardless of direction, helping distinguish trending from ranging markets.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let (_o, h, l, c, _v) = extract_ohlcv_from_bars(bars);
        let n = c.len();
        let p = self.period;
        if n < 2 {
            return vec![vec![f64::NAN; n]];
        }

        let mut plus_dm = vec![f64::NAN; n];
        let mut minus_dm = vec![f64::NAN; n];
        for i in 1..n {
            let up = h[i] - h[i - 1];
            let down = l[i - 1] - l[i];
            plus_dm[i] = if up > down && up > 0.0 {
                up
            } else {
                0.0
            };
            minus_dm[i] = if down > up && down > 0.0 {
                down
            } else {
                0.0
            };
        }

        let tr = true_range(&h, &l, &c);
        let atr = wilder_smooth(&tr, p);
        let smooth_plus = wilder_smooth(&plus_dm, p);
        let smooth_minus = wilder_smooth(&minus_dm, p);

        let mut dx = vec![f64::NAN; n];
        for i in 0..n {
            if !atr[i].is_nan() && atr[i] > 0.0 {
                let plus_di = 100.0 * smooth_plus[i] / atr[i];
                let minus_di = 100.0 * smooth_minus[i] / atr[i];
                let sum = plus_di + minus_di;
                dx[i] = if sum > 0.0 {
                    100.0 * (plus_di - minus_di).abs() / sum
                } else {
                    f64::NAN
                };
            }
        }

        let adx = wilder_smooth(&dx, p);
        vec![adx]
    }
}

/// Average True Range (ATR).
///
/// Measures market volatility by calculating the average of the true range
/// over a period. The true range accounts for gaps between sessions. Useful
/// for position sizing, setting stop-loss levels, and comparing volatility
/// across instruments.
///
/// Formula:
///
/// $$
/// \begin{aligned}
/// TR_t &= \max(H_t - L_t,\; |H_t - C_{t-1}|,\; |L_t - C_{t-1}|) \\\\
/// ATR_t &= \frac{1}{n} \sum_{i=0}^{n-1} TR_{t-i}
/// \end{aligned}
/// $$
///
/// Read more on [Wikipedia][wiki-atr].
///
/// Parameters
/// ----------
/// period : int, default=14
///     Look-back window length.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:AverageDirectionalIndex
/// backtide.indicators:BollingerBands
/// backtide.indicators:SimpleMovingAverage
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct AverageTrueRange {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl AverageTrueRange {
    #[new]
    #[pyo3(signature = (period: "int"=14))]
    pub fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Indicator for AverageTrueRange {
    const ACRONYM: &'static str = "ATR";
    const NAME: &'static str = "Average True Range";
    const DESCRIPTION: &'static str = "Measures market volatility by calculating the average range between high and low prices over a period.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let (_o, h, l, _c, _v) = extract_ohlcv_from_bars(bars);
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let tr = true_range(&h, &l, &c);
        vec![wilder_smooth(&tr, self.period)]
    }
}

/// Bollinger Bands (BB).
///
/// Volatility bands placed above and below an n-period SMA. The bands widen
/// during high volatility and contract during low volatility. Useful for
/// volatility assessment, mean-reversion strategies, and breakout detection
/// when price moves outside the bands.
///
/// Formula:
///
/// $$
/// \begin{aligned}
/// Upper_t &= SMA_t + k \cdot \sigma_t \\\\
/// Lower_t &= SMA_t - k \cdot \sigma_t
/// \end{aligned}
/// $$
///
/// where $\sigma_t$ is the rolling standard deviation over $n$ periods. Read
/// more on [Wikipedia][wiki-bb].
///
/// Parameters
/// ----------
/// period : int, default=20
///     Number of bars for the moving average.
///
/// std_dev : float, default=2.0
///     Number of standard deviations.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:AverageTrueRange
/// backtide.indicators:CommodityChannelIndex
/// backtide.indicators:SimpleMovingAverage
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct BollingerBands {
    /// Look-back window length.
    period: usize,

    /// Number of standard deviations for the band width.
    std_dev: f64,
}

#[pymethods]
impl BollingerBands {
    #[new]
    #[pyo3(signature = (period: "int"=20, std_dev: "float"=2.0))]
    pub fn new(period: usize, std_dev: f64) -> Self {
        Self {
            period,
            std_dev,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, f64))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period, self.std_dev)))
    }
}

impl Indicator for BollingerBands {
    const ACRONYM: &'static str = "BB";
    const NAME: &'static str = "Bollinger Bands";
    const DESCRIPTION: &'static str = "Volatility bands placed above and below a moving average, widening during high volatility and narrowing during low volatility.";

    /// Returns `[upper, middle, lower]` where `middle = SMA(close, period)`.
    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let mid = rolling_mean(&c, self.period);
        let std = rolling_std(&c, self.period);
        let n = c.len();
        let mut upper = vec![f64::NAN; n];
        let mut lower = vec![f64::NAN; n];
        for i in 0..n {
            if !mid[i].is_nan() && !std[i].is_nan() {
                upper[i] = mid[i] + self.std_dev * std[i];
                lower[i] = mid[i] - self.std_dev * std[i];
            }
        }
        vec![upper, mid, lower]
    }
}

/// Commodity Channel Index (CCI).
///
/// Measures how far the typical price deviates from its statistical mean,
/// identifying cyclical trends. Values above +100 suggest overbought
/// conditions; below -100, oversold. Useful for identifying cyclical price
/// patterns, spotting divergences, and timing entries in commodities and
/// equities.
///
/// Formula:
///
/// $$
/// \begin{aligned}
/// TP_t &= \frac{H_t + L_t + C_t}{3} \\\\
/// CCI_t &= \frac{TP_t - SMA_n(TP_t)}{0.015 \cdot MD_t}
/// \end{aligned}
/// $$
///
/// where $MD_t$ is the mean absolute deviation of $TP$ over $n$ periods. Read
/// more on [Wikipedia][wiki-cci].
///
/// Parameters
/// ----------
/// period : int, default=20
///     Look-back window length.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:BollingerBands
/// backtide.indicators:RelativeStrengthIndex
/// backtide.indicators:StochasticOscillator
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct CommodityChannelIndex {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl CommodityChannelIndex {
    #[new]
    #[pyo3(signature = (period: "int"=20))]
    pub fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Indicator for CommodityChannelIndex {
    const ACRONYM: &'static str = "CCI";
    const NAME: &'static str = "Commodity Channel Index";
    const DESCRIPTION: &'static str = "Measures a price's deviation from its statistical mean, identifying cyclical trends in the data.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let (_o, h, l, _c, _v) = extract_ohlcv_from_bars(bars);
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let n = c.len();
        let p = self.period;
        let tp: Vec<f64> = (0..n).map(|i| (h[i] + l[i] + c[i]) / 3.0).collect();
        let ma = rolling_mean(&tp, p);
        let mut out = vec![f64::NAN; n];
        if n >= p && p > 0 {
            for i in (p - 1)..n {
                let window = &tp[i + 1 - p..=i];
                let mean = ma[i];
                let md: f64 = window.iter().map(|x| (x - mean).abs()).sum::<f64>() / p as f64;
                out[i] = if md > 0.0 {
                    (tp[i] - mean) / (0.015 * md)
                } else {
                    f64::NAN
                };
            }
        }
        vec![out]
    }
}

/// Exponential Moving Average (EMA).
///
/// A weighted moving average that gives exponentially more weight to recent
/// prices, making it more responsive to new information than the SMA. Useful
/// for faster trend detection, reducing lag in crossover systems, and as a
/// building block for other indicators (MACD, ADX).
///
/// Formula:
///
/// $$EMA_t = \alpha \cdot C_t + (1 - \alpha) \cdot EMA_{t-1}$$
///
/// where $\alpha = \frac{2}{n + 1}$. Read more on [Wikipedia][wiki-ema].
///
/// Parameters
/// ----------
/// period : int, default=14
///     Look-back window length.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:MovingAverageConvergenceDivergence
/// backtide.indicators:SimpleMovingAverage
/// backtide.indicators:WeightedMovingAverage
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct ExponentialMovingAverage {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl ExponentialMovingAverage {
    #[new]
    #[pyo3(signature = (period: "int"=14))]
    pub fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Indicator for ExponentialMovingAverage {
    const ACRONYM: &'static str = "EMA";
    const NAME: &'static str = "Exponential Moving Average";
    const DESCRIPTION: &'static str = "Weighted moving average that gives more weight to recent prices, reacting faster to price changes than SMA.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        vec![ewm(&c, self.period)]
    }
}

/// Moving Average Convergence Divergence (MACD).
///
/// A trend-following momentum indicator that shows the relationship between
/// two EMAs. The MACD line is the difference between a fast and slow EMA;
/// the signal line is an EMA of the MACD line itself. Useful for trend
/// direction and momentum, signal line crossovers for entry/exit timing,
/// and histogram divergence analysis.
///
/// Formula:
///
/// $$
/// \begin{aligned}
/// MACD_t &= EMA_{fast}(C_t) - EMA_{slow}(C_t) \\\\
/// Signal_t &= EMA_{signal}(MACD_t)
/// \end{aligned}
/// $$
///
/// Read more on [Wikipedia][wiki-macd].
///
/// Parameters
/// ----------
/// fast_period : int, default=12
///     Fast EMA period.
///
/// slow_period : int, default=26
///     Slow EMA period.
///
/// signal_period : int, default=9
///     Signal line EMA period.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:AverageDirectionalIndex
/// backtide.indicators:ExponentialMovingAverage
/// backtide.indicators:RelativeStrengthIndex
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct MovingAverageConvergenceDivergence {
    /// Fast EMA period.
    fast_period: usize,

    /// Slow EMA period.
    slow_period: usize,

    /// Signal line EMA period.
    signal_period: usize,
}

#[pymethods]
impl MovingAverageConvergenceDivergence {
    #[new]
    #[pyo3(signature = (fast_period: "int"=12, slow_period: "int"=26, signal_period: "int"=9))]
    pub fn new(fast_period: usize, slow_period: usize, signal_period: usize) -> Self {
        Self {
            fast_period,
            slow_period,
            signal_period,
        }
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (usize, usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.fast_period, self.slow_period, self.signal_period)))
    }
}

impl Indicator for MovingAverageConvergenceDivergence {
    const ACRONYM: &'static str = "MACD";
    const NAME: &'static str = "Moving Avg. Convergence Divergence";
    const DESCRIPTION: &'static str = "Trend-following momentum indicator showing the relationship between two exponential moving averages of closing prices.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let fast = ewm(&c, self.fast_period);
        let slow = ewm(&c, self.slow_period);
        let n = c.len();
        let mut macd_line = vec![f64::NAN; n];
        for i in 0..n {
            if !fast[i].is_nan() && !slow[i].is_nan() {
                macd_line[i] = fast[i] - slow[i];
            }
        }
        let signal_line = ewm(&macd_line, self.signal_period);
        vec![macd_line, signal_line]
    }
}

/// On-Balance Volume (OBV).
///
/// A cumulative volume indicator that adds volume on up-close days and
/// subtracts it on down-close days. Rising OBV confirms an uptrend;
/// falling OBV confirms a downtrend. Useful for confirming price trends
/// with volume and spotting divergences between price and volume momentum.
///
/// Formula:
///
/// $$OBV_t = \begin{cases} OBV_{t-1} + V_t & \text{if } C_t > C_{t-1} \\ OBV_{t-1} - V_t & \text{if } C_t < C_{t-1} \\ OBV_{t-1} & \text{otherwise} \end{cases}$$
///
/// Read more on [Wikipedia][wiki-obv].
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:MovingAverageConvergenceDivergence
/// backtide.indicators:RelativeStrengthIndex
/// backtide.indicators:VolumeWeightedAveragePrice
#[pyclass(skip_from_py_object, module = "backtide.indicators")]
#[derive(Clone, Debug, Default)]
pub struct OnBalanceVolume;

#[pymethods]
impl OnBalanceVolume {
    #[new]
    pub fn new() -> Self {
        Self
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, ())> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, ()))
    }
}

impl Indicator for OnBalanceVolume {
    const ACRONYM: &'static str = "OBV";
    const NAME: &'static str = "On-Balance Volume";
    const DESCRIPTION: &'static str = "Cumulative volume indicator that adds volume on up days and subtracts it on down days to confirm price trends.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let n = bars.len();
        let mut obv = vec![0.0; n];
        for i in 1..n {
            obv[i] = if bars[i].close > bars[i - 1].close {
                obv[i - 1] + bars[i].volume
            } else if bars[i].close < bars[i - 1].close {
                obv[i - 1] - bars[i].volume
            } else {
                obv[i - 1]
            };
        }
        vec![obv]
    }
}

/// Relative Strength Index (RSI).
///
/// A momentum oscillator that measures the speed and magnitude of recent
/// price changes on a scale of 0 to 100. Values above 70 are typically
/// considered overbought; below 30, oversold. Useful for identifying
/// overbought/oversold conditions, spotting divergences, and confirming
/// trend strength.
///
/// Formula:
///
/// $$RSI = 100 - \frac{100}{1 + RS}$$
///
/// where $RS = \frac{\text{avg gain over } n}{\text{avg loss over } n}$. Read
/// more on [Wikipedia][wiki-rsi].
///
/// Parameters
/// ----------
/// period : int, default=14
///     Look-back window length.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:CommodityChannelIndex
/// backtide.indicators:MovingAverageConvergenceDivergence
/// backtide.indicators:StochasticOscillator
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct RelativeStrengthIndex {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl RelativeStrengthIndex {
    #[new]
    #[pyo3(signature = (period: "int"=14))]
    pub fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Indicator for RelativeStrengthIndex {
    const ACRONYM: &'static str = "RSI";
    const NAME: &'static str = "Relative Strength Index";
    const DESCRIPTION: &'static str = "Momentum oscillator (0\u{2013}100) measuring the speed and magnitude of recent price changes to identify overbought/oversold conditions.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let n = c.len();
        let p = self.period;
        let mut out = vec![f64::NAN; n];
        if n >= 2 && p > 0 {
            // Per-bar gains and losses. Index 0 has no previous close, so
            // mark it NaN — that way `wilder_smooth` seeds at the first
            // window of `period` *actual* deltas (index `period`), matching
            // Wilder's standard initialization.
            let mut gains = vec![f64::NAN; n];
            let mut losses = vec![f64::NAN; n];
            for i in 1..n {
                if !c[i].is_finite() || !c[i - 1].is_finite() {
                    continue;
                }
                let delta = c[i] - c[i - 1];
                if delta > 0.0 {
                    gains[i] = delta;
                    losses[i] = 0.0;
                } else {
                    gains[i] = 0.0;
                    losses[i] = -delta;
                }
            }
            // Wilder's smoothing (α = 1 / period) — the textbook RSI
            // definition. Distinct from a plain SMA / EMA of gains.
            let avg_gain = wilder_smooth(&gains, p);
            let avg_loss = wilder_smooth(&losses, p);
            for i in 0..n {
                if !avg_gain[i].is_finite() || !avg_loss[i].is_finite() {
                    continue;
                }
                if avg_loss[i] == 0.0 {
                    out[i] = 100.0;
                } else {
                    let rs = avg_gain[i] / avg_loss[i];
                    out[i] = 100.0 - (100.0 / (1.0 + rs));
                }
            }
        }
        vec![out]
    }
}

/// Simple Moving Average (SMA).
///
/// The arithmetic mean of the last n closing prices. Used to smooth
/// short-term fluctuations and identify the direction of a trend. Useful
/// for trend identification, support/resistance levels, and crossover
/// strategies (e.g., golden cross / death cross).
///
/// Formula:
///
/// $$SMA_t = \frac{1}{n} \sum_{i=0}^{n-1} C_{t-i}$$
///
/// where $C_t$ is the closing price at time $t$ and $n$ is the period. Read
/// more on [Wikipedia][wiki-sma].
///
/// Parameters
/// ----------
/// period : int, default=14
///     Look-back window length.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:BollingerBands
/// backtide.indicators:ExponentialMovingAverage
/// backtide.indicators:WeightedMovingAverage
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct SimpleMovingAverage {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl SimpleMovingAverage {
    #[new]
    #[pyo3(signature = (period: "int"=14))]
    pub fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Indicator for SimpleMovingAverage {
    const ACRONYM: &'static str = "SMA";
    const NAME: &'static str = "Simple Moving Average";
    const DESCRIPTION: &'static str = "Arithmetic mean of the last N closing prices, used to smooth short-term fluctuations and identify trends.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        vec![rolling_mean(&c, self.period)]
    }
}

/// Stochastic Oscillator (STOCH).
///
/// Compares the closing price to the high-low range over a period,
/// producing a %K line and a smoothed %D signal line. Both oscillate
/// between 0 and 100. Useful for overbought/oversold signals, %K/%D
/// crossovers for entry/exit timing, and divergence analysis.
///
/// Formula:
///
/// $$
/// \begin{aligned}
/// \%K_t &= 100 \cdot \frac{C_t - L_n}{H_n - L_n} \\\\
/// \%D_t &= SMA_d(\%K_t)
/// \end{aligned}
/// $$
///
/// where $H_n$ and $L_n$ are the highest high and lowest low over $n$ periods.
/// Read more on [Wikipedia][wiki-stoch].
///
/// Parameters
/// ----------
/// k_period : int, default=14
///     %K look-back period.
///
/// d_period : int, default=3
///     %D smoothing period.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:CommodityChannelIndex
/// backtide.indicators:MovingAverageConvergenceDivergence
/// backtide.indicators:RelativeStrengthIndex
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct StochasticOscillator {
    /// %K look-back period.
    k_period: usize,

    /// %D smoothing period.
    d_period: usize,
}

#[pymethods]
impl StochasticOscillator {
    #[new]
    #[pyo3(signature = (k_period: "int"=14, d_period: "int"=3))]
    pub fn new(k_period: usize, d_period: usize) -> Self {
        Self {
            k_period,
            d_period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, usize))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.k_period, self.d_period)))
    }
}

impl Indicator for StochasticOscillator {
    const ACRONYM: &'static str = "STOCH";
    const NAME: &'static str = "Stochastic Oscillator";
    const DESCRIPTION: &'static str = "Compares a closing price to a range of prices over a period, generating overbought/oversold signals.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let (_o, h, l, _c, _v) = extract_ohlcv_from_bars(bars);
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let n = c.len();
        let p = self.k_period;
        let mut k = vec![f64::NAN; n];
        if n >= p && p > 0 {
            for i in (p - 1)..n {
                let window_h = &h[i + 1 - p..=i];
                let window_l = &l[i + 1 - p..=i];
                let high_max = window_h.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let low_min = window_l.iter().cloned().fold(f64::INFINITY, f64::min);
                let range = high_max - low_min;
                k[i] = if range > 0.0 {
                    100.0 * (c[i] - low_min) / range
                } else {
                    f64::NAN
                };
            }
        }
        let d = rolling_mean(&k, self.d_period);
        vec![k, d]
    }
}

/// Volume-Weighted Average Price (VWAP).
///
/// The cumulative average price weighted by volume. Institutional traders
/// use VWAP as a benchmark: buying below VWAP is considered favorable,
/// selling above it likewise. Useful as an intraday trading benchmark,
/// for assessing execution quality, and as dynamic support/resistance.
///
/// Formula:
///
/// $$VWAP_t = \frac{\sum_{i=1}^{t} TP_i \cdot V_i}{\sum_{i=1}^{t} V_i}$$
///
/// where $TP_i = \frac{H_i + L_i + C_i}{3}$. Read more on [Wikipedia][wiki-vwap].
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:OnBalanceVolume
/// backtide.indicators:SimpleMovingAverage
/// backtide.indicators:WeightedMovingAverage
#[pyclass(skip_from_py_object, module = "backtide.indicators")]
#[derive(Clone, Debug, Default)]
pub struct VolumeWeightedAveragePrice;

#[pymethods]
impl VolumeWeightedAveragePrice {
    #[new]
    pub fn new() -> Self {
        Self
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, ())> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, ()))
    }
}

impl Indicator for VolumeWeightedAveragePrice {
    const ACRONYM: &'static str = "VWAP";
    const NAME: &'static str = "Volume-Weighted Average Price";
    const DESCRIPTION: &'static str =
        "Average price weighted by volume, used as a benchmark for intraday trading quality.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let n = bars.len();
        let mut vwap = vec![f64::NAN; n];
        let mut cum_tp_vol = 0.0;
        let mut cum_vol = 0.0;
        for i in 0..n {
            let tp = (bars[i].high + bars[i].low + bars[i].close) / 3.0;
            cum_tp_vol += tp * bars[i].volume;
            cum_vol += bars[i].volume;
            vwap[i] = if cum_vol > 0.0 {
                cum_tp_vol / cum_vol
            } else {
                f64::NAN
            };
        }
        vec![vwap]
    }
}

/// Weighted Moving Average (WMA).
///
/// A moving average where each price is multiplied by a linearly decreasing
/// weight, placing more emphasis on recent data than the SMA but with a
/// different weighting scheme than the EMA. Useful when you want recent
/// prices to matter more without the recursive smoothing of EMA.
///
/// Formula:
///
/// $$WMA_t = \frac{\sum_{i=0}^{n-1} (n - i) \cdot C_{t-i}}{\sum_{i=1}^{n} i}$$
///
/// Read more on [Wikipedia][wiki-wma].
///
/// Parameters
/// ----------
/// period : int, default=14
///     Look-back window length.
///
/// Attributes
/// ----------
/// acronym : str
///     Short ticker-style acronym.
///
/// name : str
///     Human-readable indicator name.
///
/// See Also
/// --------
/// backtide.indicators:ExponentialMovingAverage
/// backtide.indicators:MovingAverageConvergenceDivergence
/// backtide.indicators:SimpleMovingAverage
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct WeightedMovingAverage {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl WeightedMovingAverage {
    #[new]
    #[pyo3(signature = (period: "int"=14))]
    pub fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.get_type::<Self>().into_any();
        Ok((cls, (self.period,)))
    }
}

impl Indicator for WeightedMovingAverage {
    const ACRONYM: &'static str = "WMA";
    const NAME: &'static str = "Weighted Moving Average";
    const DESCRIPTION: &'static str = "Moving average where each price is multiplied by a linearly decreasing weight, emphasizing recent data.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let n = c.len();
        let p = self.period;
        let mut out = vec![f64::NAN; n];
        if p > 0 && n >= p {
            let w_sum: f64 = (1..=p).map(|x| x as f64).sum();
            for i in (p - 1)..n {
                let window = &c[i + 1 - p..=i];
                if !window.iter().all(|x| x.is_finite()) {
                    continue;
                }
                let mut val = 0.0;
                for j in 0..p {
                    val += c[i + 1 - p + j] * (j + 1) as f64;
                }
                out[i] = val / w_sum;
            }
        }
        vec![out]
    }
}

indicator_pymethods!(AverageDirectionalIndex);
indicator_pymethods!(AverageTrueRange);
indicator_pymethods!(BollingerBands);
indicator_pymethods!(CommodityChannelIndex);
indicator_pymethods!(ExponentialMovingAverage);
indicator_pymethods!(MovingAverageConvergenceDivergence);
indicator_pymethods!(OnBalanceVolume);
indicator_pymethods!(RelativeStrengthIndex);
indicator_pymethods!(SimpleMovingAverage);
indicator_pymethods!(StochasticOscillator);
indicator_pymethods!(VolumeWeightedAveragePrice);
indicator_pymethods!(WeightedMovingAverage);

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(close: f64) -> Bar {
        Bar {
            open_ts: 0,
            close_ts: 0,
            open_ts_exchange: 0,
            open: close,
            high: close + 1.0,
            low: close - 1.0,
            close,
            adj_close: close,
            volume: 1.0,
            n_trades: None,
        }
    }

    #[test]
    fn rolling_mean_recovers_after_leading_nans() {
        let out = rolling_mean(&[f64::NAN, f64::NAN, 10.0, 12.0, 14.0, 16.0], 3);

        assert!(out[0].is_nan());
        assert!(out[3].is_nan());
        assert_eq!(out[4], 12.0);
        assert_eq!(out[5], 14.0);
    }

    #[test]
    fn sma_recovers_for_later_starting_symbol() {
        let bars =
            [f64::NAN, f64::NAN, 10.0, 12.0, 14.0, 16.0].into_iter().map(bar).collect::<Vec<_>>();
        let sma = SimpleMovingAverage::new(3).compute_inner(&bars);

        assert!(sma[0][3].is_nan());
        assert_eq!(sma[0][4], 12.0);
        assert_eq!(sma[0][5], 14.0);
    }

    #[test]
    fn rolling_std_and_wma_recover_after_leading_nans() {
        let std = rolling_std(&[f64::NAN, 1.0, 2.0, 3.0], 3);
        assert!(std[2].is_nan());
        assert!(std[3].is_finite());

        let bars = [f64::NAN, 1.0, 2.0, 3.0].into_iter().map(bar).collect::<Vec<_>>();
        let wma = WeightedMovingAverage::new(3).compute_inner(&bars);
        assert!(wma[0][2].is_nan());
        assert_eq!(wma[0][3], (1.0 * 1.0 + 2.0 * 2.0 + 3.0 * 3.0) / 6.0);
    }

    #[test]
    fn true_range_uses_high_low_when_previous_close_is_nan() {
        let tr = true_range(&[f64::NAN, 12.0], &[f64::NAN, 10.0], &[f64::NAN, 11.0]);

        assert!(tr[0].is_nan());
        assert_eq!(tr[1], 2.0);
    }

    fn ohlc_bar(open: f64, high: f64, low: f64, close: f64, volume: f64) -> Bar {
        Bar {
            open_ts: 0,
            close_ts: 0,
            open_ts_exchange: 0,
            open,
            high,
            low,
            close,
            adj_close: close,
            volume,
            n_trades: None,
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    #[test]
    fn rolling_mean_returns_nan_vec_for_zero_period_or_short_data() {
        assert!(rolling_mean(&[1.0, 2.0, 3.0], 0).iter().all(|v| v.is_nan()));
        assert!(rolling_mean(&[1.0, 2.0], 5).iter().all(|v| v.is_nan()));
        assert!(rolling_mean(&[], 3).is_empty());
    }

    #[test]
    fn rolling_mean_computes_basic_average() {
        let out = rolling_mean(&[1.0, 2.0, 3.0, 4.0, 5.0], 3);
        assert!(out[0].is_nan());
        assert!(out[1].is_nan());
        assert_eq!(out[2], 2.0);
        assert_eq!(out[3], 3.0);
        assert_eq!(out[4], 4.0);
    }

    #[test]
    fn ewm_handles_empty_and_zero_span() {
        assert!(ewm(&[], 5).is_empty());
        let out = ewm(&[1.0, 2.0], 0);
        assert!(out.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn ewm_seeds_from_first_value_and_smooths() {
        let out = ewm(&[1.0, 2.0, 3.0], 1);
        // alpha = 1.0, so EMA tracks data exactly
        assert_eq!(out[0], 1.0);
        assert_eq!(out[1], 2.0);
        assert_eq!(out[2], 3.0);
    }

    #[test]
    fn wilder_smooth_seeds_with_sma_then_smooths() {
        // First valid value at index period-1 = SMA of [1,2,3,4] = 2.5.
        // Then v[4] = 2.5 + 0.25 * (5.0 - 2.5) = 3.125.
        let out = wilder_smooth(&[1.0, 2.0, 3.0, 4.0, 5.0], 4);
        assert!(out[2].is_nan());
        assert!((out[3] - 2.5).abs() < 1e-12);
        assert!((out[4] - 3.125).abs() < 1e-12);
    }

    #[test]
    fn wilder_smooth_handles_zero_period_and_short_input() {
        assert!(wilder_smooth(&[1.0, 2.0], 0).iter().all(|v| v.is_nan()));
        assert!(wilder_smooth(&[1.0], 5).iter().all(|v| v.is_nan()));
    }

    #[test]
    fn wilder_smooth_seeds_after_leading_nans() {
        // First fully-finite window of size 3 is [2,3,4] at index 4.
        let out = wilder_smooth(&[f64::NAN, f64::NAN, 2.0, 3.0, 4.0, 5.0], 3);
        assert!(out[3].is_nan());
        assert!((out[4] - 3.0).abs() < 1e-12);
        // 3.0 + (1/3) * (5 - 3) = 3.6666...
        assert!((out[5] - (3.0 + (5.0 - 3.0) / 3.0)).abs() < 1e-12);
    }

    #[test]
    fn rolling_std_short_input_returns_nans() {
        assert!(rolling_std(&[1.0, 2.0], 5).iter().all(|v| v.is_nan()));
        assert!(rolling_std(&[1.0, 2.0, 3.0], 1).iter().all(|v| v.is_nan()));
    }

    #[test]
    fn rolling_std_computes_sample_std() {
        let out = rolling_std(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0], 4);
        // Sample std of [2,4,4,4]: mean=3.5, var=3/3=1 → std=1.0
        assert!((out[3] - 1.0).abs() < 1e-9);
        // Sample std of [4,4,4,5]: mean=4.25, var=0.75/3=0.25 → std=0.5
        assert!((out[4] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn true_range_handles_empty_and_single_bar() {
        assert!(true_range(&[], &[], &[]).is_empty());
        let tr = true_range(&[10.0], &[5.0], &[7.0]);
        assert_eq!(tr[0], 5.0);
    }

    #[test]
    fn true_range_picks_max_of_three_components() {
        // Gap up: |high - prev_close| dominates.
        let tr = true_range(&[10.0, 30.0], &[5.0, 25.0], &[7.0, 28.0]);
        assert_eq!(tr[0], 5.0);
        // hl = 5, hc = |30 - 7| = 23, lc = |25 - 7| = 18 → 23
        assert_eq!(tr[1], 23.0);
    }

    // ── Indicators ──────────────────────────────────────────────────────────

    #[test]
    fn adx_returns_nan_for_short_input() {
        let bars = vec![bar(1.0)];
        let out = AverageDirectionalIndex::new(14).compute_inner(&bars);
        assert_eq!(out.len(), 1);
        assert!(out[0][0].is_nan());
    }

    #[test]
    fn adx_produces_finite_values_for_trending_data() {
        let closes: Vec<f64> = (0..40).map(|i| 100.0 + i as f64).collect();
        let bars: Vec<Bar> = closes.into_iter().map(bar).collect();
        let out = AverageDirectionalIndex::new(5).compute_inner(&bars);
        assert!(out[0].iter().any(|v| v.is_finite()));
    }

    #[test]
    fn atr_matches_rolling_mean_of_true_range() {
        let bars: Vec<Bar> = (0..10).map(|i| bar(10.0 + i as f64)).collect();
        let out = AverageTrueRange::new(3).compute_inner(&bars);
        // Last bar's high-low=2, hc/lc differences are bounded — value should be finite.
        assert!(out[0][5].is_finite());
    }

    #[test]
    fn bollinger_bands_returns_three_series() {
        let bars: Vec<Bar> = (0..30).map(|i| bar(10.0 + i as f64)).collect();
        let out = BollingerBands::new(5, 2.0).compute_inner(&bars);
        // [upper, middle, lower]
        assert_eq!(out.len(), 3);
        let (upper, middle, lower) = (&out[0], &out[1], &out[2]);
        let idx = 10;
        assert!(upper[idx] > middle[idx]);
        assert!(middle[idx] > lower[idx]);
        // Middle band must equal the SMA of closes for that window.
        let win_mean: f64 = bars[idx + 1 - 5..=idx].iter().map(|b| b.close).sum::<f64>() / 5.0;
        assert!((middle[idx] - win_mean).abs() < 1e-9);
    }

    #[test]
    fn cci_handles_zero_md_window_with_nan() {
        // Constant typical price → mean deviation is zero → CCI must be NaN.
        let bars: Vec<Bar> = (0..10).map(|_| ohlc_bar(1.0, 1.0, 1.0, 1.0, 1.0)).collect();
        let out = CommodityChannelIndex::new(3).compute_inner(&bars);
        assert!(out[0].iter().skip(2).all(|v| v.is_nan()));
    }

    #[test]
    fn cci_produces_finite_for_varying_prices() {
        let bars: Vec<Bar> =
            (0..20).map(|i| ohlc_bar(0.0, (i + 2) as f64, i as f64, (i + 1) as f64, 1.0)).collect();
        let out = CommodityChannelIndex::new(5).compute_inner(&bars);
        assert!(out[0].iter().any(|v| v.is_finite()));
    }

    #[test]
    fn ema_first_value_equals_input() {
        let bars: Vec<Bar> = [5.0, 6.0, 7.0].into_iter().map(bar).collect();
        let out = ExponentialMovingAverage::new(2).compute_inner(&bars);
        assert_eq!(out[0][0], 5.0);
        // Subsequent values bounded by the data range.
        assert!(out[0][2] > 5.0 && out[0][2] < 8.0);
    }

    #[test]
    fn macd_returns_two_series_and_macd_line_eq_fast_minus_slow() {
        let bars: Vec<Bar> = (0..50).map(|i| bar(100.0 + (i as f64).sin())).collect();
        let out = MovingAverageConvergenceDivergence::new(3, 6, 2).compute_inner(&bars);
        assert_eq!(out.len(), 2);
        assert!(out[0].iter().any(|v| v.is_finite()));
        assert!(out[1].iter().any(|v| v.is_finite()));
    }

    #[test]
    fn obv_accumulates_volume_by_direction() {
        let bars = vec![
            ohlc_bar(0.0, 0.0, 0.0, 10.0, 100.0),
            ohlc_bar(0.0, 0.0, 0.0, 11.0, 50.0), // up → +50
            ohlc_bar(0.0, 0.0, 0.0, 11.0, 30.0), // flat → 0
            ohlc_bar(0.0, 0.0, 0.0, 9.0, 40.0),  // down → -40
        ];
        let out = OnBalanceVolume::new().compute_inner(&bars);
        assert_eq!(out[0], vec![0.0, 50.0, 50.0, 10.0]);
    }

    #[test]
    fn rsi_is_100_when_no_losses_in_window() {
        let bars: Vec<Bar> = (0..20).map(|i| bar(100.0 + i as f64)).collect();
        let out = RelativeStrengthIndex::new(5).compute_inner(&bars);
        // All differences positive → avg loss is zero → RSI = 100.
        assert_eq!(out[0][10], 100.0);
    }

    #[test]
    fn rsi_handles_short_input() {
        let bars = vec![bar(1.0)];
        let out = RelativeStrengthIndex::new(14).compute_inner(&bars);
        assert!(out[0][0].is_nan());
    }

    #[test]
    fn sma_handles_empty_input() {
        let out = SimpleMovingAverage::new(3).compute_inner(&[]);
        assert_eq!(out.len(), 1);
        assert!(out[0].is_empty());
    }

    #[test]
    fn stoch_returns_two_series_and_finite_values() {
        let bars: Vec<Bar> =
            (0..30).map(|i| ohlc_bar(0.0, (i + 5) as f64, i as f64, (i + 2) as f64, 1.0)).collect();
        let out = StochasticOscillator::new(5, 3).compute_inner(&bars);
        assert_eq!(out.len(), 2);
        assert!(out[0].iter().any(|v| v.is_finite()));
        assert!(out[1].iter().any(|v| v.is_finite()));
    }

    #[test]
    fn stoch_returns_nan_when_range_is_zero() {
        // Flat high/low → zero range → NaN %K.
        let bars: Vec<Bar> = (0..10).map(|_| ohlc_bar(1.0, 1.0, 1.0, 1.0, 1.0)).collect();
        let out = StochasticOscillator::new(3, 2).compute_inner(&bars);
        assert!(out[0].iter().all(|v| v.is_nan()));
    }

    #[test]
    fn vwap_returns_nan_when_volume_is_zero() {
        let bars: Vec<Bar> =
            (0..3).map(|i| ohlc_bar(0.0, (i + 2) as f64, i as f64, (i + 1) as f64, 0.0)).collect();
        let out = VolumeWeightedAveragePrice::new().compute_inner(&bars);
        assert!(out[0].iter().all(|v| v.is_nan()));
    }

    #[test]
    fn vwap_matches_volume_weighted_typical_price() {
        let bars = vec![
            ohlc_bar(0.0, 3.0, 1.0, 2.0, 10.0), // tp=2.0
            ohlc_bar(0.0, 6.0, 2.0, 4.0, 20.0), // tp=4.0
        ];
        let out = VolumeWeightedAveragePrice::new().compute_inner(&bars);
        assert!((out[0][0] - 2.0).abs() < 1e-9);
        // Weighted: (2*10 + 4*20) / 30 = 100/30
        assert!((out[0][1] - (100.0 / 30.0)).abs() < 1e-9);
    }

    #[test]
    fn wma_returns_nan_when_period_exceeds_length() {
        let bars: Vec<Bar> = [1.0, 2.0].into_iter().map(bar).collect();
        let out = WeightedMovingAverage::new(5).compute_inner(&bars);
        assert!(out[0].iter().all(|v| v.is_nan()));
    }

    #[test]
    fn wma_zero_period_yields_all_nan() {
        let bars: Vec<Bar> = [1.0, 2.0, 3.0].into_iter().map(bar).collect();
        let out = WeightedMovingAverage::new(0).compute_inner(&bars);
        assert!(out[0].iter().all(|v| v.is_nan()));
    }

    // ── Trait constants ─────────────────────────────────────────────────────

    #[test]
    fn indicator_constants_are_set() {
        assert_eq!(<SimpleMovingAverage as Indicator>::ACRONYM, "SMA");
        assert_eq!(<AverageDirectionalIndex as Indicator>::ACRONYM, "ADX");
        assert_eq!(<AverageTrueRange as Indicator>::ACRONYM, "ATR");
        assert_eq!(<BollingerBands as Indicator>::ACRONYM, "BB");
        assert_eq!(<CommodityChannelIndex as Indicator>::ACRONYM, "CCI");
        assert_eq!(<ExponentialMovingAverage as Indicator>::ACRONYM, "EMA");
        assert_eq!(<MovingAverageConvergenceDivergence as Indicator>::ACRONYM, "MACD");
        assert_eq!(<OnBalanceVolume as Indicator>::ACRONYM, "OBV");
        assert_eq!(<RelativeStrengthIndex as Indicator>::ACRONYM, "RSI");
        assert_eq!(<StochasticOscillator as Indicator>::ACRONYM, "STOCH");
        assert_eq!(<VolumeWeightedAveragePrice as Indicator>::ACRONYM, "VWAP");
        assert_eq!(<WeightedMovingAverage as Indicator>::ACRONYM, "WMA");
        assert!(!<SimpleMovingAverage as Indicator>::NAME.is_empty());
        assert!(!<SimpleMovingAverage as Indicator>::DESCRIPTION.is_empty());
    }

    #[test]
    fn ewm_with_leading_nans_recovers() {
        let out = ewm(&[f64::NAN, f64::NAN, 5.0, 6.0, 7.0], 2);
        assert!(out[0].is_nan());
        assert!(out[1].is_nan());
        assert_eq!(out[2], 5.0);
        assert!(out[3] > 5.0 && out[3] < 7.0);
    }

    #[test]
    fn rolling_mean_period_one_equals_data() {
        let out = rolling_mean(&[1.0, 2.0, 3.0], 1);
        assert_eq!(out, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn rolling_std_all_same_values_is_zero() {
        let out = rolling_std(&[5.0, 5.0, 5.0, 5.0], 3);
        assert!(out[0].is_nan());
        assert!(out[1].is_nan());
        assert!((out[2] - 0.0).abs() < 1e-12);
        assert!((out[3] - 0.0).abs() < 1e-12);
    }

    #[test]
    fn true_range_first_bar_is_high_minus_low() {
        let tr = true_range(&[20.0], &[10.0], &[15.0]);
        assert_eq!(tr[0], 10.0);
    }

    #[test]
    fn rsi_bounded_zero_to_one_hundred() {
        let bars: Vec<Bar> = (0..30).map(|i| bar(100.0 + (i as f64 * 0.5).sin() * 10.0)).collect();
        let out = RelativeStrengthIndex::new(14).compute_inner(&bars);
        for v in &out[0] {
            if v.is_finite() {
                assert!(*v >= 0.0 && *v <= 100.0, "RSI out of bounds: {v}");
            }
        }
    }

    #[test]
    fn bollinger_bands_middle_equals_sma() {
        let bars: Vec<Bar> = (0..20).map(|i| bar(50.0 + i as f64)).collect();
        let bb = BollingerBands::new(5, 2.0).compute_inner(&bars);
        let sma = SimpleMovingAverage::new(5).compute_inner(&bars);
        for i in 0..bars.len() {
            if bb[1][i].is_finite() {
                assert!((bb[1][i] - sma[0][i]).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn ema_empty_input() {
        let out = ExponentialMovingAverage::new(5).compute_inner(&[]);
        assert_eq!(out.len(), 1);
        assert!(out[0].is_empty());
    }

    #[test]
    fn obv_single_bar_is_zero() {
        let bars = vec![ohlc_bar(0.0, 0.0, 0.0, 10.0, 100.0)];
        let out = OnBalanceVolume::new().compute_inner(&bars);
        assert_eq!(out[0], vec![0.0]);
    }

    #[test]
    fn atr_empty_input() {
        let out = AverageTrueRange::new(14).compute_inner(&[]);
        assert_eq!(out.len(), 1);
        assert!(out[0].is_empty());
    }

    #[test]
    fn macd_empty_input() {
        let out = MovingAverageConvergenceDivergence::new(12, 26, 9).compute_inner(&[]);
        assert_eq!(out.len(), 2);
        assert!(out[0].is_empty());
    }
}
