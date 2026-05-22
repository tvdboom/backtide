use crate::backtest::interface::{experiment_log, request_abort, run_experiment};
use crate::backtest::models::*;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub mod engine;
pub mod fx;
pub mod interface;
mod margin;
pub mod models;
mod orders;
pub mod utils;

/// Register the Python interface for `backtide.core.backtest`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.backtest")?;

    // Models
    m.add_class::<CommissionType>()?;
    m.add_class::<ConversionPeriod>()?;
    m.add_class::<CurrencyConversionMode>()?;
    m.add_class::<EmptyBarPolicy>()?;
    m.add_class::<ExperimentStatus>()?;
    m.add_class::<Order>()?;
    m.add_class::<OrderStatus>()?;
    m.add_class::<OrderType>()?;
    m.add_class::<Portfolio>()?;
    m.add_class::<State>()?;

    // Experiment config
    m.add_class::<DataExpConfig>()?;
    m.add_class::<EngineExpConfig>()?;
    m.add_class::<ExchangeExpConfig>()?;
    m.add_class::<ExperimentConfig>()?;
    m.add_class::<GeneralExpConfig>()?;
    m.add_class::<IndicatorExpConfig>()?;
    m.add_class::<PortfolioExpConfig>()?;
    m.add_class::<StrategyExpConfig>()?;

    // Experiment result
    m.add_class::<EquitySample>()?;
    m.add_class::<ExperimentResult>()?;
    m.add_class::<OrderRecord>()?;
    m.add_class::<RunResult>()?;
    m.add_class::<Trade>()?;

    // Functions
    m.add_function(wrap_pyfunction!(run_experiment, &m)?)?;
    m.add_function(wrap_pyfunction!(request_abort, &m)?)?;
    m.add_function(wrap_pyfunction!(experiment_log, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.backtest", &m)?;

    Ok(())
}
