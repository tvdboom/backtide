//! Python custom-metric execution helpers.

use crate::backtest::models::{EquitySample, Trade};
use crate::utils::python::dict_to_dataframe;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Call a custom metric with result dataframes and extract a finite scalar.
pub fn compute_custom_metric(
    metric: &Py<PyAny>,
    curve: &[EquitySample],
    trades: &[Trade],
) -> PyResult<f64> {
    Python::attach(|py| {
        let equity = PyDict::new(py);
        equity
            .set_item("timestamp", curve.iter().map(|item| item.timestamp).collect::<Vec<_>>())?;
        equity.set_item("equity", curve.iter().map(|item| item.equity).collect::<Vec<_>>())?;
        equity.set_item("drawdown", curve.iter().map(|item| item.drawdown).collect::<Vec<_>>())?;

        let trade_data = PyDict::new(py);
        trade_data.set_item(
            "symbol",
            trades.iter().map(|item| item.symbol.as_str()).collect::<Vec<_>>(),
        )?;
        trade_data
            .set_item("quantity", trades.iter().map(|item| item.quantity).collect::<Vec<_>>())?;
        trade_data
            .set_item("entry_ts", trades.iter().map(|item| item.entry_ts).collect::<Vec<_>>())?;
        trade_data
            .set_item("exit_ts", trades.iter().map(|item| item.exit_ts).collect::<Vec<_>>())?;
        trade_data.set_item(
            "entry_price",
            trades.iter().map(|item| item.entry_price).collect::<Vec<_>>(),
        )?;
        trade_data.set_item(
            "exit_price",
            trades.iter().map(|item| item.exit_price).collect::<Vec<_>>(),
        )?;
        trade_data.set_item("pnl", trades.iter().map(|item| item.pnl).collect::<Vec<_>>())?;

        let value: f64 = metric
            .bind(py)
            .call_method1(
                "compute",
                (dict_to_dataframe(py, &equity)?, dict_to_dataframe(py, &trade_data)?),
            )?
            .extract()?;
        if !value.is_finite() {
            return Err(PyValueError::new_err("metric returned a non-finite value"));
        }
        Ok(value)
    })
}
