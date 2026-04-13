use crate::backtest::models::commission_type::CommissionType;
use crate::backtest::models::conversion_period::ConversionPeriod;
use crate::backtest::models::currency_conversion_mode::CurrencyConversionMode;
use crate::backtest::models::empty_bar_policy::EmptyBarPolicy;
use crate::backtest::models::experiment_config::{
    CodeSnippet, DataExpConfig, EngineExpConfig, ExchangeExpConfig, ExperimentConfig,
    GeneralExpConfig, IndicatorExpConfig, PortfolioExpConfig, StrategyExpConfig,
};
use crate::backtest::models::indicator_type::IndicatorType;
use crate::backtest::models::order_type::OrderType;
use crate::backtest::models::strategy_type::StrategyType;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub mod models;

/// Register the Python interface for `backtide.core.backtest`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.backtest")?;

    m.add_class::<CommissionType>()?;
    m.add_class::<ConversionPeriod>()?;
    m.add_class::<CurrencyConversionMode>()?;
    m.add_class::<EmptyBarPolicy>()?;
    m.add_class::<IndicatorType>()?;
    m.add_class::<OrderType>()?;
    m.add_class::<StrategyType>()?;

    m.add_class::<CodeSnippet>()?;
    m.add_class::<DataExpConfig>()?;
    m.add_class::<EngineExpConfig>()?;
    m.add_class::<ExchangeExpConfig>()?;
    m.add_class::<ExperimentConfig>()?;
    m.add_class::<GeneralExpConfig>()?;
    m.add_class::<IndicatorExpConfig>()?;
    m.add_class::<PortfolioExpConfig>()?;
    m.add_class::<StrategyExpConfig>()?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.backtest", &m)?;

    Ok(())
}
