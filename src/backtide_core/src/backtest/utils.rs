use crate::analysis::compute_series_stats;
use crate::backtest::fx::FxTable;
use crate::backtest::models::{
    EmptyBarPolicy, EquitySample, ExperimentConfig, ExperimentConfigInner, Trade,
};
use crate::constants::{Cash, DataT, IndicatorsT, Positions, Symbol, MIN_POSITION};
use crate::data::models::{Bar, InstrumentType};
use crate::utils::python::{dict_to_dataframe, to_python};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Returns `true` when `v` is negligibly small (effectively zero).
#[inline]
pub fn is_negligible(v: f64) -> bool {
    v.abs() <= MIN_POSITION
}

/// Returns `true` when `v` represents a meaningful (non-zero) quantity.
#[inline]
pub fn is_significant(v: f64) -> bool {
    v.abs() > MIN_POSITION
}

/// Serialize `config` and write it to `/experiments/<experiment_id>/config.toml`.
pub fn persist_experiment_config(
    path: &PathBuf,
    config: &ExperimentConfig,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(path)
        .map_err(|e| format!("create_dir_all({}): {e}", path.display()))?;

    let inner = ExperimentConfigInner {
        general: config.general.clone(),
        data: config.data.clone(),
        portfolio: config.portfolio.clone(),
        strategy: config.strategy.clone(),
        indicators: config.indicators.clone(),
        exchange: config.exchange.clone(),
        engine: config.engine.clone(),
    };
    let toml_str = toml::to_string_pretty(&inner).map_err(|e| format!("toml serialize: {e}"))?;

    let path = path.join("config.toml");
    std::fs::write(&path, toml_str).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

/// Check whether a position/order quantity is valid.
pub fn validate_qty(qty: f64, it: InstrumentType) -> Option<String> {
    if !qty.is_finite() {
        return Some("quantity must be a finite number".to_owned());
    }

    if qty == 0.0 {
        return Some("quantity must be non-zero".to_owned());
    }

    if !it.allows_fractional_quantities() && qty.fract() != 0. {
        return Some(format!("fractional quantities aren't allowed for instrument type {it}"));
    }

    None
}

/// Parse a date in ISO 8601 format (YYYY-MM-DD) into Unix seconds.
pub fn iso_to_ts(s: &str) -> Option<u64> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp() as u64)
}

/// Align bars to a master timeline using the configured empty-bar policy.
///
/// Uses binary search on the (already-sorted) per-symbol bar vectors.
pub fn align_bars(
    bars: &HashMap<Symbol, Vec<Bar>>,
    timeline: &[i64],
    policy: EmptyBarPolicy,
) -> HashMap<Symbol, Vec<Option<Bar>>> {
    let mut out: HashMap<Symbol, Vec<Option<Bar>>> = HashMap::with_capacity(bars.len());
    for (sym, sym_bars) in bars {
        let mut row: Vec<Option<Bar>> = Vec::with_capacity(timeline.len());
        let mut last: Option<Bar> = None;
        for ts in timeline {
            // Binary search on the sorted bar slice (sorted by open_ts in load_bars).
            let found = sym_bars
                .binary_search_by_key(&(*ts as u64), |b| b.open_ts)
                .ok()
                .map(|i| &sym_bars[i]);

            match found {
                Some(b) => {
                    last = Some(*b);
                    row.push(Some(*b));
                },
                None => match policy {
                    EmptyBarPolicy::Skip => row.push(None),
                    EmptyBarPolicy::ForwardFill => {
                        if let Some(b) = &last {
                            let mut filled = *b;
                            filled.open_ts = *ts as u64;
                            filled.close_ts = *ts as u64;
                            filled.volume = 0.0;
                            row.push(Some(filled));
                        } else {
                            row.push(None);
                        }
                    },
                    EmptyBarPolicy::FillWithNaN => {
                        row.push(Some(Bar {
                            open_ts: *ts as u64,
                            close_ts: *ts as u64,
                            open_ts_exchange: *ts as u64,
                            open: f64::NAN,
                            high: f64::NAN,
                            low: f64::NAN,
                            close: f64::NAN,
                            adj_close: f64::NAN,
                            volume: f64::NAN,
                            n_trades: None,
                        }));
                    },
                },
            }
        }
        out.insert(sym.clone(), row);
    }
    out
}

/// Compute the currently invested across all positions in the target currency.
pub fn compute_invested_equity(
    positions: &Positions,
    aligned: &HashMap<Symbol, Vec<Option<Bar>>>,
    bar_index: usize,
    quote_ccy: &HashMap<&str, &str>,
    target_ccy: &str,
    fx: &FxTable,
    ts: i64,
) -> f64 {
    let mut total = 0.0_f64;

    for (sym, qty) in positions {
        if qty.abs() < MIN_POSITION {
            continue;
        }

        if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
            let value = qty.abs() * b.close;
            let ccy = quote_ccy.get(&sym.as_str()).unwrap_or(&target_ccy);
            total += fx.convert(value, ccy, target_ccy, ts).unwrap_or(value);
        }
    }

    total
}

/// Return the total portfolio equity (cash + positions) in the target currency.
pub fn compute_portfolio_equity(
    cash: &Cash,
    positions: &Positions,
    aligned: &HashMap<Symbol, Vec<Option<Bar>>>,
    bar_index: usize,
    quote_ccy: &HashMap<&str, &str>,
    target_ccy: &str,
    fx: &FxTable,
    ts: i64,
) -> f64 {
    let mut equity = 0.0_f64;

    for (ccy, amount) in cash {
        equity += fx.convert(*amount, &ccy.to_string(), target_ccy, ts).unwrap_or(*amount);
    }

    equity + compute_invested_equity(positions, aligned, bar_index, quote_ccy, target_ccy, fx, ts)
}

/// Compute the final experiment metrics.
pub fn compute_metrics(
    initial_cash: f64,
    risk_free_rate: f64,
    curve: &[EquitySample],
    trades: &[Trade],
) -> HashMap<String, f64> {
    let mut m = HashMap::new();

    let final_equity = curve.last().map(|s| s.equity).unwrap_or(initial_cash);
    let total_return = if initial_cash > 0.0 {
        (final_equity - initial_cash) / initial_cash
    } else {
        0.0
    };

    // Trade-derived metrics.
    let n_trades = trades.len() as f64;
    let n_wins = trades.iter().filter(|t| t.pnl > 0.0).count() as f64;
    let win_rate = if n_trades > 0.0 {
        n_wins / n_trades
    } else {
        0.0
    };

    m.insert("total_return".into(), total_return);
    m.insert("final_equity".into(), final_equity);
    m.insert("pnl".into(), final_equity - initial_cash);
    m.insert("n_trades".into(), n_trades);
    m.insert("win_rate".into(), win_rate);

    // Annualized stats reuse the shared kernel from `analysis.rs`.
    let values: Vec<f64> = curve.iter().map(|s| s.equity).collect();
    let timestamps: Vec<f64> = curve.iter().map(|s| s.timestamp as f64).collect();
    let stats = compute_series_stats(&values, &timestamps, risk_free_rate, None);

    let (cagr, ann_vol, sharpe, sortino, max_dd) = match stats {
        Some(s) => (s.ann_return, s.ann_volatility, s.sharpe, s.sortino, s.max_dd),
        None => (0.0, 0.0, 0.0, 0.0, 0.0),
    };

    m.insert("cagr".into(), cagr);
    m.insert("ann_volatility".into(), ann_vol);
    m.insert("sharpe".into(), sharpe);
    m.insert("sortino".into(), sortino);
    m.insert("max_dd".into(), max_dd);

    m
}

// ────────────────────────────────────────────────────────────────────────────
// Python data-cache helpers
// ────────────────────────────────────────────────────────────────────────────

/// Build a Python data/indicator cache under the GIL.
pub fn build_py_cache(
    py: Python<'_>,
    aligned: &HashMap<Symbol, Vec<Option<Bar>>>,
    indicators: &HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>>,
    symbols: &HashSet<&str>,
) -> PyResult<(DataT, IndicatorsT)> {
    let data_full: DataT = aligned
        .iter()
        .filter(|(sym, _)| symbols.contains(sym.as_str()))
        .map(|(sym, row)| {
            let extract = |f: fn(&Bar) -> f64| -> PyResult<Py<PyAny>> {
                Ok(PyList::new(py, row.iter().map(|b| b.as_ref().map_or(f64::NAN, f)))?.into())
            };

            let dict = PyDict::new(py);
            dict.set_item("open", extract(|b| b.open)?)?;
            dict.set_item("high", extract(|b| b.high)?)?;
            dict.set_item("low", extract(|b| b.low)?)?;
            dict.set_item("close", extract(|b| b.close)?)?;
            dict.set_item("volume", extract(|b| b.volume)?)?;
            Ok((sym.clone(), dict_to_dataframe(py, &dict)?.unbind()))
        })
        .collect::<PyResult<_>>()?;

    let mut ind_full: IndicatorsT = HashMap::with_capacity(indicators.len());
    for (name, per_sym) in indicators {
        let by_sym: HashMap<String, Py<PyAny>> = per_sym
            .iter()
            .map(|(sym, data)| -> PyResult<(String, Py<PyAny>)> {
                Ok((sym.clone(), to_python(py, data)?.unbind()))
            })
            .collect::<PyResult<_>>()?;

        ind_full.insert(name.clone(), by_sym);
    }

    Ok((data_full, ind_full))
}

/// Build a Python dict `{symbol: dataframe}` view through bar `idx`.
///
/// Takes pre-built full dataframes per symbol and returns cheap O(1)
/// `df.iloc[:idx+1]` views.
pub fn build_per_symbol_view<'py>(
    py: Python<'py>,
    cached: &HashMap<String, Py<PyAny>>,
    idx: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let out = PyDict::new(py);

    for (sym, df) in cached {
        // The `head` method works for pandas and polars
        out.set_item(sym, df.bind(py).call_method1("head", (idx + 1,))?)?;
    }

    Ok(out.into_any())
}

/// Build a Python dict view of indicator values up to bar `idx`.
///
/// Takes pre-built full numpy arrays per (indicator, symbol, series) and
/// returns cheap O(1) `arr[:idx+1]` slice-views.
pub fn build_indicator_view<'py>(
    py: Python<'py>,
    cached: &HashMap<String, HashMap<String, Py<PyAny>>>,
    idx: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let out = PyDict::new(py);

    for (name, per_sym) in cached {
        let by_sym = PyDict::new(py);
        for (sym, df) in per_sym {
            by_sym.set_item(sym, df.bind(py).call_method1("head", (idx + 1,))?)?;
        }
        out.set_item(name, by_sym)?;
    }
    Ok(out.into_any())
}
