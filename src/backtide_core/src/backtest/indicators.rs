use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::config::interface::Config;
use crate::config::models::dataframe_library::DataFrameLibrary;
use crate::data::models::bar::Bar;

/// Trait for all built-in indicators.
pub trait Indicator {
    /// Short ticker-style acronym (e.g. `"SMA"`).
    const ACRONYM: &'static str;

    /// Human-readable name (e.g. `"Simple Moving Average"`).
    const NAME: &'static str;

    /// One-sentence explanation of what the indicator measures.
    const DESCRIPTION: &'static str;

    /// Compute the indicator values from a slice of [`Bar`].
    ///
    /// Returns one or more series (e.g. MACD returns two: the MACD line
    /// and the signal line).
    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions
// ─────────────────────────────────────────────────────────────────────────────

/// Extract parallel `(open, high, low, close, volume)` arrays from a bar slice.
#[allow(clippy::type_complexity)]
fn extract_ohlcv_from_bars(bars: &[Bar]) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    (
        bars.iter().map(|b| b.open).collect(),
        bars.iter().map(|b| b.high).collect(),
        bars.iter().map(|b| b.low).collect(),
        bars.iter().map(|b| b.close).collect(),
        bars.iter().map(|b| b.volume).collect(),
    )
}

/// Compute a simple rolling mean over `data` with the given `period`.
///
/// The first `period - 1` elements are [`f64::NAN`]. Uses an incremental
/// sum for O(n) performance.
fn rolling_mean(data: &[f64], period: usize) -> Vec<f64> {
    let n = data.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || n < period {
        return out;
    }
    let mut sum: f64 = data[..period].iter().sum();
    out[period - 1] = sum / period as f64;
    for i in period..n {
        sum += data[i] - data[i - period];
        out[i] = sum / period as f64;
    }
    out
}

/// Compute an exponential weighted moving average with the given `span`.
///
/// Uses the standard smoothing factor `α = 2 / (span + 1)`.
fn ewm(data: &[f64], span: usize) -> Vec<f64> {
    let n = data.len();
    let mut out = vec![f64::NAN; n];
    if n == 0 || span == 0 {
        return out;
    }
    let alpha = 2.0 / (span as f64 + 1.0);
    out[0] = data[0];
    for i in 1..n {
        let prev = out[i - 1];
        out[i] = if prev.is_nan() {
            data[i]
        } else {
            alpha * data[i] + (1.0 - alpha) * prev
        };
    }
    out
}

/// Compute the sample rolling standard deviation over `data` with `period`.
///
/// Uses Bessel's correction (denominator `period - 1`). The first
/// `period - 1` elements are [`f64::NAN`].
fn rolling_std(data: &[f64], period: usize) -> Vec<f64> {
    let n = data.len();
    let mut out = vec![f64::NAN; n];
    if period < 2 || n < period {
        return out;
    }
    for i in (period - 1)..n {
        let window = &data[i + 1 - period..=i];
        let mean: f64 = window.iter().sum::<f64>() / period as f64;
        let var: f64 = window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (period - 1) as f64;
        out[i] = var.sqrt();
    }
    out
}

/// Compute the True Range for each bar.
///
/// TR = max(high − low, |high − prev_close|, |low − prev_close|).
/// The first element uses `high[0] − low[0]` since there is no previous close.
fn true_range(high: &[f64], low: &[f64], close: &[f64]) -> Vec<f64> {
    let n = high.len();
    let mut tr = vec![0.0; n];
    tr[0] = high[0] - low[0];
    for i in 1..n {
        let hl = high[i] - low[i];
        let hc = (high[i] - close[i - 1]).abs();
        let lc = (low[i] - close[i - 1]).abs();
        tr[i] = hl.max(hc).max(lc);
    }
    tr
}

/// Extract `open`, `high`, `low`, `close`, `volume` arrays from a pandas DataFrame.
///
/// Falls back to a zero-filled volume array when the column is missing.
#[allow(clippy::type_complexity)]
fn extract_ohlcv(
    df: &Bound<'_, PyAny>,
) -> PyResult<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>)> {
    let extract_col = |name: &str| -> PyResult<Vec<f64>> {
        let col = df.get_item(name)?;

        // Fast path: direct extract works for numpy arrays and lists.
        // Fallback: pandas/polars Series need .values or .to_numpy() first.
        col.extract::<Vec<f64>>().or_else(|_| {
            col.getattr("values")
                .and_then(|v| v.extract::<Vec<f64>>())
                .or_else(|_| col.call_method0("to_numpy")?.extract::<Vec<f64>>())
        })
    };

    Ok((
        extract_col("open")?,
        extract_col("high")?,
        extract_col("low")?,
        extract_col("close")?,
        extract_col("volume").unwrap_or_else(|_| vec![0.0; df.len().unwrap_or(0)]),
    ))
}

/// Convert indicator output series into the configured data backend format.
///
/// Each inner `Vec<f64>` is one output series (e.g. upper band, lower band).
/// The result is shaped as (n_points, n_series) — i.e. rows × columns.
/// Single-series indicators return a 1-D array / single-column frame.
fn to_backend_type(py: Python, series: Vec<Vec<f64>>) -> PyResult<Bound<PyAny>> {
    let backend =
        Config::get().map(|c| c.data.dataframe_library).unwrap_or(DataFrameLibrary::Pandas);

    let np = py.import("numpy")?;

    if series.len() == 1 {
        // Single series → 1-D
        let arr = np.call_method1("array", (series.into_iter().next().unwrap(),))?;
        match backend {
            DataFrameLibrary::Numpy => Ok(arr),
            DataFrameLibrary::Pandas => {
                let pd = py.import("pandas")?;
                pd.call_method1("Series", (&arr,))
            },
            DataFrameLibrary::Polars => {
                let pl = py.import("polars")?;
                pl.call_method1("Series", (&arr,))
            },
        }
    } else {
        // Multiple series → transpose to (n_points, n_series)
        let arr_2d = np.call_method1("array", (series,))?;
        let arr_t = arr_2d.getattr("T")?;
        match backend {
            DataFrameLibrary::Numpy => Ok(arr_t),
            DataFrameLibrary::Pandas => {
                let pd = py.import("pandas")?;
                pd.call_method1("DataFrame", (&arr_t,))
            },
            DataFrameLibrary::Polars => {
                let pl = py.import("polars")?;
                pl.call_method1("from_numpy", (&arr_t,))
            },
        }
    }
}

/// Shared pymethods macro for all indicator structs.
///
/// The struct must already have a `#[pymethods]` block with `new` and `__reduce__`.
/// This macro adds `acronym`, `name`, `description`, `calculate`, `__repr__`.
macro_rules! indicator_pymethods {
    ($ty:ident) => {
        #[pymethods]
        impl $ty {
            /// Short ticker-style acronym (e.g. `"SMA"`).
            #[classattr]
            fn acronym() -> &'static str {
                <$ty as Indicator>::ACRONYM
            }

            /// Human-readable name (e.g. `"Simple Moving Average"`).
            #[classattr]
            fn name() -> &'static str {
                <$ty as Indicator>::NAME
            }

            /// One-sentence explanation of what the indicator measures.
            #[classmethod]
            fn description(_cls: &Bound<'_, PyType>) -> &'static str {
                <$ty as Indicator>::DESCRIPTION
            }

            /// Compute the indicator on a DataFrame.
            ///
            /// Parameters
            /// ----------
            /// data : np.ndarray | Series | DataFrame
            ///     Historical OHLCV data.
            ///
            /// Returns
            /// -------
            /// np.ndarray | pd.Series | pl.Series |  pd.DataFrame | pl.DataFrame
            ///     The computed values. For multi-output indicators (e.g., upper
            ///     and lower bounds), return a 2d structure.
            fn compute<'py>(
                &self,
                py: Python<'py>,
                data: &Bound<'py, PyAny>,
            ) -> PyResult<Bound<'py, PyAny>> {
                let (o, h, l, c, v) = extract_ohlcv(data)?;
                let bars: Vec<Bar> = (0..c.len())
                    .map(|i| Bar {
                        open_ts: 0,
                        close_ts: 0,
                        open_ts_exchange: 0,
                        open: o[i],
                        high: h[i],
                        low: l[i],
                        close: c[i],
                        adj_close: c[i],
                        volume: v[i],
                        n_trades: None,
                    })
                    .collect();
                to_backend_type(py, self.compute_inner(&bars))
            }

            /// Return a debug representation.
            fn __repr__(&self) -> String {
                format!("{}()", <$ty as Indicator>::ACRONYM)
            }
        }
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Indicator structs
// ─────────────────────────────────────────────────────────────────────────────

/// Average Directional Index indicator.
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct AverageDirectionalIndex {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl AverageDirectionalIndex {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.import("backtide.indicators")?.getattr("AverageDirectionalIndex")?;
        Ok((cls, (self.period,)))
    }
}

impl Indicator for AverageDirectionalIndex {
    const ACRONYM: &'static str = "ADX";
    const NAME: &'static str = "Average Directional Index";
    const DESCRIPTION: &'static str = "Quantifies trend strength (0\u{2013}100) regardless of direction, helping distinguish trending from ranging markets.";

    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>> {
        let (_o, h, l, _c, _v) = extract_ohlcv_from_bars(bars);
        let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let n = c.len();
        let p = self.period;
        if n < 2 {
            return vec![vec![f64::NAN; n]];
        }

        let mut plus_dm = vec![0.0; n];
        let mut minus_dm = vec![0.0; n];
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
        let atr = ewm(&tr, p);
        let smooth_plus = ewm(&plus_dm, p);
        let smooth_minus = ewm(&minus_dm, p);

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

        let adx = ewm(&dx, p);
        vec![adx]
    }
}

/// Average True Range indicator.
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct AverageTrueRange {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl AverageTrueRange {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.import("backtide.indicators")?.getattr("AverageTrueRange")?;
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
        vec![rolling_mean(&tr, self.period)]
    }
}

/// Bollinger Bands indicator.
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
    #[pyo3(signature = (period=20, std_dev=2.0))]
    fn new(period: usize, std_dev: f64) -> Self {
        Self {
            period,
            std_dev,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, f64))> {
        let cls = py.import("backtide.indicators")?.getattr("BollingerBands")?;
        Ok((cls, (self.period, self.std_dev)))
    }
}

impl Indicator for BollingerBands {
    const ACRONYM: &'static str = "BB";
    const NAME: &'static str = "Bollinger Bands";
    const DESCRIPTION: &'static str = "Volatility bands placed above and below a moving average, widening during high volatility and narrowing during low volatility.";

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
        vec![upper, lower]
    }
}

/// Commodity Channel Index indicator.
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct CommodityChannelIndex {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl CommodityChannelIndex {
    #[new]
    #[pyo3(signature = (period=20))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.import("backtide.indicators")?.getattr("CommodityChannelIndex")?;
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

/// Exponential Moving Average indicator.
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct ExponentialMovingAverage {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl ExponentialMovingAverage {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.import("backtide.indicators")?.getattr("ExponentialMovingAverage")?;
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

/// Moving Average Convergence Divergence indicator.
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
    #[pyo3(signature = (fast_period=12, slow_period=26, signal_period=9))]
    fn new(fast_period: usize, slow_period: usize, signal_period: usize) -> Self {
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
        let cls =
            py.import("backtide.indicators")?.getattr("MovingAverageConvergenceDivergence")?;
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

/// On-Balance Volume indicator.
#[pyclass(skip_from_py_object, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct OnBalanceVolume;

#[pymethods]
impl OnBalanceVolume {
    #[new]
    fn new() -> Self {
        Self
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, ())> {
        let cls = py.import("backtide.indicators")?.getattr("OnBalanceVolume")?;
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

/// Relative Strength Index indicator.
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct RelativeStrengthIndex {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl RelativeStrengthIndex {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.import("backtide.indicators")?.getattr("RelativeStrengthIndex")?;
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
            let mut gains = vec![0.0; n];
            let mut losses = vec![0.0; n];
            for i in 1..n {
                let delta = c[i] - c[i - 1];
                if delta > 0.0 {
                    gains[i] = delta;
                } else {
                    losses[i] = -delta;
                }
            }
            let avg_gain = rolling_mean(&gains, p);
            let avg_loss = rolling_mean(&losses, p);
            for i in 0..n {
                if !avg_gain[i].is_nan() && !avg_loss[i].is_nan() {
                    if avg_loss[i] == 0.0 {
                        out[i] = 100.0;
                    } else {
                        let rs = avg_gain[i] / avg_loss[i];
                        out[i] = 100.0 - (100.0 / (1.0 + rs));
                    }
                }
            }
        }
        vec![out]
    }
}

/// Simple Moving Average indicator.
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct SimpleMovingAverage {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl SimpleMovingAverage {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.import("backtide.indicators")?.getattr("SimpleMovingAverage")?;
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

/// Stochastic Oscillator indicator.
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
    #[pyo3(signature = (k_period=14, d_period=3))]
    fn new(k_period: usize, d_period: usize) -> Self {
        Self {
            k_period,
            d_period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize, usize))> {
        let cls = py.import("backtide.indicators")?.getattr("StochasticOscillator")?;
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

/// Volume-Weighted Average Price indicator.
#[pyclass(skip_from_py_object, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct VolumeWeightedAveragePrice;

#[pymethods]
impl VolumeWeightedAveragePrice {
    #[new]
    fn new() -> Self {
        Self
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, ())> {
        let cls = py.import("backtide.indicators")?.getattr("VolumeWeightedAveragePrice")?;
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

/// Weighted Moving Average indicator.
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.indicators")]
#[derive(Clone, Debug)]
pub struct WeightedMovingAverage {
    /// Look-back window length.
    period: usize,
}

#[pymethods]
impl WeightedMovingAverage {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self {
            period,
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (usize,))> {
        let cls = py.import("backtide.indicators")?.getattr("WeightedMovingAverage")?;
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
