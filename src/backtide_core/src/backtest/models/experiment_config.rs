//! Experiment configuration data model.
//!
//! A complete specification for a single backtest experiment, split into
//! logical sections that mirror the tabs on the Streamlit experiment page.

use crate::backtest::models::commission_type::CommissionType;
use crate::backtest::models::conversion_period::ConversionPeriod;
use crate::backtest::models::currency_conversion_mode::CurrencyConversionMode;
use crate::backtest::models::empty_bar_policy::EmptyBarPolicy;
use crate::backtest::models::order_type::OrderType;
use crate::backtest::models::strategy_type::StrategyType;
use crate::data::models::currency::Currency;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use itertools::Itertools;
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::pythonize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;


// ────────────────────────────────────────────────────────────────────────────
// GeneralExpConfig
// ────────────────────────────────────────────────────────────────────────────

/// General metadata for an experiment.
///
/// Attributes
/// ----------
/// name : str, default=""
///     A human-readable name to identify this experiment.
///
/// tags : list[str], default=[]
///     Descriptive tags for organizing and filtering experiments.
///
/// description : str, default=""
///     Free-text description of the experiment.
///
/// See Also
/// --------
/// - backtide.backtest:DataExpConfig
/// - backtide.backtest:EngineExpConfig
/// - backtide.backtest:ExchangeExpConfig
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:IndicatorExpConfig
/// - backtide.backtest:PortfolioExpConfig
/// - backtide.backtest:StrategyExpConfig
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GeneralExpConfig {
    pub name: String,
    pub tags: Vec<String>,
    pub description: String,
}

#[pymethods]
impl GeneralExpConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (name: "str"="", tags: "list[str]"=vec![], description: "str"=""))]
    fn new(name: &str, tags: Vec<String>, description: &str) -> Self {
        Self {
            name: name.to_owned(),
            tags,
            description: description.to_owned(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "GeneralExpConfig(name={:?}, tags=[{:?}], description={:?})",
            self.name,
            self.tags.iter().map(|s| s.to_string()).join(", "),
            self.description,
        )
    }

    /// Convert to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(pythonize(py, self)?.unbind())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// DataExpConfig
// ────────────────────────────────────────────────────────────────────────────

/// Data settings for an experiment.
///
/// Attributes
/// ----------
/// instrument_type : str | [InstrumentType], default="stocks"
///     The category of financial instrument.
///
/// symbols : list[str], default=[]
///     Ticker symbols included in the backtest.
///
/// full_history : bool, default=True
///     If `True`, use the maximum available history for every symbol.
///
/// start_date : str | None, default=None
///     ISO-8601 start date. Ignored when `full_history` is `True`.
///
/// end_date : str | None, default=None
///     ISO-8601 end date.
///
/// interval : str | [Interval], default="1d"
///     Bar interval.
///
/// See Also
/// --------
/// - backtide.backtest:EngineExpConfig
/// - backtide.backtest:ExchangeExpConfig
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:GeneralExpConfig
/// - backtide.backtest:IndicatorExpConfig
/// - backtide.backtest:PortfolioExpConfig
/// - backtide.backtest:StrategyExpConfig
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataExpConfig {
    pub instrument_type: InstrumentType,
    pub symbols: Vec<String>,
    pub full_history: bool,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub interval: Interval,
}

impl Default for DataExpConfig {
    fn default() -> Self {
        Self {
            instrument_type: InstrumentType::default(),
            symbols: Vec::new(),
            full_history: true,
            start_date: None,
            end_date: None,
            interval: Interval::default(),
        }
    }
}

#[pymethods]
impl DataExpConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        instrument_type: "str | InstrumentType" = InstrumentType::Stocks,
        symbols: "list[str]" = vec![],
        full_history: "bool" = true,
        start_date: "str | None" = None,
        end_date: "str | None" = None,
        interval: "str | Interval" = Interval::default(),
    ))]
    fn new(
        instrument_type: InstrumentType,
        symbols: Vec<String>,
        full_history: bool,
        start_date: Option<&str>,
        end_date: Option<&str>,
        interval: Interval,
    ) -> Self {
        Self {
            instrument_type,
            symbols,
            full_history,
            start_date: start_date.map(|s| s.to_owned()),
            end_date: end_date.map(|s| s.to_owned()),
            interval,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "DataExpConfig(instrument_type={}, symbols={:?}, interval={})",
            self.instrument_type, self.symbols, self.interval,
        )
    }

    /// Convert to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(pythonize(py, self)?.unbind())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// PortfolioExpConfig
// ────────────────────────────────────────────────────────────────────────────

/// Portfolio settings for an experiment.
///
/// Attributes
/// ----------
/// initial_cash : int, default=10000
///     Cash balance at the start of the simulation.
///
/// base_currency : str | [Currency], default="USD"
///     ISO 4217 code the portfolio is denominated in.
///
/// starting_positions : dict[str, int], default={}
///     Pre-loaded positions `{symbol: quantity}`.
///
/// See Also
/// --------
/// - backtide.backtest:DataExpConfig
/// - backtide.backtest:EngineExpConfig
/// - backtide.backtest:ExchangeExpConfig
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:GeneralExpConfig
/// - backtide.backtest:IndicatorExpConfig
/// - backtide.backtest:StrategyExpConfig
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortfolioExpConfig {
    pub initial_cash: u64,
    pub base_currency: Currency,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub starting_positions: HashMap<String, i64>,
}

impl Default for PortfolioExpConfig {
    fn default() -> Self {
        Self {
            initial_cash: 10_000,
            base_currency: Currency::default(),
            starting_positions: HashMap::new(),
        }
    }
}

#[pymethods]
impl PortfolioExpConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        initial_cash: "float" = 10_000,
        base_currency: "str | Currency" = Currency::default(),
        starting_positions: "dict[str, int]" = HashMap::new(),
    ))]
    fn new(
        initial_cash: u64,
        base_currency: Currency,
        starting_positions: HashMap<String, i64>,
    ) -> Self {
        Self {
            initial_cash,
            base_currency,
            starting_positions,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioExpConfig(initial_cash={}, base_currency={})",
            self.initial_cash, self.base_currency,
        )
    }

    /// Convert to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(pythonize(py, self)?.unbind())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// StrategyExpConfig
// ────────────────────────────────────────────────────────────────────────────

/// Strategy settings for an experiment.
///
/// Attributes
/// ----------
/// predefined_strategies : list[str | [StrategyType]], default=[]
///     Built-in strategies to run.
///
/// custom_strategies : list[tuple[str, str]], default=[]
///     User-defined strategy code as `(name, code)` tuples.
///
/// See Also
/// --------
/// - backtide.backtest:DataExpConfig
/// - backtide.backtest:EngineExpConfig
/// - backtide.backtest:ExchangeExpConfig
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:GeneralExpConfig
/// - backtide.backtest:IndicatorExpConfig
/// - backtide.backtest:PortfolioExpConfig
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StrategyExpConfig {
    pub predefined_strategies: Vec<StrategyType>,
    pub custom_strategies: Vec<(String, String)>,
}

#[pymethods]
impl StrategyExpConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (predefined_strategies: "list[str | StrategyType]"=vec![], custom_strategies: "list[tuple[str, str]]"=vec![]))]
    fn new(
        predefined_strategies: Vec<StrategyType>,
        custom_strategies: Vec<(String, String)>,
    ) -> Self {
        Self {
            predefined_strategies,
            custom_strategies,
        }
    }

    fn __repr__(&self) -> String {
        format!("StrategyExpConfig(predefined={:?})", self.predefined_strategies,)
    }

    /// Convert to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(pythonize(py, self)?.unbind())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// IndicatorExpConfig
// ────────────────────────────────────────────────────────────────────────────

/// Indicator settings for an experiment.
///
/// Indicators are stored by name. Each name refers to a pickled indicator
/// object saved in the local indicators directory.
///
/// Attributes
/// ----------
/// indicators : list[str], default=[]
///     Names of the indicators to use in this experiment. Each name must
///     match a stored indicator.
///
/// See Also
/// --------
/// - backtide.backtest:DataExpConfig
/// - backtide.backtest:EngineExpConfig
/// - backtide.backtest:ExchangeExpConfig
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:GeneralExpConfig
/// - backtide.backtest:PortfolioExpConfig
/// - backtide.backtest:StrategyExpConfig
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IndicatorExpConfig {
    #[serde(default)]
    pub indicators: Vec<String>,
}

#[pymethods]
impl IndicatorExpConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (indicators: "list[str]"=vec![]))]
    fn new(indicators: Vec<String>) -> Self {
        Self { indicators }
    }

    fn __repr__(&self) -> String {
        format!("IndicatorExpConfig(indicators={:?})", self.indicators)
    }

    /// Convert to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(pythonize(py, self)?.unbind())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// ExchangeExpConfig
// ────────────────────────────────────────────────────────────────────────────

/// Exchange and execution settings for an experiment.
///
/// Attributes
/// ----------
/// commission_type : str | [CommissionType], default="percentage"
///     Fee structure applied to every executed order.
///
/// commission_pct : float, default=0.1
///     Percentage commission per trade.
///
/// commission_fixed : float, default=0.0
///     Fixed commission per trade.
///
/// slippage : float, default=0.05
///     Simulated market-impact percentage.
///
/// allowed_order_types : list[str | [OrderType]], default=["market"]
///     Which order types the engine accepts.
///
/// partial_fills : bool, default=False
///     Whether to simulate partial order fills.
///
/// allow_margin : bool, default=True
///     Whether margin trading is enabled.
///
/// max_leverage : float, default=1.0
///     Maximum leverage ratio.
///
/// initial_margin : float, default=50.0
///     Initial margin percentage.
///
/// maintenance_margin : float, default=25.0
///     Maintenance margin percentage.
///
/// margin_interest : float, default=0.0
///     Annual interest rate on borrowed funds.
///
/// allow_short_selling : bool, default=True
///     Whether short selling is permitted.
///
/// borrow_rate : float, default=0.0
///     Annual borrow cost for short positions.
///
/// max_position_size : int, default=100
///     Max allocation to one position (% of portfolio).
///
/// conversion_mode : str | [CurrencyConversionMode], default="immediate"
///     How foreign-currency proceeds are converted.
///
/// conversion_threshold : float | None, default=None
///     Threshold for `HoldUntilThreshold` mode.
///
/// conversion_period : str | [ConversionPeriod] | None, default=None
///     Period for `EndOfPeriod` mode.
///
/// conversion_interval : int | None, default=None
///     Bar count for `CustomInterval` mode.
///
/// See Also
/// --------
/// - backtide.backtest:DataExpConfig
/// - backtide.backtest:EngineExpConfig
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:GeneralExpConfig
/// - backtide.backtest:IndicatorExpConfig
/// - backtide.backtest:PortfolioExpConfig
/// - backtide.backtest:StrategyExpConfig
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExchangeExpConfig {
    pub commission_type: CommissionType,
    pub commission_pct: f64,
    pub commission_fixed: f64,
    pub slippage: f64,
    pub allowed_order_types: Vec<OrderType>,
    pub partial_fills: bool,
    pub allow_margin: bool,
    pub max_leverage: f64,
    pub initial_margin: f64,
    pub maintenance_margin: f64,
    pub margin_interest: f64,
    pub allow_short_selling: bool,
    pub borrow_rate: f64,
    pub max_position_size: u32,
    pub conversion_mode: CurrencyConversionMode,
    pub conversion_threshold: Option<f64>,
    pub conversion_period: Option<ConversionPeriod>,
    pub conversion_interval: Option<u32>,
}

impl Default for ExchangeExpConfig {
    fn default() -> Self {
        Self {
            commission_type: CommissionType::default(),
            commission_pct: 0.1,
            commission_fixed: 0.0,
            slippage: 0.05,
            allowed_order_types: vec![OrderType::Market],
            partial_fills: false,
            allow_margin: true,
            max_leverage: 1.0,
            initial_margin: 50.0,
            maintenance_margin: 25.0,
            margin_interest: 0.0,
            allow_short_selling: true,
            borrow_rate: 0.0,
            max_position_size: 100,
            conversion_mode: CurrencyConversionMode::default(),
            conversion_threshold: None,
            conversion_period: None,
            conversion_interval: None,
        }
    }
}

#[pymethods]
impl ExchangeExpConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        commission_type: "str | CommissionType" = CommissionType::default(),
        commission_pct: "float" = 0.1,
        commission_fixed: "float" = 0.0,
        slippage: "float" = 0.05,
        allowed_order_types: "list[str | OrderType]" = vec![OrderType::Market],
        partial_fills: "bool" = false,
        allow_margin: "bool" = true,
        max_leverage: "float" = 1.0,
        initial_margin: "float" = 50.0,
        maintenance_margin: "float" = 25.0,
        margin_interest: "float" = 0.0,
        allow_short_selling: "bool" = true,
        borrow_rate: "float" = 0.0,
        max_position_size: "int" = 100,
        conversion_mode: "str | CurrencyConversionMode" = CurrencyConversionMode::default(),
        conversion_threshold: "float | None" = None,
        conversion_period: "str | ConversionPeriod | None" = None,
        conversion_interval: "int | None" = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        commission_type: CommissionType,
        commission_pct: f64,
        commission_fixed: f64,
        slippage: f64,
        allowed_order_types: Vec<OrderType>,
        partial_fills: bool,
        allow_margin: bool,
        max_leverage: f64,
        initial_margin: f64,
        maintenance_margin: f64,
        margin_interest: f64,
        allow_short_selling: bool,
        borrow_rate: f64,
        max_position_size: u32,
        conversion_mode: CurrencyConversionMode,
        conversion_threshold: Option<f64>,
        conversion_period: Option<ConversionPeriod>,
        conversion_interval: Option<u32>,
    ) -> Self {
        Self {
            commission_type,
            commission_pct,
            commission_fixed,
            slippage,
            allowed_order_types,
            partial_fills,
            allow_margin,
            max_leverage,
            initial_margin,
            maintenance_margin,
            margin_interest,
            allow_short_selling,
            borrow_rate,
            max_position_size,
            conversion_mode,
            conversion_threshold,
            conversion_period,
            conversion_interval,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ExchangeExpConfig(commission_type={}, slippage={})",
            self.commission_type, self.slippage,
        )
    }

    /// Convert to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(pythonize(py, self)?.unbind())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// EngineExpConfig
// ────────────────────────────────────────────────────────────────────────────

/// Engine / simulation settings for an experiment.
///
/// Attributes
/// ----------
/// warmup_period : int, default=0
///     Bars to skip before the strategy starts.
///
/// trade_on_close : bool, default=False
///     Fill orders at the close price of the current bar.
///
/// risk_free_rate : float, default=0.0
///     Annualised risk-free rate for metrics.
///
/// benchmark : str, default=""
///     Optional benchmark ticker symbol.
///
/// exclusive_orders : bool, default=False
///     Cancel pending orders when a new order is submitted.
///
/// random_seed : int | None, default=None
///     Fixed RNG seed for reproducibility.
///
/// empty_bar_policy : str | [EmptyBarPolicy], default="forward_fill"
///     How to handle bars with no data.
///
/// See Also
/// --------
/// - backtide.backtest:DataExpConfig
/// - backtide.backtest:ExchangeExpConfig
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:GeneralExpConfig
/// - backtide.backtest:IndicatorExpConfig
/// - backtide.backtest:PortfolioExpConfig
/// - backtide.backtest:StrategyExpConfig
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineExpConfig {
    pub warmup_period: u32,
    pub trade_on_close: bool,
    pub risk_free_rate: f64,
    pub benchmark: String,
    pub exclusive_orders: bool,
    pub random_seed: Option<u64>,
    pub empty_bar_policy: EmptyBarPolicy,
}

impl Default for EngineExpConfig {
    fn default() -> Self {
        Self {
            warmup_period: 0,
            trade_on_close: false,
            risk_free_rate: 0.0,
            benchmark: String::new(),
            exclusive_orders: false,
            random_seed: None,
            empty_bar_policy: EmptyBarPolicy::default(),
        }
    }
}

#[pymethods]
impl EngineExpConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        warmup_period: "int" = 0,
        trade_on_close: "bool" = false,
        risk_free_rate: "float" = 0.0,
        benchmark: "str" = "",
        exclusive_orders: "bool" = false,
        random_seed: "int | None" = None,
        empty_bar_policy: "str | EmptyBarPolicy" = EmptyBarPolicy::default(),
    ))]
    fn new(
        warmup_period: u32,
        trade_on_close: bool,
        risk_free_rate: f64,
        benchmark: &str,
        exclusive_orders: bool,
        random_seed: Option<u64>,
        empty_bar_policy: EmptyBarPolicy,
    ) -> Self {
        Self {
            warmup_period,
            trade_on_close,
            risk_free_rate,
            benchmark: benchmark.to_owned(),
            exclusive_orders,
            random_seed,
            empty_bar_policy,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "EngineExpConfig(warmup_period={}, trade_on_close={})",
            self.warmup_period, self.trade_on_close,
        )
    }

    /// Convert to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(pythonize(py, self)?.unbind())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// ExperimentConfigInner (serde-friendly, no Py<> wrappers)
// ────────────────────────────────────────────────────────────────────────────

/// Internal (pure-Rust) representation used for serialisation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExperimentConfigInner {
    pub general: GeneralExpConfig,
    pub data: DataExpConfig,
    pub portfolio: PortfolioExpConfig,
    pub strategy: StrategyExpConfig,
    pub indicators: IndicatorExpConfig,
    pub exchange: ExchangeExpConfig,
    pub engine: EngineExpConfig,
}

// ────────────────────────────────────────────────────────────────────────────
// ExperimentConfig (Python-facing)
// ────────────────────────────────────────────────────────────────────────────

/// Complete configuration for a single backtest experiment.
///
/// Enum-valued settings accept both their enum variant and
/// plain strings.
///
/// Attributes
/// ----------
/// general : [GeneralExpConfig]
///     Experiment name, tags and description.
///
/// data : [DataExpConfig]
///     Instrument type, symbols, date range and interval.
///
/// portfolio : [PortfolioExpConfig]
///     Initial cash, base currency and starting positions.
///
/// strategy : [StrategyExpConfig]
///     Predefined and custom strategies.
///
/// indicators : [IndicatorExpConfig]
///     Built-in and custom indicators.
///
/// exchange : [ExchangeExpConfig]
///     Commission, slippage, order execution, margin and short-selling.
///
/// engine : [EngineExpConfig]
///     Warmup, timing, benchmark and data-handling policies.
///
/// See Also
/// --------
/// - backtide.backtest:DataExpConfig
/// - backtide.backtest:EngineExpConfig
/// - backtide.backtest:ExchangeExpConfig
/// - backtide.backtest:GeneralExpConfig
/// - backtide.backtest:IndicatorExpConfig
/// - backtide.backtest:PortfolioExpConfig
/// - backtide.backtest:StrategyExpConfig
#[pyclass(get_all, set_all, skip_from_py_object, module = "backtide.backtest")]
#[derive(Clone, Debug)]
pub struct ExperimentConfig {
    pub general: GeneralExpConfig,
    pub data: DataExpConfig,
    pub portfolio: PortfolioExpConfig,
    pub strategy: StrategyExpConfig,
    pub indicators: IndicatorExpConfig,
    pub exchange: ExchangeExpConfig,
    pub engine: EngineExpConfig,
}

impl ExperimentConfig {
    /// Convert from the inner (serde-friendly) representation.
    pub fn from_inner(_py: Python<'_>, inner: ExperimentConfigInner) -> PyResult<Self> {
        Ok(Self {
            general: inner.general,
            data: inner.data,
            portfolio: inner.portfolio,
            strategy: inner.strategy,
            indicators: inner.indicators,
            exchange: inner.exchange,
            engine: inner.engine,
        })
    }

    /// Convert to the inner (serde-friendly) representation.
    pub fn to_inner(&self, _py: Python<'_>) -> ExperimentConfigInner {
        ExperimentConfigInner {
            general: self.general.clone(),
            data: self.data.clone(),
            portfolio: self.portfolio.clone(),
            strategy: self.strategy.clone(),
            indicators: self.indicators.clone(),
            exchange: self.exchange.clone(),
            engine: self.engine.clone(),
        }
    }
}

#[pymethods]
impl ExperimentConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        general: "GeneralExpConfig" = GeneralExpConfig::default(),
        data: "DataExpConfig" = DataExpConfig::default(),
        portfolio: "PortfolioExpConfig" = PortfolioExpConfig::default(),
        strategy: "StrategyExpConfig" = StrategyExpConfig::default(),
        indicators: "IndicatorExpConfig" = IndicatorExpConfig::default(),
        exchange: "ExchangeExpConfig" = ExchangeExpConfig::default(),
        engine: "EngineExpConfig" = EngineExpConfig::default(),
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        general: GeneralExpConfig,
        data: DataExpConfig,
        portfolio: PortfolioExpConfig,
        strategy: StrategyExpConfig,
        indicators: IndicatorExpConfig,
        exchange: ExchangeExpConfig,
        engine: EngineExpConfig,
    ) -> Self {
        Self {
            general,
            data,
            portfolio,
            strategy,
            indicators,
            exchange,
            engine,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ExperimentConfig(general={:?}, data={:?}, ...)",
            self.general.name, self.data.symbols,
        )
    }

    fn __richcmp__(&self, py: Python<'_>, other: PyRef<Self>, op: CompareOp) -> bool {
        match op {
            CompareOp::Eq => self.to_inner(py) == other.to_inner(py),
            CompareOp::Ne => self.to_inner(py) != other.to_inner(py),
            _ => false,
        }
    }

    /// Convert the experiment configuration to a nested dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(pythonize(py, &self.to_inner(py))?.unbind())
    }

    /// Build an `ExperimentConfig` from a (possibly nested) dictionary.
    ///
    /// The dict may use the same nested structure produced by `to_toml`
    /// (with `general`, `data`, `portfolio`, etc. sections) **or**
    /// a flat key-value mapping. Missing keys silently fall back to defaults.
    ///
    /// Parameters
    /// ----------
    /// data : dict
    ///     Source dictionary.
    ///
    /// Returns
    /// -------
    /// self
    ///     The created instance.
    #[staticmethod]
    fn from_dict(py: Python<'_>, data: &Bound<'_, PyDict>) -> PyResult<Self> {
        let inner: ExperimentConfigInner = pythonize::depythonize(data)?;
        Self::from_inner(py, inner)
    }

    /// Serialise the configuration to a TOML string.
    ///
    /// The output is grouped into `[general]`, `[data]`,
    /// `[portfolio]`, `[strategy]`, `[indicators]`,
    /// `[exchange]` and `[engine]` sections.
    ///
    /// Returns
    /// -------
    /// str
    ///     TOML representation of the config.
    pub fn to_toml(&self, py: Python<'_>) -> PyResult<String> {
        toml::to_string_pretty(&self.to_inner(py)).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Build an `ExperimentConfig` from a TOML string.
    ///
    /// Parameters
    /// ----------
    /// text : str
    ///     TOML document.
    ///
    /// Returns
    /// -------
    /// self
    ///     The created instance.
    #[staticmethod]
    fn from_toml(py: Python<'_>, text: &str) -> PyResult<Self> {
        let inner: ExperimentConfigInner =
            toml::from_str(text).map_err(|e| PyValueError::new_err(e.to_string()))?;
        Self::from_inner(py, inner)
    }
}
