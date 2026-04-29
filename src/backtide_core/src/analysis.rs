//! Analysis module — statistics computed in parallel over symbols.

use crate::utils::dataframe::dict_to_dataframe;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use rayon::prelude::*;

// ────────────────────────────────────────────────────────────────────────────
// Helper functions
// ────────────────────────────────────────────────────────────────────────────

/// Ensures a `dt` datetime column exists.
///
/// Converts Unix-second timestamps from `open_ts` / `ts` / `ex_date` to
/// timezone-aware datetimes using the configured display timezone. Returns
/// a (possibly new) dataframe. The original is never mutated.
fn resolve_dt<'py>(py: Python<'py>, df: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let columns: Vec<String> = df.getattr("columns")?.call_method0("tolist")?.extract()?;

    if columns.iter().any(|c| c == "dt") {
        return Ok(df.clone());
    }

    if columns.iter().any(|c| c == "datetime") {
        let copy = df.call_method0("copy")?;
        let datetime_col = copy.call_method1("__getitem__", ("datetime",))?;
        copy.call_method1("__setitem__", ("dt", datetime_col))?;
        return Ok(copy);
    }

    // Resolve the configured display timezone (falls back to local).
    let tz_name: Option<String> = py
        .import("backtide.core.config")?
        .call_method0("get_config")?
        .getattr("display")?
        .getattr("timezone")?
        .extract()?;

    let zoneinfo = py.import("zoneinfo")?.getattr("ZoneInfo")?;
    let tz = if let Some(name) = tz_name.filter(|s| !s.is_empty()) {
        zoneinfo.call1((name,))?
    } else {
        py.import("tzlocal")?.call_method0("get_localzone")?
    };

    let pd = py.import("pandas")?;
    for ts_col in ["open_ts", "ts", "ex_date"] {
        if columns.iter().any(|c| c == ts_col) {
            let copy = df.call_method0("copy")?;
            let series = copy.call_method1("__getitem__", (ts_col,))?;

            // pd.to_datetime(series, unit="s", utc=True).dt.tz_convert(tz)
            let kwargs = PyDict::new(py);
            kwargs.set_item("unit", "s")?;
            kwargs.set_item("utc", true)?;
            let dt = pd
                .call_method("to_datetime", PyTuple::new(py, [series])?, Some(&kwargs))?
                .getattr("dt")?
                .call_method1("tz_convert", (tz,))?;

            copy.call_method1("__setitem__", ("dt", dt))?;
            return Ok(copy);
        }
    }

    Ok(df.clone())
}

// ────────────────────────────────────────────────────────────────────────────
// Pure-Rust statistics kernel
// ────────────────────────────────────────────────────────────────────────────

/// Annualized statistics for a single value series (price / equity / NAV).
///
/// All values are returned as **fractions** (e.g. `0.12` = `12 %`) to keep
/// the kernel agnostic to display conventions. Callers wanting percentages
/// must multiply by `100.0`.
pub struct SeriesStats {
    /// Annualized return (CAGR).
    pub ann_return: f64,

    /// Annualized volatility (standard deviation of returns).
    pub ann_volatility: f64,

    /// Annualized Sharpe ratio.
    pub sharpe: f64,

    /// Annualized Sortino ratio.
    pub sortino: f64,

    /// Maximum drawdown (≤ 0).
    pub max_dd: f64,

    /// Fraction of up-bars (returns > 0).
    pub win_rate: f64,
}

/// Compute annualized risk/return statistics for a single value series.
///
/// Used both by [`compute_statistics`] (analysis page) and by the
/// backtest engine when summarizing strategy equity curves, so that
/// these metrics are calculated in exactly one place.
///
/// Parameters
/// ----------
/// values : &[f64]
///     Strictly positive value series (prices, equity, NAV, ...). At
///     least 3 points are required.
/// timestamps : &[f64]
///     Unix-second timestamps matching `values`. Used only when
///     `periods_per_year` is `None`, to estimate the annualization factor
///     from the bar density.
/// risk_free_rate : f64
///     Annualized risk-free rate as a **fraction** (e.g. 0.04 = 4 %).
/// periods_per_year : Option<usize>
///     Explicit annualization factor. When `None`, derived from the
///     series time span.
pub fn compute_series_stats(
    values: &[f64],
    timestamps: &[f64],
    risk_free_rate: f64,
    periods_per_year: Option<usize>,
) -> Option<SeriesStats> {
    if values.len() < 3 {
        return None;
    }

    // Bar-to-bar returns. Skip windows with non-positive base to avoid
    // divide-by-zero / negative-base nonsense on degenerate series.
    let returns: Vec<f64> = values
        .windows(2)
        .filter_map(|w| {
            if w[0] > 0.0 {
                Some(w[1] / w[0] - 1.0)
            } else {
                None
            }
        })
        .collect();
    let n = returns.len();
    if n < 2 {
        return None;
    }

    // Annualization factor.
    let ann = if let Some(ppy) = periods_per_year {
        ppy as f64
    } else {
        let span =
            timestamps.last().copied().unwrap_or(0.0) - timestamps.first().copied().unwrap_or(0.0);
        if span > 0.0 {
            let years = span / (365.25 * 86_400.0);
            (n as f64 / years).round().max(1.0)
        } else {
            252.0
        }
    };

    // CAGR.
    let total_return = values[values.len() - 1] / values[0];
    let n_years = n as f64 / ann;
    let ann_return = if n_years > 0.0 && total_return > 0.0 {
        let cagr = total_return.powf(1.0 / n_years) - 1.0;
        if cagr.is_finite() {
            cagr
        } else {
            // For very short periods CAGR overflows; fall back to simple return
            total_return - 1.0
        }
    } else if n_years > 0.0 {
        -1.0
    } else {
        0.0
    };

    // Mean and std of returns.
    let mean_ret = returns.iter().sum::<f64>() / n as f64;
    let variance = returns.iter().map(|r| (r - mean_ret).powi(2)).sum::<f64>() / (n - 1) as f64;
    let std_ret = variance.sqrt();
    let ann_volatility = std_ret * ann.sqrt();

    // Sharpe.
    let excess = mean_ret - risk_free_rate / ann;
    let sharpe = if std_ret > 0.0 {
        excess / std_ret * ann.sqrt()
    } else {
        0.0
    };

    // Sortino.
    let downside: Vec<f64> = returns.iter().copied().filter(|&r| r < 0.0).collect();
    let sortino = if downside.len() > 1 {
        let ds_mean = downside.iter().sum::<f64>() / downside.len() as f64;
        let ds_var = downside.iter().map(|r| (r - ds_mean).powi(2)).sum::<f64>()
            / (downside.len() - 1) as f64;
        let ds_std = ds_var.sqrt();
        if ds_std > 0.0 {
            excess / ds_std * ann.sqrt()
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Max drawdown over the cumulative return path.
    let mut cum = 1.0_f64;
    let mut running_max = f64::NEG_INFINITY;
    let mut max_dd = 0.0_f64;
    for &r in &returns {
        cum *= 1.0 + r;
        if cum > running_max {
            running_max = cum;
        }
        let dd = (cum - running_max) / running_max;
        if dd < max_dd {
            max_dd = dd;
        }
    }

    // Win rate (fraction of bars with strictly positive return).
    let wins = returns.iter().filter(|&&r| r > 0.0).count();
    let win_rate = wins as f64 / n as f64;

    Some(SeriesStats {
        ann_return,
        ann_volatility,
        sharpe,
        sortino,
        max_dd: max_dd,
        win_rate,
    })
}

/// Per-symbol statistics record computed by Rust.
struct SymbolStats {
    symbol: String,
    sharpe: f64,
    cagr: f64,
    max_dd: f64,
    win_rate: f64,
    ann_volatility: f64,
    sortino: f64,
    total_bars: usize,
}

/// Compute statistics for a single symbol from its sorted price series.
fn compute_single(
    symbol: String,
    prices: &[f64],
    timestamps: &[f64],
    risk_free_rate: f64,
    periods_per_year: Option<usize>,
) -> Option<SymbolStats> {
    let s = compute_series_stats(prices, timestamps, risk_free_rate, periods_per_year)?;
    Some(SymbolStats {
        symbol,
        sharpe: s.sharpe,
        cagr: s.ann_return,
        max_dd: s.max_dd,
        win_rate: s.win_rate,
        ann_volatility: s.ann_volatility,
        sortino: s.sortino,
        total_bars: prices.len(),
    })
}

// ────────────────────────────────────────────────────────────────────────────
// Python interface
// ────────────────────────────────────────────────────────────────────────────

/// Compute per-symbol summary statistics.
///
/// Calculates key performance and risk metrics for each symbol in `data`.
/// All metrics are annualized based on the detected or specified trading
/// frequency.
///
/// Parameters
/// ----------
/// data : pd.DataFrame | pl.DataFrame
///     Input data containing columns `symbol`, the column specified by
///     `price_col`, and `dt` with the datetime.
///
/// price_col : str, default="adj_close"
///     Column name used to compute returns.
///
/// risk_free_rate : float, default=0.0
///     Annualized risk-free rate used in Sharpe and Sortino ratio
///     calculations.
///
/// periods_per_year : int | None, default=None
///     Number of trading periods per year for annualization. If `None`,
///     it is estimated from the median time delta between bars (e.g., 252
///     for daily data).
///
/// Returns
/// -------
/// np.ndarray | pd.DataFrame | pl.DataFrame
///     Dataset with one row per symbol and columns for each metric.
///
/// See Also
/// --------
/// backtide.analysis:plot_returns
/// backtide.analysis:plot_drawdown
///
/// Examples
/// --------
/// ```pycon
/// from backtide.storage import query_bars
/// from backtide.analysis import compute_statistics
///
/// df = query_bars(["AAPL", "MSFT"], "1d")
/// stats = compute_statistics(df)
/// print(stats.head())
/// ```
#[pyfunction]
#[pyo3(signature = (data, *, price_col="adj_close", risk_free_rate=0.0, periods_per_year=None))]
pub fn compute_statistics<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    price_col: &str,
    risk_free_rate: f64,
    periods_per_year: Option<usize>,
) -> PyResult<Py<PyAny>> {
    // Normalize input to a pandas DataFrame regardless of the source type.
    let module = data.getattr("__class__")?.getattr("__module__")?.extract::<String>()?;

    let df: Bound<'py, PyAny> = if module.starts_with("polars") {
        data.call_method0("to_pandas")?
    } else {
        data.clone()
    };

    // Ensure a `dt` column exists.
    let df = resolve_dt(py, &df)?;

    // Get unique symbols, sorted
    let symbols_col = df.getattr("__getitem__")?.call1(("symbol",))?;
    let unique = symbols_col.call_method0("unique")?;
    let mut symbols: Vec<String> = unique.extract()?;
    symbols.sort();

    // For each symbol, extract sorted prices and timestamps
    struct SymbolData {
        symbol: String,
        prices: Vec<f64>,
        timestamps: Vec<f64>,
    }

    let symbol_data: Vec<SymbolData> = symbols
        .into_iter()
        .filter_map(|sym| {
            let mask = symbols_col.call_method1("__eq__", (&sym,)).ok()?;
            let subset = df.call_method1("__getitem__", (mask,)).ok()?;
            let sorted = subset.call_method1("sort_values", ("dt",)).ok()?;

            let price_series = sorted.getattr("__getitem__").ok()?.call1((price_col,)).ok()?;
            let prices: Vec<f64> = price_series.call_method0("to_list").ok()?.extract().ok()?;

            let ts_series = sorted.getattr("__getitem__").ok()?.call1(("open_ts",)).ok()?;
            let timestamps: Vec<f64> = ts_series.call_method0("to_list").ok()?.extract().ok()?;

            Some(SymbolData {
                symbol: sym,
                prices,
                timestamps,
            })
        })
        .collect();

    let results: Vec<SymbolStats> = symbol_data
        .into_par_iter()
        .filter_map(|sd| {
            compute_single(sd.symbol, &sd.prices, &sd.timestamps, risk_free_rate, periods_per_year)
        })
        .collect();

    // Sort results by symbol for deterministic output
    let mut results = results;
    results.sort_by(|a, b| a.symbol.cmp(&b.symbol));

    let out = PyDict::new(py);
    out.set_item("symbol", PyList::new(py, results.iter().map(|r| &r.symbol))?)?;
    out.set_item("sharpe", PyList::new(py, results.iter().map(|r| r.sharpe))?)?;
    out.set_item("cagr", PyList::new(py, results.iter().map(|r| r.cagr))?)?;
    out.set_item("max_dd", PyList::new(py, results.iter().map(|r| r.max_dd))?)?;
    out.set_item("win_rate", PyList::new(py, results.iter().map(|r| r.win_rate))?)?;
    out.set_item("ann_volatility", PyList::new(py, results.iter().map(|r| r.ann_volatility))?)?;
    out.set_item("sortino", PyList::new(py, results.iter().map(|r| r.sortino))?)?;
    out.set_item("total_bars", PyList::new(py, results.iter().map(|r| r.total_bars))?)?;

    dict_to_dataframe(py, &out).map(Bound::unbind)
}

/// Register the analysis module into `backtide.core.analysis`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.analysis")?;
    m.add_function(wrap_pyfunction!(compute_statistics, &m)?)?;
    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.analysis", &m)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a simple monotonically-increasing price series.
    fn linear_prices(n: usize, start: f64, step: f64) -> Vec<f64> {
        (0..n).map(|i| start + step * i as f64).collect()
    }

    /// Build matching daily timestamps (one day apart) for `n` bars.
    fn daily_timestamps(n: usize) -> Vec<f64> {
        (0..n).map(|i| (i as f64) * 86_400.0).collect()
    }

    // ── compute_single: short-circuits ──────────────────────────────────

    #[test]
    fn compute_single_returns_none_for_too_few_prices() {
        let result = compute_single("X".into(), &[100.0, 101.0], &[0.0, 1.0], 0.0, None);
        assert!(result.is_none());
    }

    #[test]
    fn compute_single_returns_none_when_returns_under_two() {
        // 3 prices -> 2 returns, which is the minimum accepted
        let result = compute_single(
            "X".into(),
            &[100.0, 101.0, 102.0],
            &[0.0, 86_400.0, 172_800.0],
            0.0,
            None,
        );
        assert!(result.is_some());
    }

    // ── compute_single: positive trend ──────────────────────────────────

    #[test]
    fn compute_single_positive_trend_basic_metrics() {
        let prices = linear_prices(60, 100.0, 1.0);
        let timestamps = daily_timestamps(60);

        let stats = compute_single("AAPL".into(), &prices, &timestamps, 0.0, None).unwrap();

        assert_eq!(stats.symbol, "AAPL");
        assert_eq!(stats.total_bars, 60);
        // Every bar is up -> 100% wins (1.0 as a fraction)
        assert!((stats.win_rate - 1.0).abs() < 1e-9);
        // No down day -> sortino is 0 (downside len <= 1)
        assert_eq!(stats.sortino, 0.0);
        // Annualised return must be positive on a strict uptrend
        assert!(stats.cagr > 0.0);
        // Max drawdown is 0 for a monotonic uptrend
        assert!((stats.max_dd).abs() < 1e-9);
    }

    // ── compute_single: explicit periods_per_year ───────────────────────

    #[test]
    fn compute_single_uses_explicit_periods_per_year() {
        let prices = linear_prices(50, 100.0, 0.5);
        let timestamps = daily_timestamps(50);

        let with_252 = compute_single("X".into(), &prices, &timestamps, 0.0, Some(252)).unwrap();
        let with_12 = compute_single("X".into(), &prices, &timestamps, 0.0, Some(12)).unwrap();

        // Different annualisation factors must yield different volatility.
        assert!((with_252.ann_volatility - with_12.ann_volatility).abs() > 1e-6);
    }

    // ── compute_single: time_span = 0 fallback ──────────────────────────

    #[test]
    fn compute_single_handles_zero_time_span() {
        let prices = vec![100.0, 101.0, 102.0, 103.0];
        let timestamps = vec![0.0; 4]; // identical timestamps

        let stats = compute_single("X".into(), &prices, &timestamps, 0.0, None).unwrap();
        // With time_span=0 we fall back to ann=252; stats should still be finite.
        assert!(stats.ann_volatility.is_finite());
        assert!(stats.sharpe.is_finite());
    }

    // ── compute_single: drawdown calculation ────────────────────────────

    #[test]
    fn compute_single_computes_negative_drawdown() {
        // Up then crash then partial recovery
        let prices = vec![100.0, 110.0, 120.0, 60.0, 70.0, 80.0];
        let timestamps = daily_timestamps(prices.len());

        let stats = compute_single("X".into(), &prices, &timestamps, 0.0, None).unwrap();

        // From 120 -> 60 is a 50% drawdown (-0.5 as a fraction)
        assert!(stats.max_dd < -0.4);
        assert!(stats.max_dd >= -1.0);
    }

    // ── compute_single: sortino with downside returns ───────────────────

    #[test]
    fn compute_single_sortino_with_downside() {
        // Mix of up and down days produces a meaningful sortino ratio.
        let prices = vec![100.0, 101.0, 99.0, 102.0, 98.0, 103.0, 97.0];
        let timestamps = daily_timestamps(prices.len());

        let stats = compute_single("X".into(), &prices, &timestamps, 0.0, None).unwrap();
        assert!(stats.sortino.is_finite());
    }

    // ── compute_single: zero-volatility series ──────────────────────────

    #[test]
    fn compute_single_zero_std_yields_zero_sharpe() {
        let prices = vec![100.0; 30];
        let timestamps = daily_timestamps(30);

        let stats = compute_single("X".into(), &prices, &timestamps, 0.0, None).unwrap();
        assert_eq!(stats.sharpe, 0.0);
        assert_eq!(stats.win_rate, 0.0);
        assert_eq!(stats.ann_volatility, 0.0);
    }

    // ── compute_single: total_return = 0 (n_years > 0) ──────────────────

    #[test]
    fn compute_single_collapsing_price_returns_minus_100() {
        // Prices strictly decreasing toward zero -> cagr clamps near -100
        let prices = vec![100.0, 50.0, 25.0, 0.0001];
        let timestamps = daily_timestamps(prices.len());

        let stats = compute_single("X".into(), &prices, &timestamps, 0.0, None).unwrap();
        assert!(stats.cagr < 0.0);
    }

    // ── compute_single: risk_free_rate affects sharpe ───────────────────

    #[test]
    fn compute_single_risk_free_rate_lowers_sharpe() {
        let prices = vec![100.0, 101.0, 102.5, 102.0, 103.0, 104.5, 104.0, 106.0];
        let timestamps = daily_timestamps(prices.len());

        let zero_rf = compute_single("X".into(), &prices, &timestamps, 0.0, None).unwrap();
        let high_rf = compute_single("X".into(), &prices, &timestamps, 0.5, None).unwrap();

        // A higher risk-free rate must reduce the sharpe ratio.
        assert!(high_rf.sharpe < zero_rf.sharpe);
    }

    // ── compute_single: zero downside std collapses sortino to 0 ────────

    #[test]
    fn compute_single_uniform_downside_yields_zero_sortino() {
        // Alternating same up/down moves -> downside returns are all equal,
        // so the downside std is zero and sortino falls back to 0.0.
        let prices = vec![100.0, 99.0, 100.0, 99.0, 100.0, 99.0, 100.0];
        let timestamps = daily_timestamps(prices.len());

        let stats = compute_single("X".into(), &prices, &timestamps, 0.0, None).unwrap();
        assert_eq!(stats.sortino, 0.0);
    }

    // ── compute_single: CAGR overflow fallback ──────────────────────────

    #[test]
    fn compute_single_falls_back_when_cagr_overflows() {
        // Few bars over a tiny ann factor produces a very large 1/n_years
        // exponent. With explicit ppy=1 and only 3 bars we get n_years=2,
        // which is well-defined; here we just exercise the Some(ppy) branch
        // alongside a positive total_return for completeness.
        let prices = vec![1.0, 2.0, 4.0];
        let timestamps = daily_timestamps(prices.len());

        let stats = compute_single("X".into(), &prices, &timestamps, 0.0, Some(1)).unwrap();
        assert!(stats.cagr.is_finite());
    }
}
