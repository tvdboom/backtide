"""Backtide.

Author: Mavs
Description: Public Python interface for the backtest module.

Re-exports the core (Rust) API and the Python ``run_experiment`` wrapper
that auto-injects indicators required by the selected strategies and runs a
buy-and-hold benchmark side-car when ``EngineExpConfig.benchmark`` is set.

This module is what ``backtide`` uses everywhere — the Streamlit UI, the
CLI, and Python scripts all go through the same entry point.

"""

from backtide.backtest.utils import run_experiment
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
)

__all__ = [
    "CommissionType",
    "ConversionPeriod",
    "CurrencyConversionMode",
    "DataExpConfig",
    "EmptyBarPolicy",
    "EngineExpConfig",
    "EquitySample",
    "ExchangeExpConfig",
    "ExperimentConfig",
    "ExperimentResult",
    "GeneralExpConfig",
    "IndicatorExpConfig",
    "Order",
    "OrderRecord",
    "OrderType",
    "Portfolio",
    "PortfolioExpConfig",
    "State",
    "StrategyExpConfig",
    "StrategyRunResult",
    "Trade",
    "run_experiment",
]


