use crate::constants::Symbol;
use crate::data::models::Bar;
use crate::errors::EngineResult;
use crate::indicators::interface::*;
use crate::indicators::traits::Indicator;
use crate::utils::python::dict_to_dataframe;
use indicatif::ProgressBar;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyAnyMethods, PyDictMethods};
use pyo3::types::{PyDict, PyList, PyTuple};
use pyo3::{Bound, Py, PyAny, PyResult, Python};
use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

/// One-time resolved compute strategy for an indicator.
pub enum IndicatorCompute {
    /// Pure-Rust path — no GIL, full parallelism with rayon.
    Builtin(Arc<dyn Fn(&[Bar]) -> Vec<Vec<f64>> + Send + Sync>),

    /// Python fallback — requires GIL, serialized.
    Python(Py<PyAny>),
}

impl IndicatorCompute {
    fn resolve(py: Python<'_>, obj: &Py<PyAny>) -> Self {
        let bound = obj.bind(py);

        macro_rules! try_builtin {
            ($($t:ty),* $(,)?) => {$(
                if let Ok(cell) = bound.cast::<$t>() {
                    // Clone out the indicator's parameters (period, etc...)
                    // so the closure owns them with no further Python touch.
                    let inst = cell.borrow().clone();
                    return Self::Builtin(Arc::new(move |bars: &[Bar]| {
                        <$t as Indicator>::compute_inner(&inst, bars)
                    }));
                }
            )*};
        }

        try_builtin!(
            AverageDirectionalIndex,
            AverageTrueRange,
            BollingerBands,
            CommodityChannelIndex,
            ExponentialMovingAverage,
            MovingAverageConvergenceDivergence,
            OnBalanceVolume,
            RelativeStrengthIndex,
            SimpleMovingAverage,
            StochasticOscillator,
            VolumeWeightedAveragePrice,
            WeightedMovingAverage,
        );

        Self::Python(obj.clone_ref(py))
    }
}

/// Convert a [`Bar`] into a Python object depending on the config.
fn bars_to_data<'py>(py: Python<'py>, bars: &[Bar]) -> PyResult<Bound<'py, PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("open", PyList::new(py, bars.iter().map(|b| b.open))?)?;
    dict.set_item("high", PyList::new(py, bars.iter().map(|b| b.high))?)?;
    dict.set_item("low", PyList::new(py, bars.iter().map(|b| b.low))?)?;
    dict.set_item("close", PyList::new(py, bars.iter().map(|b| b.close))?)?;
    dict.set_item("volume", PyList::new(py, bars.iter().map(|b| b.volume))?)?;
    dict_to_dataframe(py, &dict)
}

/// Compute all indicator values for all symbols.
pub fn compute_indicators(
    indicator_objs: &[(String, Py<PyAny>)],
    aligned: &HashMap<String, Vec<Option<Bar>>>,
    pb: Option<&ProgressBar>,
) -> EngineResult<HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>>> {
    // Build dense bar slices once, shared across all indicators.
    let symbol_bars: Vec<(&String, Vec<Bar>)> = aligned
        .iter()
        .map(|(sym, row)| {
            let bars = row.iter().map(|b| b.as_ref().cloned().unwrap_or(Bar::NAN)).collect();
            (sym, bars)
        })
        .collect();

    // Resolve all indicators to their compute strategy under a single GIL
    // acquisition, paying the downcast cost once here rather than per symbol.
    let resolved: Vec<(String, IndicatorCompute)> = Python::attach(|py| {
        indicator_objs
            .iter()
            .map(|(name, obj)| (name.clone(), IndicatorCompute::resolve(py, obj)))
            .collect()
    });

    // Parallel over indicators. Builtins also parallelize over symbols
    // internally; Python fallbacks acquire the GIL per symbol and serialize.
    let results: HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>> = resolved
        .par_iter()
        .map(|(name, compute)| {
            let per_symbol = match compute {
                IndicatorCompute::Builtin(f) => {
                    // Pure Rust — no GIL touch at all.
                    symbol_bars
                        .par_iter()
                        .filter_map(|(sym, bars)| Some(((*sym).clone(), f(bars))))
                        .collect()
                },
                IndicatorCompute::Python(obj) => {
                    // GIL-serialized; no benefit from par_iter here.
                    symbol_bars
                        .iter()
                        .filter_map(|(sym, bars)| {
                            let result = Python::attach(|py| {
                                let df = bars_to_data(py, bars)?;
                                let result = obj.bind(py).call_method1("compute", (df,))?;
                                extract_indicator_result(py, &result)
                            });

                            match result {
                                Ok(s) => Some(((*sym).clone(), s)),
                                Err(e) => {
                                    warn!("Indicator {name} failed for {sym}: {e}");
                                    None
                                },
                            }
                        })
                        .collect()
                },
            };

            if let Some(p) = pb {
                p.inc(1);
            }

            (name.clone(), per_symbol)
        })
        .collect();

    Ok(results)
}

/// Normalize any supported return type from a custom indicator.
///
/// Convert into `Vec<Vec<f64>>` shaped as `(n_series, n_points)`. All paths
/// go through numpy so we only need one extraction branch.
fn extract_indicator_result(py: Python, result: &Bound<PyAny>) -> PyResult<Vec<Vec<f64>>> {
    let np = py.import("numpy")?;

    // Convert to a float64 numpy array regardless of origin.
    let arr = if result.hasattr("to_numpy")? {
        let kwargs = PyDict::new(py);
        kwargs.set_item("allow_copy", true)?;
        result.call_method("to_numpy", (), Some(&kwargs))?
    } else {
        np.call_method1("asarray", (result,))?
    };

    // Cast to f64 in case the indicator returned ints or another float width.
    let arr = arr.call_method1("astype", ("float64",))?;

    let ndim: usize = arr.getattr("ndim")?.extract()?;
    match ndim {
        // 1-D → single series, already (n_points,).
        1 => {
            let points: Vec<f64> = arr.extract()?;
            Ok(vec![points])
        },
        // 2-D → (n_points, n_series) by DataFrame convention.
        // Transpose to (n_series, n_points) with `.T` before extracting.
        2 => {
            let transposed = arr.getattr("T")?;
            let series: Vec<Vec<f64>> = transposed.extract()?;
            Ok(series)
        },
        other => Err(PyValueError::new_err(format!(
            "indicator returned a {other}-D array. Expected 1-D or 2-D"
        ))),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Interface utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Extract the indicator acronym from a Python instance.
pub fn indicator_acronym_from_py(indicator: &Bound<'_, PyAny>) -> PyResult<String> {
    let cls = indicator.get_type();
    if let Ok(acr) = cls.getattr("acronym").and_then(|v| v.extract::<String>()) {
        if !acr.is_empty() {
            return Ok(acr);
        }
    }

    cls.getattr("__name__")?.extract::<String>()
}

/// Extract the indicator arguments from a Python instance.
pub fn indicator_args_from_py(indicator: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if let Ok(reduced) = indicator.call_method0("__reduce__") {
        if let Ok(reduced_tuple) = reduced.cast::<PyTuple>() {
            if reduced_tuple.len()? >= 2 {
                let args_obj = reduced_tuple.get_item(1)?;
                let mut args = Vec::new();

                if let Ok(iter) = args_obj.try_iter() {
                    for item in iter {
                        args.push(item?.str()?.extract::<String>()?);
                    }
                }

                if !args.is_empty() {
                    return Ok(args);
                }
            }
        }
    }

    if let Ok(dict_obj) = indicator.getattr("__dict__") {
        if let Ok(dict) = dict_obj.cast::<PyDict>() {
            let mut args = Vec::with_capacity(dict.len());
            for (_k, v) in dict.iter() {
                args.push(v.str()?.extract::<String>()?);
            }

            if !args.is_empty() {
                return Ok(args);
            }
        }
    }

    Ok(Vec::new())
}

/// Determine the deterministic name for an indicator.
pub fn indicator_deterministic_name(acronym: &str, args: &[String]) -> String {
    let args_str = args.join("_");
    let sanitized = args_str.replace('.', "p").replace('-', "n").replace(' ', "");

    if sanitized.is_empty() {
        acronym.to_owned()
    } else {
        format!("{acronym}_{sanitized}")
    }
}

/// Extract `(open, high, low, close, volume)` arrays from a bar slice.
pub fn extract_ohlcv_from_bars(bars: &[Bar]) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut o = Vec::with_capacity(bars.len());
    let mut h = Vec::with_capacity(bars.len());
    let mut l = Vec::with_capacity(bars.len());
    let mut c = Vec::with_capacity(bars.len());
    let mut v = Vec::with_capacity(bars.len());

    for b in bars {
        o.push(b.open);
        h.push(b.high);
        l.push(b.low);
        c.push(b.close);
        v.push(b.volume);
    }

    (o, h, l, c, v)
}

/// Compute a simple rolling mean over `data` with the given `period`.
///
/// The first `period - 1` elements are [`f64::NAN`]. Uses an incremental
/// sum for O(n) performance. Windows containing non-finite values remain
/// `NAN`, then recover once a full finite window is available.
pub fn rolling_mean(data: &[f64], period: usize) -> Vec<f64> {
    let n = data.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || n < period {
        return out;
    }
    let mut sum = 0.0;
    let mut finite = 0usize;
    for i in 0..n {
        if data[i].is_finite() {
            sum += data[i];
            finite += 1;
        }
        if i >= period {
            let old = data[i - period];
            if old.is_finite() {
                sum -= old;
                finite -= 1;
            }
        }
        if i + 1 >= period && finite == period {
            out[i] = sum / period as f64;
        }
    }
    out
}

/// Compute Wilder's smoothing (a.k.a. Wilder's MA / RMA) over `data`.
///
/// Wilder's smoothing is an EMA with `α = 1 / period`, seeded with the
/// SMA of the first `period` values. It is the smoothing used by the
/// textbook definitions of [`RelativeStrengthIndex`], [`AverageTrueRange`]
/// and [`AverageDirectionalIndex`] — distinct from the standard EMA
/// (α = 2 / (n + 1)) used by [`ExponentialMovingAverage`] / [`MovingAverageConvergenceDivergence`].
///
/// Handles leading non-finite values by seeding at the first index whose
/// look-back window of size `period` is fully finite. Subsequent
/// non-finite samples are skipped (the previous value carries through).
pub fn wilder_smooth(data: &[f64], period: usize) -> Vec<f64> {
    let n = data.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || n < period {
        return out;
    }
    let alpha = 1.0 / period as f64;
    // Seed at the first index i >= period-1 whose look-back window
    // [i+1-period..=i] is fully finite.
    let mut seeded_at: Option<usize> = None;
    for i in (period - 1)..n {
        let window = &data[i + 1 - period..=i];
        if window.iter().all(|x| x.is_finite()) {
            out[i] = window.iter().sum::<f64>() / period as f64;
            seeded_at = Some(i);
            break;
        }
    }
    if let Some(start) = seeded_at {
        for i in (start + 1)..n {
            let prev = out[i - 1];
            if !prev.is_finite() || !data[i].is_finite() {
                continue;
            }
            out[i] = prev + alpha * (data[i] - prev);
        }
    }
    out
}

/// Compute an exponential weighted moving average with the given `span`.
///
/// Uses the standard smoothing factor `α = 2 / (span + 1)`. For Wilder's
/// smoothing (α = 1 / period), use [`wilder_smooth`] instead.
pub fn ewm(data: &[f64], span: usize) -> Vec<f64> {
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
///
/// Implements an incremental sum / sum-of-squares algorithm for O(n)
/// performance instead of the naïve O(n × period) double loop. Windows
/// containing any non-finite value produce NAN; the window recovers
/// as soon as all values in the window are finite.
pub fn rolling_std(data: &[f64], period: usize) -> Vec<f64> {
    let n = data.len();
    let mut out = vec![f64::NAN; n];
    if period < 2 || n < period {
        return out;
    }
    let p = period as f64;

    // Seed: compute sum and sum-of-squares for the first full window.
    let mut sum = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    let mut finite_count = 0_usize;
    for item in data.iter().take(period) {
        if item.is_finite() {
            sum += item;
            sum_sq += item * item;
            finite_count += 1;
        }
    }
    if finite_count == period {
        let mean = sum / p;
        let var = (sum_sq - p * mean * mean) / (p - 1.0);
        out[period - 1] = var.max(0.0).sqrt();
    }

    // Slide the window one step at a time.
    for i in period..n {
        let new = data[i];
        let old = data[i - period];
        // Update finite count.
        if old.is_finite() {
            sum -= old;
            sum_sq -= old * old;
            finite_count -= 1;
        }
        if new.is_finite() {
            sum += new;
            sum_sq += new * new;
            finite_count += 1;
        }
        if finite_count == period {
            let mean = sum / p;
            let var = (sum_sq - p * mean * mean) / (p - 1.0);
            out[i] = var.max(0.0).sqrt();
        }
    }
    out
}

/// Compute the True Range for each bar.
///
/// TR = max(high − low, |high − prev_close|, |low − prev_close|).
/// The first element uses `high[0] − low[0]` since there is no previous close.
pub fn true_range(high: &[f64], low: &[f64], close: &[f64]) -> Vec<f64> {
    let n = high.len();
    let mut tr = vec![f64::NAN; n];
    if n == 0 {
        return tr;
    }
    if high[0].is_finite() && low[0].is_finite() {
        tr[0] = high[0] - low[0];
    }
    for i in 1..n {
        if !high[i].is_finite() || !low[i].is_finite() {
            continue;
        }
        let hl = high[i] - low[i];
        tr[i] = if close[i - 1].is_finite() {
            let hc = (high[i] - close[i - 1]).abs();
            let lc = (low[i] - close[i - 1]).abs();
            hl.max(hc).max(lc)
        } else {
            hl
        };
    }
    tr
}
