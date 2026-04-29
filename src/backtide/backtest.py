"""Backtide.

Author: Mavs
Description: Public Python interface for the backtest module.
"""

from backtide.core.backtest import (
    CommissionType,
    ConversionPeriod,
    CurrencyConversionMode,
    DataExpConfig,
    EmptyBarPolicy,
    EngineExpConfig,
    EquitySample,
    ExchangeExpConfig,
    ExperimentConfig,
    ExperimentResult,
    GeneralExpConfig,
    IndicatorExpConfig,
    Order,
    OrderRecord,
    OrderType,
    Portfolio,
    PortfolioExpConfig,
    State,
    StrategyExpConfig,
    StrategyRunResult,
    Trade,
    run_experiment,
)

#: Name used to identify the auto-injected Buy & Hold benchmark run inside an
#: ``ExperimentResult.strategies`` list. Must stay in sync with the Rust-side
#: constant in ``backtide_core/src/backtest/engine.rs``.
BENCHMARK_STRATEGY_NAME = "Buy & Hold (Benchmark)"
