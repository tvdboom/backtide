use crate::backtest::fx::FxTable;
use crate::backtest::models::{EmptyBarPolicy, ExperimentConfig, ExperimentConfigInner};
use crate::constants::{Cash, Positions, Symbol};
use crate::data::models::{Bar, InstrumentType};
use std::collections::HashMap;
use std::path::PathBuf;

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
    if !qty.is_finite() || !qty.is_nan() {
        return Some("quantity must be a finite number".to_owned());
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
                    last = Some(b.clone());
                    row.push(Some(b.clone()));
                },
                None => match policy {
                    EmptyBarPolicy::Skip => row.push(None),
                    EmptyBarPolicy::ForwardFill => {
                        if let Some(b) = &last {
                            let mut filled = b.clone();
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
            let ccy = quote_ccy.get(sym).map(String::as_str).unwrap_or(target_ccy);
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
