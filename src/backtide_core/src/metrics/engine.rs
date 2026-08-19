//! Built-in metric catalog and computation engine.

use crate::analysis::compute_series_stats;
use crate::backtest::models::{EquitySample, Trade};
use crate::metrics::models::MetricDefinition;
use rayon::prelude::*;
use std::collections::HashMap;

/// Stable keys selected by a new experiment unless explicitly overridden.
pub const DEFAULT_METRICS: &[&str] = &[
    "sharpe",
    "total_return",
    "pnl",
    "max_dd",
    "cagr",
    "n_trades",
    "win_rate",
    "sortino",
    "ann_volatility",
    "final_equity",
    "excess_return",
    "alpha",
];

/// Return metadata for every metric implemented by the Rust engine.
pub fn builtin_metric_definitions() -> Vec<MetricDefinition> {
    vec![
        MetricDefinition::new(
            "total_return",
            "Total return",
            "Net portfolio return over the experiment.",
            true,
            true,
        ),
        MetricDefinition::new(
            "pnl",
            "Profit and loss",
            "Final equity minus initial cash.",
            false,
            true,
        ),
        MetricDefinition::new(
            "final_equity",
            "Final equity",
            "Portfolio value at the final sample.",
            false,
            true,
        ),
        MetricDefinition::new("cagr", "CAGR", "Compound annual growth rate.", true, true),
        MetricDefinition::new(
            "ann_volatility",
            "Annualized volatility",
            "Annualized standard deviation of returns.",
            true,
            false,
        ),
        MetricDefinition::new(
            "sharpe",
            "Sharpe ratio",
            "Annualized excess return per unit of volatility.",
            false,
            true,
        ),
        MetricDefinition::new(
            "sortino",
            "Sortino ratio",
            "Annualized excess return per unit of downside deviation.",
            false,
            true,
        ),
        MetricDefinition::new(
            "max_dd",
            "Maximum drawdown",
            "Largest fractional fall from a running equity peak.",
            true,
            true,
        ),
        MetricDefinition::new(
            "calmar",
            "Calmar ratio",
            "CAGR divided by absolute maximum drawdown.",
            false,
            true,
        ),
        MetricDefinition::new(
            "n_trades",
            "Trades",
            "Number of completed round-trip trades.",
            false,
            true,
        ),
        MetricDefinition::new(
            "win_rate",
            "Win rate",
            "Fraction of completed trades with positive PnL.",
            true,
            true,
        ),
        MetricDefinition::new(
            "profit_factor",
            "Profit factor",
            "Gross winning PnL divided by gross losing PnL.",
            false,
            true,
        ),
        MetricDefinition::new(
            "expectancy",
            "Expectancy",
            "Average PnL per completed trade.",
            false,
            true,
        ),
        MetricDefinition::new(
            "avg_win",
            "Average win",
            "Average PnL of profitable trades.",
            false,
            true,
        ),
        MetricDefinition::new(
            "avg_loss",
            "Average loss",
            "Average PnL of losing trades.",
            false,
            true,
        ),
        MetricDefinition::new(
            "best_trade",
            "Best trade",
            "Largest completed-trade PnL.",
            false,
            true,
        ),
        MetricDefinition::new(
            "worst_trade",
            "Worst trade",
            "Smallest completed-trade PnL.",
            false,
            true,
        ),
        MetricDefinition::new(
            "payoff_ratio",
            "Payoff ratio",
            "Average win divided by absolute average loss.",
            false,
            true,
        ),
        MetricDefinition::new(
            "recovery_factor",
            "Recovery factor",
            "Net PnL divided by absolute maximum drawdown amount.",
            false,
            true,
        ),
        MetricDefinition::new(
            "excess_return",
            "Excess return",
            "Return above the compounded risk-free rate.",
            true,
            true,
        ),
        MetricDefinition::new(
            "alpha",
            "Alpha",
            "Return above the selected benchmark over the shared window.",
            true,
            true,
        ),
    ]
}

/// Return whether `key` is implemented by the Rust engine.
pub fn is_builtin_metric(key: &str) -> bool {
    builtin_metric_definitions().iter().any(|item| item.key == key)
}

/// Compute selected independent metrics in parallel.
pub fn compute_builtin_metrics(
    selected: &[String],
    initial_cash: f64,
    risk_free_rate: f64,
    curve: &[EquitySample],
    trades: &[Trade],
) -> HashMap<String, f64> {
    let final_equity = curve.last().map(|sample| sample.equity).unwrap_or(initial_cash);
    let total_return = if initial_cash > 0.0 {
        (final_equity - initial_cash) / initial_cash
    } else {
        0.0
    };
    let pnl = final_equity - initial_cash;
    let n_trades = trades.len() as f64;
    let wins: Vec<f64> =
        trades.iter().filter(|trade| trade.pnl > 0.0).map(|trade| trade.pnl).collect();
    let losses: Vec<f64> =
        trades.iter().filter(|trade| trade.pnl < 0.0).map(|trade| trade.pnl).collect();
    let gross_profit: f64 = wins.iter().sum();
    let gross_loss: f64 = losses.iter().sum::<f64>().abs();
    let avg_win = if wins.is_empty() {
        0.0
    } else {
        gross_profit / wins.len() as f64
    };
    let avg_loss = if losses.is_empty() {
        0.0
    } else {
        -gross_loss / losses.len() as f64
    };
    let values: Vec<f64> = curve.iter().map(|sample| sample.equity).collect();
    let timestamps: Vec<f64> = curve.iter().map(|sample| sample.timestamp as f64).collect();
    let stats = compute_series_stats(&values, &timestamps, risk_free_rate, None);
    let cagr = stats.as_ref().map_or(0.0, |value| value.ann_return);
    let max_dd = stats.as_ref().map_or(0.0, |value| value.max_dd);
    let max_drawdown_amount =
        curve.iter().map(|sample| sample.equity * sample.drawdown.abs()).fold(0.0_f64, f64::max);

    selected
        .par_iter()
        .filter_map(|key| {
            let value = match key.as_str() {
                "total_return" => total_return,
                "pnl" => pnl,
                "final_equity" => final_equity,
                "cagr" => cagr,
                "ann_volatility" => stats.as_ref().map_or(0.0, |value| value.ann_volatility),
                "sharpe" => stats.as_ref().map_or(0.0, |value| value.sharpe),
                "sortino" => stats.as_ref().map_or(0.0, |value| value.sortino),
                "max_dd" => max_dd,
                "calmar" => {
                    if max_dd < 0.0 {
                        cagr / max_dd.abs()
                    } else {
                        0.0
                    }
                },
                "n_trades" => n_trades,
                "win_rate" => {
                    if n_trades > 0.0 {
                        wins.len() as f64 / n_trades
                    } else {
                        0.0
                    }
                },
                "profit_factor" => {
                    if gross_loss > 0.0 {
                        gross_profit / gross_loss
                    } else {
                        0.0
                    }
                },
                "expectancy" => {
                    if n_trades > 0.0 {
                        trades.iter().map(|trade| trade.pnl).sum::<f64>() / n_trades
                    } else {
                        0.0
                    }
                },
                "avg_win" => avg_win,
                "avg_loss" => avg_loss,
                "best_trade" => {
                    trades.iter().map(|trade| trade.pnl).reduce(f64::max).unwrap_or(0.0)
                },
                "worst_trade" => {
                    trades.iter().map(|trade| trade.pnl).reduce(f64::min).unwrap_or(0.0)
                },
                "payoff_ratio" => {
                    if avg_loss < 0.0 {
                        avg_win / avg_loss.abs()
                    } else {
                        0.0
                    }
                },
                "recovery_factor" => {
                    if max_drawdown_amount > 0.0 {
                        pnl / max_drawdown_amount
                    } else {
                        0.0
                    }
                },
                "excess_return" | "alpha" => return None,
                _ => return None,
            };
            Some((
                key.clone(),
                if value.is_finite() {
                    value
                } else {
                    0.0
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::models::Currency;

    #[test]
    fn default_metrics_follow_the_result_summary_priority() {
        assert_eq!(
            DEFAULT_METRICS,
            [
                "sharpe",
                "total_return",
                "pnl",
                "max_dd",
                "cagr",
                "n_trades",
                "win_rate",
                "sortino",
                "ann_volatility",
                "final_equity",
                "excess_return",
                "alpha",
            ],
        );
    }

    #[test]
    fn selected_metrics_are_computed_and_unselected_metrics_are_omitted() {
        let curve = vec![
            EquitySample {
                timestamp: 0,
                equity: 100.0,
                cash: [(Currency::USD, 100.0)].into(),
                drawdown: 0.0,
            },
            EquitySample {
                timestamp: 31_536_000,
                equity: 120.0,
                cash: [(Currency::USD, 120.0)].into(),
                drawdown: 0.0,
            },
        ];
        let selected = vec!["total_return".to_owned(), "cagr".to_owned()];
        let values = compute_builtin_metrics(&selected, 100.0, 0.0, &curve, &[]);

        assert_eq!(values.len(), 2);
        assert!((values["total_return"] - 0.2).abs() < 1e-12);
        assert!(!values.contains_key("sharpe"));
    }
}
