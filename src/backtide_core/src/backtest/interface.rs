//! Python interface for the backtest module.

use crate::backtest::models::experiment_config::{
    ExperimentConfig, ExperimentConfigInner, IndicatorExpConfig,
};
use crate::backtest::models::experiment_result::ExperimentResult;
use crate::engine::Engine;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use std::collections::HashMap;

// Field-name → sub-section routing. Used to dispatch flat kwargs into the
// appropriate sub-config of `ExperimentConfig`.
const GENERAL_FIELDS: &[&str] = &["name", "tags", "description"];
const DATA_FIELDS: &[&str] =
    &["instrument_type", "symbols", "full_history", "start_date", "end_date", "interval"];
const PORTFOLIO_FIELDS: &[&str] = &["initial_cash", "base_currency", "starting_positions"];
const STRATEGY_FIELDS: &[&str] = &["benchmark", "strategies"];
const INDICATOR_FIELDS: &[&str] = &["indicators"];
const EXCHANGE_FIELDS: &[&str] = &[
    "commission_type",
    "commission_pct",
    "commission_fixed",
    "slippage",
    "allowed_order_types",
    "partial_fills",
    "allow_margin",
    "max_leverage",
    "initial_margin",
    "maintenance_margin",
    "margin_interest",
    "allow_short_selling",
    "borrow_rate",
    "max_position_size",
    "conversion_mode",
    "conversion_threshold",
    "conversion_period",
    "conversion_interval",
];
const ENGINE_FIELDS: &[&str] = &[
    "warmup_period",
    "trade_on_close",
    "risk_free_rate",
    "exclusive_orders",
    "random_seed",
    "empty_bar_policy",
];
const SECTION_NAMES: &[&str] =
    &["general", "data", "portfolio", "strategy", "indicators", "exchange", "engine"];

/// Run a backtest experiment with the provided configuration.
///
/// Performs the full pipeline end-to-end:
///
/// 1. Resolves and downloads any missing market data (skipped if already present in the
///    database).
/// 2. Computes every indicator once over the entire dataset.
/// 3. Runs every strategy in parallel — each strategy has its own independent portfolio,
///    order book and equity curve.
/// 4. Persists the aggregated [`ExperimentResult`] (and per-strategy artifacts) into the
///    database.
///
/// Parameters
/// ----------
/// config : [ExperimentConfig], optional
///     The complete experiment configuration. If omitted, defaults are
///     used and `kwargs` populate the configuration.
///
/// verbose : bool, default=True
///     Whether to display a progress bar while running.
///
/// **kwargs
///     Any combination of:
///
///     * Sub-config objects via keyword (`general`, `data`, `portfolio`, `strategy`,
///       `indicators`, `exchange`, `engine`).
///     * Flat keyword arguments matching any field of the sub-configs (e.g., `name`,
///      `symbols`, `interval`, `initial_cash`, etc...).
///
///     The `strategies` and `indicators` keyword arguments additionally accept — beyond
///     a list of stored names — any of:
///
///     * A single string (name of a stored strategy / indicator).
///     * A `BaseStrategy` / `BaseIndicator` subclass instance (the class name is used
///       as the display name).
///     * A `dict[str, instance]` mapping explicit names to instances.
///     * A list mixing any of the forms above.
///
/// Returns
/// -------
/// [ExperimentResult]
///     The aggregated result of the run.
///
/// See Also
/// --------
/// - backtide.backtest:ExperimentConfig
/// - backtide.backtest:ExperimentResult
/// - backtide.storage:query_experiments
///
/// Examples
/// --------
/// ```pycon
/// from backtide.backtest import run_experiment
/// from backtide.strategies import BuyAndHold
///
/// result = run_experiment(
///     name="Apple and Microsoft",
///     symbols=["AAPL", "MSFT"],
///     interval="1d",
///     strategies=[BuyAndHold()],
/// )
/// print(result)
/// ```
#[pyfunction]
#[pyo3(signature = (config: "ExperimentConfig | None" = None, *, verbose: "bool" = true, **kwargs: "object") -> "ExperimentResult")]
pub fn run_experiment(
    py: Python<'_>,
    config: Option<&Bound<'_, PyAny>>,
    verbose: bool,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<ExperimentResult> {
    let (cfg, strategy_overrides, indicator_overrides) =
        build_experiment_config(py, config, kwargs)?;
    let engine = Engine::get()?;

    // Release the GIL so rayon workers can acquire it.
    Ok(py.detach(|| {
        engine.run_experiment(&cfg, verbose, &strategy_overrides, &indicator_overrides)
    })?)
}

// ────────────────────────────────────────────────────────────────────────────
// kwargs → ExperimentConfig conversion helpers
// ────────────────────────────────────────────────────────────────────────────

/// Build an [`ExperimentConfig`] from a positional config and `**kwargs`.
///
/// All sub-configs are mutated through Python-level attribute access so
/// the pyclass `FromPyObject` derives (which support enum string aliases
/// like `"1d"` for `Interval::OneDay`) handle value coercion correctly.
fn build_experiment_config(
    py: Python<'_>,
    config: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<(ExperimentConfig, HashMap<String, Py<PyAny>>, HashMap<String, Py<PyAny>>)> {
    // ── Start from positional config (if any) ──────────────────────────
    let base: ExperimentConfig = match config {
        Some(c) if !c.is_none() => {
            if let Ok(r) = c.extract::<PyRef<'_, ExperimentConfig>>() {
                (*r).clone()
            } else if let Ok(d) = c.cast::<PyDict>() {
                let inner: ExperimentConfigInner = pythonize::depythonize(d)?;
                ExperimentConfig::from_inner(py, inner)?
            } else {
                return Err(PyValueError::new_err("config must be an ExperimentConfig or a dict"));
            }
        },
        _ => ExperimentConfig::from_inner(py, ExperimentConfigInner::default())?,
    };

    let mut strategy_overrides: HashMap<String, Py<PyAny>> = HashMap::new();
    let mut indicator_overrides: HashMap<String, Py<PyAny>> = HashMap::new();

    let Some(kwargs) = kwargs else {
        return Ok((base, strategy_overrides, indicator_overrides));
    };
    if kwargs.is_empty() {
        return Ok((base, strategy_overrides, indicator_overrides));
    }

    // Promote `base` to a Python instance so we can use setattr-based
    // mutation (which goes through the pyclass FromPyObject derives).
    let base_py = Py::new(py, base)?;
    let base_bound = base_py.bind(py);

    // Per-section buckets for flat field kwargs. Filled below, applied
    // in a second pass so we only do one getattr/setattr round-trip
    // per touched section.
    let general_kw = PyDict::new(py);
    let data_kw = PyDict::new(py);
    let portfolio_kw = PyDict::new(py);
    let strategy_kw = PyDict::new(py);
    let indicators_kw = PyDict::new(py);
    let exchange_kw = PyDict::new(py);
    let engine_kw = PyDict::new(py);

    for (k, v) in kwargs.iter() {
        let key: String = k.extract()?;

        // ── Special handling for the `strategies` flat kwarg ──
        if key == "strategies" {
            let names = process_polymorphic_list(py, &v, &mut strategy_overrides)?;
            strategy_kw.set_item("strategies", PyList::new(py, &names)?)?;
            continue;
        }
        // ── Special handling for `indicators` (flat list OR sub-config) ──
        if key == "indicators" {
            // Disambiguate sub-config form vs. flat list form.
            if v.extract::<PyRef<'_, IndicatorExpConfig>>().is_ok() {
                base_bound.setattr("indicators", &v)?;
                continue;
            }
            let names = process_polymorphic_list(py, &v, &mut indicator_overrides)?;
            indicators_kw.set_item("indicators", PyList::new(py, &names)?)?;
            continue;
        }

        // ── Sub-config kwargs: replace the whole section ──
        if SECTION_NAMES.contains(&key.as_str()) {
            apply_sub_config_kwarg(py, base_bound, &key, &v, &mut strategy_overrides)?;
            continue;
        }

        // ── Flat kwarg: route to the right sub-config ──
        let target = match key.as_str() {
            k if GENERAL_FIELDS.contains(&k) => &general_kw,
            k if DATA_FIELDS.contains(&k) => &data_kw,
            k if PORTFOLIO_FIELDS.contains(&k) => &portfolio_kw,
            k if STRATEGY_FIELDS.contains(&k) => &strategy_kw,
            k if INDICATOR_FIELDS.contains(&k) => &indicators_kw,
            k if EXCHANGE_FIELDS.contains(&k) => &exchange_kw,
            k if ENGINE_FIELDS.contains(&k) => &engine_kw,
            _ => {
                return Err(PyValueError::new_err(format!(
                    "Unknown keyword argument {key:?} for run_experiment()"
                )));
            },
        };
        target.set_item(&key, v)?;
    }

    // Apply each non-empty section bucket via getattr → setattr loop.
    apply_section_kwargs(base_bound, "general", &general_kw)?;
    apply_section_kwargs(base_bound, "data", &data_kw)?;
    apply_section_kwargs(base_bound, "portfolio", &portfolio_kw)?;
    apply_section_kwargs(base_bound, "strategy", &strategy_kw)?;
    apply_section_kwargs(base_bound, "indicators", &indicators_kw)?;
    apply_section_kwargs(base_bound, "exchange", &exchange_kw)?;
    apply_section_kwargs(base_bound, "engine", &engine_kw)?;

    let cfg: ExperimentConfig = (*base_bound.borrow()).clone();
    Ok((cfg, strategy_overrides, indicator_overrides))
}

/// Apply a bag of `field → value` kwargs to a sub-config of the parent
/// `ExperimentConfig`, then write the modified sub-config back.
///
/// Sub-configs are returned by-value from the `get_all` getter (clones),
/// so we must read once, mutate, and write back.
fn apply_section_kwargs(
    base: &Bound<'_, ExperimentConfig>,
    section: &str,
    kw: &Bound<'_, PyDict>,
) -> PyResult<()> {
    if kw.is_empty() {
        return Ok(());
    }
    let sub = base.getattr(section)?;
    for (k, v) in kw.iter() {
        sub.setattr(k.cast::<PyString>()?, v)?;
    }
    base.setattr(section, sub)?;
    Ok(())
}

/// Apply a sub-config-form kwarg (e.g. `general=GeneralExpConfig(...)`
/// or `general={"name": "x"}`) to the parent.
fn apply_sub_config_kwarg(
    py: Python<'_>,
    base: &Bound<'_, ExperimentConfig>,
    section: &str,
    value: &Bound<'_, PyAny>,
    strategy_overrides: &mut HashMap<String, Py<PyAny>>,
) -> PyResult<()> {
    if let Ok(d) = value.cast::<PyDict>() {
        // Build via SubConfigCls(**dict) so enum aliases (e.g. "1d") are honoured.
        // Pre-process strategies inside a `strategy` sub-config dict.
        let kwargs_dict = PyDict::new(py);
        for (k, v) in d.iter() {
            let key: String = k.extract()?;
            if section == "strategy" && key == "strategies" && !is_list_of_strings(&v) {
                let names = process_polymorphic_list(py, &v, strategy_overrides)?;
                kwargs_dict.set_item(k, PyList::new(py, &names)?)?;
            } else {
                kwargs_dict.set_item(k, v)?;
            }
        }
        // Determine the target class from the base attribute.
        let cls = base.getattr(section)?.get_type();
        let new_sub = cls.call((), Some(&kwargs_dict))?;
        base.setattr(section, new_sub)?;
    } else {
        // Direct sub-config instance — let setattr coerce via FromPyObject.
        base.setattr(section, value)?;
    }
    Ok(())
}

fn is_list_of_strings(value: &Bound<'_, PyAny>) -> bool {
    let Ok(list) = value.cast::<PyList>() else {
        return false;
    };
    for item in list.iter() {
        if item.cast::<PyString>().is_err() {
            return false;
        }
    }
    true
}

/// Process a polymorphic strategies / indicators value.
///
/// Accepts any of:
/// * A single `str` → `[name]`
/// * A `dict[str, instance]` → entries become `(name, instance)` overrides
/// * A single `BaseStrategy` / `BaseIndicator` instance → uses class name
/// * A list of any of the above
fn process_polymorphic_list(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    overrides: &mut HashMap<String, Py<PyAny>>,
) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();

    if let Ok(s) = value.extract::<String>() {
        names.push(s);
        return Ok(names);
    }
    if let Ok(d) = value.cast::<PyDict>() {
        for (k, v) in d.iter() {
            let n: String = k.extract()?;
            overrides.insert(n.clone(), v.unbind());
            names.push(n);
        }
        return Ok(names);
    }
    if let Ok(iter) = value.try_iter() {
        for item in iter {
            let item = item?;
            if let Ok(s) = item.extract::<String>() {
                names.push(s);
            } else if let Ok(d) = item.cast::<PyDict>() {
                for (k, v) in d.iter() {
                    let n: String = k.extract()?;
                    overrides.insert(n.clone(), v.unbind());
                    names.push(n);
                }
            } else {
                let n = class_name(py, &item)?;
                overrides.insert(n.clone(), item.unbind());
                names.push(n);
            }
        }
        return Ok(names);
    }
    let n = class_name(py, value)?;
    overrides.insert(n.clone(), value.clone().unbind());
    names.push(n);
    Ok(names)
}

fn class_name(_py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<String> {
    let cls = obj.get_type();
    let name: String = cls.getattr("__name__")?.extract()?;
    Ok(name)
}
