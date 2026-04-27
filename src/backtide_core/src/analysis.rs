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

/// Per-symbol statistics record computed by Rust.
struct SymbolStats {
    symbol: String,
    ann_return: f64,
    ann_volatility: f64,
    sharpe: f64,
    sortino: f64,
    max_drawdown: f64,
    win_rate: f64,
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
    if prices.len() < 3 {
        return None;
    }

    // Returns (percentage changes)
    let returns: Vec<f64> = prices.windows(2).map(|w| w[1] / w[0] - 1.0).collect();
    let n = returns.len();
    if n < 2 {
        return None;
    }

    // Annualization factor
    let ann = if let Some(ppy) = periods_per_year {
        ppy as f64
    } else {
        // Estimate from the actual observed bar density: bars / years_of_data.
        // This naturally accounts for weekends, holidays, and market hours,
        // giving ~252 for daily equity data instead of the incorrect ~365.
        let time_span = timestamps[timestamps.len() - 1] - timestamps[0];
        if time_span > 0.0 {
            let years = time_span / (365.25 * 86400.0);
            (n as f64 / years).round().max(1.0)
        } else {
            252.0
        }
    };

    // Annualized return (geometric)
    let total_return = prices[prices.len() - 1] / prices[0];
    let n_years = n as f64 / ann;
    let ann_return = if n_years > 0.0 && total_return > 0.0 {
        let cagr = total_return.powf(1.0 / n_years) - 1.0;
        if cagr.is_finite() {
            cagr * 100.0
        } else {
            // For very short periods CAGR overflows; fall back to simple return
            (total_return - 1.0) * 100.0
        }
    } else if n_years > 0.0 {
        -100.0
    } else {
        0.0
    };

    // Mean and std of returns
    let mean_ret = returns.iter().sum::<f64>() / n as f64;
    let variance = returns.iter().map(|r| (r - mean_ret).powi(2)).sum::<f64>() / (n - 1) as f64;
    let std_ret = variance.sqrt();

    // Annualized volatility
    let ann_volatility = std_ret * ann.sqrt() * 100.0;

    // Sharpe ratio
    let excess = mean_ret - risk_free_rate / ann;
    let sharpe = if std_ret > 0.0 {
        excess / std_ret * ann.sqrt()
    } else {
        0.0
    };

    // Sortino ratio
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

    // Max drawdown
    let mut cumulative = Vec::with_capacity(n);
    let mut cum = 1.0;
    for &r in &returns {
        cum *= 1.0 + r;
        cumulative.push(cum);
    }
    let mut running_max = f64::NEG_INFINITY;
    let mut max_dd = 0.0_f64;
    for &c in &cumulative {
        if c > running_max {
            running_max = c;
        }
        let dd = (c - running_max) / running_max;
        if dd < max_dd {
            max_dd = dd;
        }
    }
    let max_drawdown = max_dd * 100.0;

    // Win rate
    let wins = returns.iter().filter(|&&r| r > 0.0).count();
    let win_rate = wins as f64 / n as f64 * 100.0;

    Some(SymbolStats {
        symbol,
        ann_return,
        ann_volatility,
        sharpe,
        sortino,
        max_drawdown,
        win_rate,
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
    out.set_item("ann_return", PyList::new(py, results.iter().map(|r| r.ann_return))?)?;
    out.set_item("ann_volatility", PyList::new(py, results.iter().map(|r| r.ann_volatility))?)?;
    out.set_item("sharpe_ratio", PyList::new(py, results.iter().map(|r| r.sharpe))?)?;
    out.set_item("sortino_ratio", PyList::new(py, results.iter().map(|r| r.sortino))?)?;
    out.set_item("max_drawdown", PyList::new(py, results.iter().map(|r| r.max_drawdown))?)?;
    out.set_item("win_rate", PyList::new(py, results.iter().map(|r| r.win_rate))?)?;
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
