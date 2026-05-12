use crate::backtest::interface::run_experiment;
use crate::backtest::models::commission_type::CommissionType;
use crate::backtest::models::conversion_period::ConversionPeriod;
use crate::backtest::models::currency_conversion_mode::CurrencyConversionMode;
use crate::backtest::models::empty_bar_policy::EmptyBarPolicy;
use crate::backtest::models::experiment_config::*;
use crate::backtest::models::experiment_result::{
    EquitySample, ExperimentResult, OrderRecord, RunResult, Trade,
};
use crate::backtest::models::order::Order;
use crate::backtest::models::order_type::OrderType;
use crate::backtest::models::portfolio::Portfolio;
use crate::backtest::models::state::State;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub mod engine;
pub mod fx;
pub mod indicators;
pub mod interface;
pub mod models;
pub mod strategies;

/// Register the Python interface for `backtide.core.backtest`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.backtest")?;

    // Models
    m.add_class::<CommissionType>()?;
    m.add_class::<ConversionPeriod>()?;
    m.add_class::<CurrencyConversionMode>()?;
    m.add_class::<EmptyBarPolicy>()?;
    m.add_class::<Order>()?;
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

    // Indicators
    m.add_class::<indicators::AverageDirectionalIndex>()?;
    m.add_class::<indicators::AverageTrueRange>()?;
    m.add_class::<indicators::BollingerBands>()?;
    m.add_class::<indicators::CommodityChannelIndex>()?;
    m.add_class::<indicators::ExponentialMovingAverage>()?;
    m.add_class::<indicators::MovingAverageConvergenceDivergence>()?;
    m.add_class::<indicators::OnBalanceVolume>()?;
    m.add_class::<indicators::RelativeStrengthIndex>()?;
    m.add_class::<indicators::SimpleMovingAverage>()?;
    m.add_class::<indicators::StochasticOscillator>()?;
    m.add_class::<indicators::VolumeWeightedAveragePrice>()?;
    m.add_class::<indicators::WeightedMovingAverage>()?;

    // Strategies
    m.add_class::<strategies::AdaptiveRsi>()?;
    m.add_class::<strategies::AlphaRsiPro>()?;
    m.add_class::<strategies::BollingerMeanReversion>()?;
    m.add_class::<strategies::BuyAndHold>()?;
    m.add_class::<strategies::DoubleTop>()?;
    m.add_class::<strategies::HybridAlphaRsi>()?;
    m.add_class::<strategies::Macd>()?;
    m.add_class::<strategies::Momentum>()?;
    m.add_class::<strategies::MultiBollingerRotation>()?;
    m.add_class::<strategies::RiskAverse>()?;
    m.add_class::<strategies::Roc>()?;
    m.add_class::<strategies::RocRotation>()?;
    m.add_class::<strategies::Rsi>()?;
    m.add_class::<strategies::Rsrs>()?;
    m.add_class::<strategies::RsrsRotation>()?;
    m.add_class::<strategies::SmaCrossover>()?;
    m.add_class::<strategies::SmaNaive>()?;
    m.add_class::<strategies::TripleRsiRotation>()?;
    m.add_class::<strategies::TurtleTrading>()?;
    m.add_class::<strategies::Vcp>()?;

    // Functions
    m.add_function(wrap_pyfunction!(run_experiment, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.backtest", &m)?;

    Ok(())
}
