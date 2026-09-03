use crate::backtest::fx::FxTable;
use crate::backtest::models::{EmptyBarPolicy, ExperimentConfig, ExperimentConfigInner};
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
        metrics: config.metrics.clone(),
        exchange: config.exchange.clone(),
        engine: config.engine.clone(),
    };
    let toml_str = inner.to_toml().map_err(|e| format!("toml serialize: {e}"))?;

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

/// Return all available cash converted to the target currency.
pub fn compute_portfolio_cash(cash: &Cash, target_ccy: &str, fx: &FxTable, ts: i64) -> f64 {
    cash.iter()
        .map(|(ccy, amount)| {
            fx.convert(*amount, &ccy.to_string(), target_ccy, ts).unwrap_or(*amount)
        })
        .sum()
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
    compute_portfolio_cash(cash, target_ccy, fx, ts)
        + compute_invested_equity(positions, aligned, bar_index, quote_ccy, target_ccy, fx, ts)
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
/// Restricts a shared cache to the symbols available to this strategy run,
/// then returns cheap O(1) `df.iloc[:idx+1]` views.
pub fn build_per_symbol_view<'py>(
    py: Python<'py>,
    cached: &HashMap<String, Py<PyAny>>,
    idx: usize,
    symbols: &HashSet<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let out = PyDict::new(py);

    for (sym, df) in cached.iter().filter(|(sym, _)| symbols.contains(sym.as_str())) {
        // The `head` method works for pandas and polars
        out.set_item(sym, df.bind(py).call_method1("head", (idx + 1,))?)?;
    }

    Ok(out.into_any())
}

/// Build a Python dict view of indicator values up to bar `idx`.
///
/// Restricts pre-built full arrays to the symbols available to this strategy
/// run and returns cheap O(1) `arr[:idx+1]` slice-views.
pub fn build_indicator_view<'py>(
    py: Python<'py>,
    cached: &HashMap<String, HashMap<String, Py<PyAny>>>,
    idx: usize,
    symbols: &HashSet<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let out = PyDict::new(py);

    for (name, per_sym) in cached {
        let by_sym = PyDict::new(py);
        for (sym, df) in per_sym.iter().filter(|(sym, _)| symbols.contains(sym.as_str())) {
            by_sym.set_item(sym, df.bind(py).call_method1("head", (idx + 1,))?)?;
        }
        out.set_item(name, by_sym)?;
    }
    Ok(out.into_any())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn bar(timestamp: u64, close: f64) -> Bar {
        Bar {
            open_ts: timestamp,
            close_ts: timestamp + 60,
            open_ts_exchange: timestamp,
            open: close,
            high: close,
            low: close,
            close,
            adj_close: close,
            volume: 10.0,
            n_trades: Some(1),
        }
    }

    #[test]
    fn persists_experiment_configuration_and_reports_filesystem_errors() {
        Python::attach(|py| {
            let config =
                ExperimentConfig::from_inner(py, ExperimentConfigInner::default()).unwrap();
            let temp = tempdir().unwrap();
            let output = persist_experiment_config(&temp.path().join("experiment"), &config)
                .expect("configuration should be persisted");
            let text = std::fs::read_to_string(output).unwrap();
            assert!(text.contains("[general]"));

            let file_parent = temp.path().join("file-parent");
            std::fs::write(&file_parent, "not a directory").unwrap();
            let create_error = persist_experiment_config(&file_parent.join("child"), &config)
                .expect_err("a file cannot be used as a parent directory");
            assert!(create_error.contains("create_dir_all"));

            let blocked_output = temp.path().join("blocked-output");
            std::fs::create_dir(&blocked_output).unwrap();
            std::fs::create_dir(blocked_output.join("config.toml")).unwrap();
            let write_error = persist_experiment_config(&blocked_output, &config)
                .expect_err("a directory cannot be overwritten with TOML");
            assert!(write_error.contains("write"));
        });
    }

    #[test]
    fn validates_quantities_and_iso_dates() {
        assert!(validate_qty(f64::INFINITY, InstrumentType::Stocks).unwrap().contains("finite"));
        assert!(validate_qty(0.0, InstrumentType::Stocks).unwrap().contains("non-zero"));
        assert!(validate_qty(1.5, InstrumentType::Stocks).unwrap().contains("fractional"));
        assert_eq!(validate_qty(1.5, InstrumentType::Crypto), None);

        assert_eq!(iso_to_ts("1970-01-01"), Some(0));
        assert_eq!(iso_to_ts("not-a-date"), None);
    }

    #[test]
    fn aligns_missing_bars_under_each_policy() {
        let bars = HashMap::from([("AAPL".to_owned(), vec![bar(2, 100.0)])]);
        let timeline = [1, 2, 3];

        let skipped = align_bars(&bars, &timeline, EmptyBarPolicy::Skip);
        assert!(skipped["AAPL"][0].is_none());
        assert_eq!(skipped["AAPL"][1].unwrap().close, 100.0);
        assert!(skipped["AAPL"][2].is_none());

        let forward = align_bars(&bars, &timeline, EmptyBarPolicy::ForwardFill);
        assert!(forward["AAPL"][0].is_none());
        let filled = forward["AAPL"][2].unwrap();
        assert_eq!(filled.open_ts, 3);
        assert_eq!(filled.close_ts, 3);
        assert_eq!(filled.volume, 0.0);
        assert_eq!(filled.close, 100.0);

        let nan = align_bars(&bars, &timeline, EmptyBarPolicy::FillWithNaN);
        assert_eq!(nan["AAPL"][0].unwrap().open_ts, 1);
        assert!(nan["AAPL"][0].unwrap().close.is_nan());
        assert_eq!(nan["AAPL"][1].unwrap().close, 100.0);
        assert!(nan["AAPL"][2].unwrap().volume.is_nan());
    }

    #[test]
    fn python_strategy_views_exclude_symbols_outside_the_run_universe() {
        let aligned = HashMap::from([
            ("SAB.MC".to_owned(), vec![Some(Bar::NAN)]),
            ("EXW1.DE".to_owned(), vec![Some(Bar::NAN)]),
        ]);
        let indicators = HashMap::from([(
            "signal".to_owned(),
            HashMap::from([
                ("SAB.MC".to_owned(), vec![vec![1.0]]),
                ("EXW1.DE".to_owned(), vec![vec![2.0]]),
            ]),
        )]);
        let cached_symbols = HashSet::from(["SAB.MC", "EXW1.DE"]);
        let strategy_symbols = HashSet::from(["SAB.MC"]);

        Python::attach(|py| {
            let (data, indicator_data) =
                build_py_cache(py, &aligned, &indicators, &cached_symbols).unwrap();

            let data_view = build_per_symbol_view(py, &data, 0, &strategy_symbols).unwrap();
            let data_view = data_view.cast::<PyDict>().unwrap();
            assert!(data_view.contains("SAB.MC").unwrap());
            assert!(!data_view.contains("EXW1.DE").unwrap());

            let indicator_view =
                build_indicator_view(py, &indicator_data, 0, &strategy_symbols).unwrap();
            let indicator_view = indicator_view.cast::<PyDict>().unwrap();
            let signal =
                indicator_view.get_item("signal").unwrap().unwrap().cast_into::<PyDict>().unwrap();
            assert!(signal.contains("SAB.MC").unwrap());
            assert!(!signal.contains("EXW1.DE").unwrap());
        });
    }
}
