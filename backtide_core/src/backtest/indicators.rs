use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::data::models::bar::Bar;

/// Trait for all built-in indicators.
pub trait Indicator {
    fn acronym(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn calculate_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions
// ─────────────────────────────────────────────────────────────────────────────

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

fn rolling_std(data: &[f64], period: usize) -> Vec<f64> {
    let n = data.len();
    let mut out = vec![f64::NAN; n];
    if period < 2 || n < period {
        return out;
    }
    for i in (period - 1)..n {
        let window = &data[i + 1 - period..=i];
        let mean: f64 = window.iter().sum::<f64>() / period as f64;
        let var: f64 =
            window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (period - 1) as f64;
        out[i] = var.sqrt();
    }
    out
}

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
fn extract_ohlcv(
    df: &Bound<'_, PyAny>,
) -> PyResult<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>)> {
    let extract_col = |name: &str| -> PyResult<Vec<f64>> {
        let col = df.get_item(name)?;
        let vals: Vec<f64> = col.call_method0("to_numpy")?.extract()?;
        Ok(vals)
    };
    Ok((
        extract_col("open")?,
        extract_col("high")?,
        extract_col("low")?,
        extract_col("close")?,
        extract_col("volume").unwrap_or_else(|_| vec![0.0; df.len().unwrap_or(0)]),
    ))
}

/// Convert indicator outputs to a numpy array (1D or 2D).
fn to_np_array<'py>(py: Python<'py>, rows: Vec<Vec<f64>>) -> PyResult<Bound<'py, PyAny>> {
    let np = py.import("numpy")?;
    if rows.len() == 1 {
        np.call_method1("array", (rows.into_iter().next().unwrap(),))
    } else {
        np.call_method1("array", (rows,))
    }
}

/// Shared pymethods macro for all indicator structs.
/// The struct must already have `#[pymethods]` with `new` and `__reduce__`.
/// This macro adds `acronym`, `name`, `description`, `calculate`, `__str__`, `__repr__`.
///
/// Usage: Put this inside the existing `#[pymethods] impl` block as method definitions.
macro_rules! indicator_pymethods {
    ($ty:ident) => {
        #[pymethods]
        impl $ty {
        /// Short ticker-style acronym (e.g. ``"SMA"``).
        #[getter]
        fn acronym(&self) -> &'static str {
            Indicator::acronym(self)
        }

        /// Human-readable name (e.g. ``"Simple Moving Average"``).
        #[getter]
        fn name(&self) -> &'static str {
            Indicator::name(self)
        }

        /// One-sentence explanation of what the indicator measures.
        #[getter]
        fn description(&self) -> &'static str {
            Indicator::description(self)
        }

        /// Compute the indicator on a pandas DataFrame.
        fn calculate<'py>(
            &self,
            py: Python<'py>,
            df: &Bound<'py, PyAny>,
        ) -> PyResult<Bound<'py, PyAny>> {
            let (o, h, l, c, v) = extract_ohlcv(df)?;
            let bars: Vec<Bar> = (0..c.len()).map(|i| Bar {
                open_ts: 0, close_ts: 0, open_ts_exchange: 0,
                open: o[i], high: h[i], low: l[i], close: c[i],
                adj_close: c[i], volume: v[i], n_trades: None,
            }).collect();
            to_np_array(py, self.calculate_inner(&bars))
        }

        fn __str__(&self) -> &'static str {
            Indicator::acronym(self)
        }

        fn __repr__(&self) -> String {
            format!("{}()", Indicator::acronym(self))
        }
        }
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Indicator structs
// ─────────────────────────────────────────────────────────────────────────────

/// Simple Moving Average indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct SimpleMovingAverage {
    #[pyo3(get)]
    period: usize,
}

#[pymethods]
impl SimpleMovingAverage {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self { period }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (usize,)) {
        (py.get_type::<Self>(), (self.period,))
    }
}


impl Indicator for SimpleMovingAverage {
    fn acronym(&self) -> &'static str {
        "SMA"
    }

    fn name(&self) -> &'static str {
        "Simple Moving Average"
    }
    fn description(&self) -> &'static str {
        "Arithmetic mean of the last N closing prices, used to smooth short-term fluctuations and identify trends."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        _h: &[f64],
        _l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
        vec![rolling_mean(c, self.period)]
    }
}

indicator_pymethods!(SimpleMovingAverage);

/// Exponential Moving Average indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct ExponentialMovingAverage {
    #[pyo3(get)]
    period: usize,
}

#[pymethods]
impl ExponentialMovingAverage {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self { period }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (usize,)) {
        (py.get_type::<Self>(), (self.period,))
    }
}


impl Indicator for ExponentialMovingAverage {
    fn acronym(&self) -> &'static str {
        "EMA"
    }

    fn name(&self) -> &'static str {
        "Exponential Moving Average"
    }
    fn description(&self) -> &'static str {
        "Weighted moving average that gives more weight to recent prices, reacting faster to price changes than SMA."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        _h: &[f64],
        _l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
        vec![ewm(c, self.period)]
    }
}

indicator_pymethods!(ExponentialMovingAverage);

/// Weighted Moving Average indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct WeightedMovingAverage {
    #[pyo3(get)]
    period: usize,
}

#[pymethods]
impl WeightedMovingAverage {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self { period }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (usize,)) {
        (py.get_type::<Self>(), (self.period,))
    }
}


impl Indicator for WeightedMovingAverage {
    fn acronym(&self) -> &'static str {
        "WMA"
    }

    fn name(&self) -> &'static str {
        "Weighted Moving Average"
    }
    fn description(&self) -> &'static str {
        "Moving average where each price is multiplied by a linearly decreasing weight, emphasizing recent data."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        _h: &[f64],
        _l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
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

indicator_pymethods!(WeightedMovingAverage);

/// Relative Strength Index indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct RelativeStrengthIndex {
    #[pyo3(get)]
    period: usize,
}

#[pymethods]
impl RelativeStrengthIndex {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self { period }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (usize,)) {
        (py.get_type::<Self>(), (self.period,))
    }
}


impl Indicator for RelativeStrengthIndex {
    fn acronym(&self) -> &'static str {
        "RSI"
    }

    fn name(&self) -> &'static str {
        "Relative Strength Index"
    }
    fn description(&self) -> &'static str {
        "Momentum oscillator (0\u{2013}100) measuring the speed and magnitude of recent price changes to identify overbought/oversold conditions."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        _h: &[f64],
        _l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
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

indicator_pymethods!(RelativeStrengthIndex);

/// Moving Average Convergence Divergence indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct MovingAverageConvergenceDivergence {
    #[pyo3(get)]
    fast_period: usize,
    #[pyo3(get)]
    slow_period: usize,
    #[pyo3(get)]
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
    ) -> (Bound<'py, PyType>, (usize, usize, usize)) {
        (
            py.get_type::<Self>(),
            (self.fast_period, self.slow_period, self.signal_period),
        )
    }
}


impl Indicator for MovingAverageConvergenceDivergence {
    fn acronym(&self) -> &'static str {
        "MACD"
    }

    fn name(&self) -> &'static str {
        "Moving Avg. Convergence Divergence"
    }
    fn description(&self) -> &'static str {
        "Trend-following momentum indicator showing the relationship between two exponential moving averages of closing prices."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        _h: &[f64],
        _l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
        let fast = ewm(c, self.fast_period);
        let slow = ewm(c, self.slow_period);
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

indicator_pymethods!(MovingAverageConvergenceDivergence);

/// Bollinger Bands indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct BollingerBands {
    #[pyo3(get)]
    period: usize,
    #[pyo3(get)]
    std_dev: f64,
}

#[pymethods]
impl BollingerBands {
    #[new]
    #[pyo3(signature = (period=20, std_dev=2.0))]
    fn new(period: usize, std_dev: f64) -> Self {
        Self { period, std_dev }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (usize, f64)) {
        (py.get_type::<Self>(), (self.period, self.std_dev))
    }
}


impl Indicator for BollingerBands {
    fn acronym(&self) -> &'static str {
        "BB"
    }

    fn name(&self) -> &'static str {
        "Bollinger Bands"
    }
    fn description(&self) -> &'static str {
        "Volatility bands placed above and below a moving average, widening during high volatility and narrowing during low volatility."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        _h: &[f64],
        _l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
        let mid = rolling_mean(c, self.period);
        let std = rolling_std(c, self.period);
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

indicator_pymethods!(BollingerBands);

/// Average True Range indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct AverageTrueRange {
    #[pyo3(get)]
    period: usize,
}

#[pymethods]
impl AverageTrueRange {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self { period }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (usize,)) {
        (py.get_type::<Self>(), (self.period,))
    }
}


impl Indicator for AverageTrueRange {
    fn acronym(&self) -> &'static str {
        "ATR"
    }

    fn name(&self) -> &'static str {
        "Average True Range"
    }
    fn description(&self) -> &'static str {
        "Measures market volatility by calculating the average range between high and low prices over a period."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        h: &[f64],
        l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
        let tr = true_range(h, l, c);
        vec![rolling_mean(&tr, self.period)]
    }
}

indicator_pymethods!(AverageTrueRange);

/// On-Balance Volume indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct OnBalanceVolume;

#[pymethods]
impl OnBalanceVolume {
    #[new]
    fn new() -> Self {
        Self
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, ()) {
        (py.get_type::<Self>(), ())
    }
}


impl Indicator for OnBalanceVolume {
    fn acronym(&self) -> &'static str {
        "OBV"
    }

    fn name(&self) -> &'static str {
        "On-Balance Volume"
    }
    fn description(&self) -> &'static str {
        "Cumulative volume indicator that adds volume on up days and subtracts it on down days to confirm price trends."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        _h: &[f64],
        _l: &[f64],
        c: &[f64],
        v: &[f64],
    ) -> Vec<Vec<f64>> {
        let n = c.len();
        let mut obv = vec![0.0; n];
        for i in 1..n {
            obv[i] = if c[i] > c[i - 1] {
                obv[i - 1] + v[i]
            } else if c[i] < c[i - 1] {
                obv[i - 1] - v[i]
            } else {
                obv[i - 1]
            };
        }
        vec![obv]
    }
}

indicator_pymethods!(OnBalanceVolume);

/// Volume-Weighted Average Price indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct VolumeWeightedAveragePrice;

#[pymethods]
impl VolumeWeightedAveragePrice {
    #[new]
    fn new() -> Self {
        Self
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, ()) {
        (py.get_type::<Self>(), ())
    }
}


impl Indicator for VolumeWeightedAveragePrice {
    fn acronym(&self) -> &'static str {
        "VWAP"
    }

    fn name(&self) -> &'static str {
        "Volume-Weighted Average Price"
    }
    fn description(&self) -> &'static str {
        "Average price weighted by volume, used as a benchmark for intraday trading quality."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        h: &[f64],
        l: &[f64],
        c: &[f64],
        v: &[f64],
    ) -> Vec<Vec<f64>> {
        let n = c.len();
        let mut vwap = vec![f64::NAN; n];
        let mut cum_tp_vol = 0.0;
        let mut cum_vol = 0.0;
        for i in 0..n {
            let tp = (h[i] + l[i] + c[i]) / 3.0;
            cum_tp_vol += tp * v[i];
            cum_vol += v[i];
            vwap[i] = if cum_vol > 0.0 {
                cum_tp_vol / cum_vol
            } else {
                f64::NAN
            };
        }
        vec![vwap]
    }
}

indicator_pymethods!(VolumeWeightedAveragePrice);

/// Stochastic Oscillator indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct StochasticOscillator {
    #[pyo3(get)]
    k_period: usize,
    #[pyo3(get)]
    d_period: usize,
}

#[pymethods]
impl StochasticOscillator {
    #[new]
    #[pyo3(signature = (k_period=14, d_period=3))]
    fn new(k_period: usize, d_period: usize) -> Self {
        Self { k_period, d_period }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (usize, usize)) {
        (py.get_type::<Self>(), (self.k_period, self.d_period))
    }
}


impl Indicator for StochasticOscillator {
    fn acronym(&self) -> &'static str {
        "STOCH"
    }

    fn name(&self) -> &'static str {
        "Stochastic Oscillator"
    }
    fn description(&self) -> &'static str {
        "Compares a closing price to a range of prices over a period, generating overbought/oversold signals."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        h: &[f64],
        l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
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

indicator_pymethods!(StochasticOscillator);

/// Commodity Channel Index indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct CommodityChannelIndex {
    #[pyo3(get)]
    period: usize,
}

#[pymethods]
impl CommodityChannelIndex {
    #[new]
    #[pyo3(signature = (period=20))]
    fn new(period: usize) -> Self {
        Self { period }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (usize,)) {
        (py.get_type::<Self>(), (self.period,))
    }
}


impl Indicator for CommodityChannelIndex {
    fn acronym(&self) -> &'static str {
        "CCI"
    }

    fn name(&self) -> &'static str {
        "Commodity Channel Index"
    }
    fn description(&self) -> &'static str {
        "Measures a price's deviation from its statistical mean, identifying cyclical trends in the data."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        h: &[f64],
        l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
        let n = c.len();
        let p = self.period;
        let tp: Vec<f64> = (0..n).map(|i| (h[i] + l[i] + c[i]) / 3.0).collect();
        let ma = rolling_mean(&tp, p);
        let mut out = vec![f64::NAN; n];
        if n >= p && p > 0 {
            for i in (p - 1)..n {
                let window = &tp[i + 1 - p..=i];
                let mean = ma[i];
                let md: f64 =
                    window.iter().map(|x| (x - mean).abs()).sum::<f64>() / p as f64;
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

indicator_pymethods!(CommodityChannelIndex);

/// Average Directional Index indicator.
#[pyclass(skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct AverageDirectionalIndex {
    #[pyo3(get)]
    period: usize,
}

#[pymethods]
impl AverageDirectionalIndex {
    #[new]
    #[pyo3(signature = (period=14))]
    fn new(period: usize) -> Self {
        Self { period }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (usize,)) {
        (py.get_type::<Self>(), (self.period,))
    }
}


impl Indicator for AverageDirectionalIndex {
    fn acronym(&self) -> &'static str {
        "ADX"
    }
    fn name(&self) -> &'static str {
        "Average Directional Index"
    }
    fn description(&self) -> &'static str {
        "Quantifies trend strength (0\u{2013}100) regardless of direction, helping distinguish trending from ranging markets."
    }
    fn calculate_inner(
        &self,
        _o: &[f64],
        h: &[f64],
        l: &[f64],
        c: &[f64],
        _v: &[f64],
    ) -> Vec<Vec<f64>> {
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
            plus_dm[i] = if up > down && up > 0.0 { up } else { 0.0 };
            minus_dm[i] = if down > up && down > 0.0 {
                down
            } else {
                0.0
            };
        }

        let tr = true_range(h, l, c);
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

indicator_pymethods!(AverageDirectionalIndex);

// ─────────────────────────────────────────────────────────────────────────────
// List all predefined indicators (default params)
// ─────────────────────────────────────────────────────────────────────────────

/// Return a list of all predefined indicator instances with default parameters.
#[pyfunction]
pub fn list_indicators(py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
    Ok(vec![
        Py::new(py, SimpleMovingAverage::new(14))?.into_any(),
        Py::new(py, ExponentialMovingAverage::new(14))?.into_any(),
        Py::new(py, WeightedMovingAverage::new(14))?.into_any(),
        Py::new(py, RelativeStrengthIndex::new(14))?.into_any(),
        Py::new(py, MovingAverageConvergenceDivergence::new(12, 26, 9))?.into_any(),
        Py::new(py, BollingerBands::new(20, 2.0))?.into_any(),
        Py::new(py, AverageTrueRange::new(14))?.into_any(),
        Py::new(py, OnBalanceVolume::new())?.into_any(),
        Py::new(py, VolumeWeightedAveragePrice::new())?.into_any(),
        Py::new(py, StochasticOscillator::new(14, 3))?.into_any(),
        Py::new(py, CommodityChannelIndex::new(20))?.into_any(),
        Py::new(py, AverageDirectionalIndex::new(14))?.into_any(),
    ])
}

