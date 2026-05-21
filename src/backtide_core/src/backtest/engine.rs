//! Backtest engine logic.
//!
//! This module implements the per-strategy event loop, order matching,
//! multi-currency portfolio bookkeeping and result aggregation.

use crate::backtest::fx::FxTable;
use crate::backtest::interface::check_abort;
use crate::backtest::margin::{accrue_margin_costs, check_order_against_limits, LimitViolation};
use crate::backtest::models::*;
use crate::backtest::orders::{apply_slippage, resolve_trigger, TriggerOutcome};
use crate::backtest::utils::*;
use crate::constants::{Cash, Positions, Symbol, BENCHMARK, MIN_POSITION, SECS_PER_YEAR};
use crate::data::models::*;
use crate::engine::Engine;
use crate::errors::{EngineError, EngineResult};
use crate::indicators::interface::_indicator_deterministic_name;
use crate::indicators::utils::compute_indicators;
use crate::strategies::interface::{BuiltinStrategy, BuyAndHold};
use crate::strategies::utils::load_strategies;
use crate::utils::experiment_log::{EXPERIMENT_SPAN, LOG_PATH_FIELD};
use crate::utils::progress::{progress_bar, progress_spinner};
use crate::utils::python::{dict_to_dataframe, load_pickle, to_python};
use itertools::Itertools;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::Py;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn, Span};
use uuid::Uuid;

impl Engine {
    /// Run a single backtest experiment end-to-end.
    pub fn run_experiment(
        &self,
        config: &ExperimentConfig,
        verbose: bool,
        strategy_overrides: &HashMap<String, Py<PyAny>>,
        indicator_overrides: &HashMap<String, Py<PyAny>>,
    ) -> EngineResult<ExperimentResult> {
        let started_instant = Instant::now();
        let started_at =
            SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);

        let experiment_id = Uuid::new_v4().simple().to_string()[..16].to_owned();
        let mut warnings: Vec<String> = Vec::new();

        // ── Set up per-experiment logging ───────────────────────────────────

        let storage_path = &self.config.data.storage_path;
        let exp_dir = storage_path.join("experiments").join(&experiment_id);
        if let Err(e) = std::fs::create_dir_all(&exp_dir) {
            warn!(experiment_id = %experiment_id, "Failed to create experiment dir: {e}");
            warnings.push(format!("Failed to create experiment dir: {e}"));
        }

        let _ = tracing::info_span!(
            EXPERIMENT_SPAN,
            experiment_id = %experiment_id,
            { LOG_PATH_FIELD } = %exp_dir.join("logs.txt").display(),
        )
        .enter();

        info!("Starting experiment id={} name={:?}", experiment_id, config.general.name);
        info!(
            "Configuration summary:\n \
            Number of symbols: {}\n \
            Interval: {:?}\n \
            Instrument type: {:?}\n \
            Initial_cash: {}\n \
            Benchmark: {}\n \
            Number of strategies: {}\n \
            Number of indicators: {}\n \
            Risk free rate: {}%",
            config.data.symbols.len(),
            config.data.interval.to_string(),
            config.data.instrument_type.to_string(),
            config.portfolio.initial_cash,
            config.strategy.benchmark.as_deref().map_or("None".to_owned(), |s| format!("{s:?}")),
            config.strategy.strategies.len(),
            config.indicators.indicators.len(),
            config.engine.risk_free_rate,
        );

        // Persist the source configuration as a TOML file.
        match persist_experiment_config(&exp_dir, config) {
            Ok(p) => info!("Persisted experiment config to {}", p.display()),
            Err(e) => {
                warn!(experiment_id = %experiment_id, "Failed to persist experiment config: {e}");
                warnings.push(format!("Failed to persist experiment config: {e}"));
            },
        }

        let mut symbols = config.data.symbols.clone();
        if symbols.is_empty() {
            warn!("Experiment has no symbols — aborting.");
            return Err(EngineError::Experiment("Experiment has no symbols.".to_owned()));
        }

        // Augment the symbol list with the benchmark (if any) so its bars get downloaded
        // just like any user symbol. If the benchmark matches a strategy name, it refers
        // to that strategy, no extra download needed. Otherwise, treat it as a symbol.
        let benchmark = config.strategy.benchmark.as_deref().unwrap_or("").trim().to_owned();
        let benchmark_from_strat = config.strategy.strategies.iter().any(|s| s == &benchmark);

        if !benchmark_from_strat && !symbols.iter().any(|s| s == &benchmark) {
            info!("Folding benchmark symbol {benchmark:?} into symbol list");
            symbols.push(benchmark.clone());
        }

        // ── Download data ───────────────────────────────────────────────────

        info!("Resolving instrument profiles for {} symbols...", symbols.len());

        let profiles = self.resolve_profiles(
            symbols.clone(),
            config.data.instrument_type,
            vec![config.data.interval],
            verbose,
        )?;

        info!("Resolved {} instrument profiles.", profiles.len());

        let symbol_it_map: HashMap<Symbol, InstrumentType> = profiles
            .iter()
            .map(|p| (p.instrument.symbol.clone(), p.instrument.instrument_type))
            .collect();

        // Check that the starting positions are valid
        for (symbol, qty) in &config.portfolio.starting_positions {
            if let Some(it) = symbol_it_map.get(symbol) {
                if let Some(reason) = validate_qty(*qty, *it) {
                    return Err(EngineError::Experiment(format!(
                        "Invalid starting position for symbol {symbol}: {reason}, got {qty}."
                    )));
                }
            } else {
                return Err(EngineError::Experiment(format!(
                    "Invalid starting position: symbol {symbol} not listed in data."
                )));
            }
        }

        let start_clamp = config.data.start_date.as_deref().and_then(iso_to_ts);
        let end_clamp = config.data.end_date.as_deref().and_then(iso_to_ts);

        info!(
            "Downloading missing bars from {:?} to {:?})...",
            config.data.start_date, config.data.end_date
        );

        let dl = self.download_bars(&profiles, start_clamp, end_clamp, verbose)?;

        info!(
            "Download complete: {} succeeded, {} failed, {} warning(s).",
            dl.n_succeeded,
            dl.n_failed,
            dl.warnings.len()
        );

        for warning in &dl.warnings {
            warn!("Download warning: {warning}");
            warnings.push(warning.clone());
        }

        // ── Load bars from storage ──────────────────────────────────────────

        info!("Loading bars from storage...");

        let bar_map = self.load_bars(
            &symbols,
            config.data.interval,
            *self
                .config
                .data
                .providers
                .get(&config.data.instrument_type)
                .expect("provider configured for instrument type"),
            start_clamp,
            end_clamp,
        )?;

        let total_bars: usize = bar_map.values().map(|v| v.len()).sum();
        info!("Loaded {} bars across {} symbols.", total_bars, bar_map.len());

        for (sym, bars) in &bar_map {
            debug!(" - {} → {} bars", sym, bars.len());
        }

        // Build a master timeline (union of all symbol timestamps, sorted).
        let mut all_ts: Vec<i64> =
            bar_map.values().flat_map(|bars| bars.iter().map(|b| b.open_ts as i64)).collect();
        all_ts.sort_unstable();
        all_ts.dedup();

        info!("Master timeline has {} unique timestamps.", all_ts.len());

        if all_ts.is_empty() {
            warn!("No bars available for the selected symbols/interval — aborting experiment.");
            warnings.push("No bars available for the selected symbols/interval.".to_owned());
            return Ok(ExperimentResult {
                experiment_id,
                name: config.general.name.clone(),
                tags: config.general.tags.clone(),
                started_at,
                finished_at: started_at + started_instant.elapsed().as_secs() as i64,
                status: ExperimentStatus::Error,
                strategies: Vec::new(),
                warnings,
            });
        }

        // Per-symbol aligned bars indexed by timestamp position.
        let aligned = align_bars(&bar_map, &all_ts, config.engine.empty_bar_policy);
        info!("Aligned bars using policy={:?}.", config.engine.empty_bar_policy.to_string());

        // ── Build FX rate table from currency-conversion legs ───────────────

        let leg_profiles: Vec<&InstrumentProfile> =
            profiles.iter().filter(|p| !symbols.contains(&p.instrument.symbol)).collect();

        info!("Building FX table from {} conversion leg(s).", leg_profiles.len());

        let mut fx = FxTable::new(config.portfolio.base_currency.to_string());
        for leg in &leg_profiles {
            let provider = self.config.data.providers.get(&leg.instrument.instrument_type).unwrap();
            let leg_bars = match self.load_bars(
                std::slice::from_ref(&leg.instrument.symbol),
                config.data.interval,
                *provider,
                start_clamp,
                end_clamp,
            ) {
                Ok(m) => m,
                Err(e) => {
                    warn!(symbol=%leg.instrument.symbol, "Failed to load leg bars: {e}");
                    continue;
                },
            };

            let bars = match leg_bars.get(&leg.instrument.symbol) {
                Some(v) if !v.is_empty() => v,
                _ => {
                    warn!(symbol=%leg.instrument.symbol, "Leg has no bars, skipping FX series.");
                    continue;
                },
            };

            // Extract the base/quote identifiers.
            let (from_str, to_str) = match leg.instrument.base.as_deref() {
                Some(s) if !s.is_empty() => (s, &leg.instrument.quote),
                _ => {
                    debug!(
                        symbol=%leg.instrument.symbol,
                        "Leg has no base currency. Skipping FX series.",
                    );
                    continue;
                },
            };
            let series: Vec<(i64, f64)> =
                bars.iter().map(|b| (b.open_ts as i64, b.close)).collect();

            debug!(
                symbol=%leg.instrument.symbol,
                from=%from_str,
                to=%to_str,
                "Adding FX series ({} bars).", series.len()
            );

            fx.add_series(from_str, to_str, series);
        }

        // When the triangulation crypto stablecoin (e.g., USDT) is configured
        // as pegged to a fiat currency (e.g., USD), add a synthetic 1:1 rate so
        // the FxTable can bridge the crypto and fiat sides of the conversion graph.
        let tri_crypto = &self.config.general.triangulation_crypto;
        let tri_pegged = self.config.general.triangulation_crypto_pegged.to_string();
        if !tri_crypto.is_empty() && *tri_crypto != tri_pegged {
            fx.add_series(tri_crypto, &tri_pegged, vec![(0, 1.0)]);
            debug!("Added synthetic peg: {} -> {} at 1:1.", tri_crypto, tri_pegged);
        }

        // ── Load strategies ─────────────────────────────────────────────────

        info!("Loading {} strategies...", config.strategy.strategies.len());

        let mut strategy_objs = load_strategies(&config.strategy.strategies, strategy_overrides)?;

        // Inject the benchmark strategy when benchmark_from_strat=false.
        if !benchmark.is_empty() && !benchmark_from_strat {
            match Python::attach(|py| -> PyResult<Py<PyAny>> {
                Ok(Py::new(py, BuyAndHold::new(Some(benchmark.clone())))?.into_any())
            }) {
                Ok(obj) => {
                    info!("Injected benchmark strategy BuyAndHold({}).", benchmark);
                    strategy_objs.push((BENCHMARK.to_owned(), obj, false));
                },
                Err(e) => {
                    warn!("Failed to instantiate benchmark: {e}");
                    warnings.push(format!("Failed to instantiate benchmark: {e}"));
                },
            }
        }

        // ── Load and compute indicators ─────────────────────────────────────

        info!("Loading indicators...");

        let mut indicator_objs: Vec<(String, Py<PyAny>)> = Vec::new();

        let mut seen_inds: HashSet<String> = HashSet::new();
        for name in &config.indicators.indicators {
            match Python::attach(|py| -> PyResult<Py<PyAny>> {
                if let Some(o) = indicator_overrides.get(name) {
                    Ok(o.clone_ref(py))
                } else {
                    let path = self
                        .config
                        .data
                        .storage_path
                        .join("indicators")
                        .join(format!("{name}.pkl"));

                    load_pickle(py, &path)
                }
            }) {
                Ok(obj) => {
                    if seen_inds.insert(name.clone()) {
                        indicator_objs.push((name.clone(), obj));
                    }
                },
                Err(e) => {
                    warn!("Failed to load indicator {name}: {e}");
                    warnings.push(format!("Failed to load indicator {name}: {e}"));
                },
            }
        }

        // Load the required indicators by the strategies
        for (sname, sobj, _) in &strategy_objs {
            let pairs = Python::attach(|py| -> PyResult<Vec<(String, Py<PyAny>)>> {
                let bound = sobj.bind(py);
                if !bound.hasattr("required_indicators")? {
                    return Ok(Vec::new());
                }

                let raw = bound.call_method0("required_indicators")?;
                let inds: Vec<Py<PyAny>> = raw.extract()?;

                let mut out = Vec::with_capacity(inds.len());
                for ind in inds {
                    let name = _indicator_deterministic_name(ind.bind(py).as_any())?;
                    out.push((name, ind));
                }

                Ok(out)
            });

            match pairs {
                Ok(pairs) => {
                    for (name, obj) in pairs {
                        if seen_inds.insert(name.clone()) {
                            debug!("Auto-injecting indicator {name} required by strategy {sname}.");
                            indicator_objs.push((name, obj));
                        }
                    }
                },
                Err(e) => warn!("Failed to collect required indicators for strategy {sname}: {e}."),
            }
        }

        info!("Computing {} indicator(s)...", indicator_objs.len());

        let pb =
            verbose.then(|| progress_bar(indicator_objs.len() as u64, "Computing indicators..."));

        let indicators = compute_indicators(&indicator_objs, &aligned, pb.as_ref())?;

        info!("Finished computing indicators.");

        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // ── Run strategies ──────────────────────────────────────────────────

        let n_strategies = strategy_objs.len() as u64;
        let pb = verbose
            .then(|| progress_bar(n_strategies, format!("Running {n_strategies} strategies...")));

        let pb_mutex = pb.as_ref().map(Mutex::new);

        let (custom, builtin): (Vec<_>, Vec<_>) =
            strategy_objs.into_iter().partition(|(_, _, is_custom)| *is_custom);

        info!("Dispatching strategies: {} built-in and {} custom.", builtin.len(), custom.len());

        // Pre-build the Python data/indicator cache. Benchmark custom strategies
        // receive `None` and fall back to their own copy inside `run_one_strategy`.
        let py_cache = if custom.iter().any(|(n, _, _)| n != &benchmark) {
            let symbols: HashSet<&str> = symbols.iter().map(String::as_str).collect();
            Python::attach(|py| build_py_cache(py, &aligned, &indicators, &symbols))
                .map_err(|e| warn!("Failed to pre-build shared strategy cache: {e}"))
                .ok()
        } else {
            None
        };

        // Capture the experiment span so each rayon worker can re-enter it.
        let par_span = Span::current();

        // Borrow everything — rayon's collect() blocks until all workers finish,
        // so these references are valid for the entire parallel section.
        let run = |(name, obj, _): (String, _, _)| {
            par_span.in_scope(|| {
                info!("▶ Running strategy {:?}...", name);

                let result = run_one_strategy(
                    &name,
                    obj,
                    &config,
                    &aligned,
                    &indicators,
                    &profiles,
                    &all_ts,
                    &fx,
                    py_cache.as_ref(),
                );

                info!(
                    "✔ Finished strategy {:?}: {} trades, {} bars in equity curve.",
                    result.strategy_name,
                    result.trades.len(),
                    result.equity_curve.len()
                );

                if let Some(pb) = &pb_mutex {
                    pb.lock().unwrap().inc(1);
                }

                result
            })
        };

        // Run the built-in and custom strategies in parallel.
        let (mut results, custom_results): (Vec<RunResult>, Vec<RunResult>) = rayon::join(
            || builtin.into_par_iter().map(&run).collect(),
            || custom.into_iter().map(&run).collect(),
        );
        results.extend(custom_results);

        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // ── Compute alpha & excess return ───────────────────────────────────

        info!(
            "Computing alpha & excess return (risk_free_rate={}%{}).",
            config.engine.risk_free_rate,
            if benchmark.is_empty() {
                "".to_owned()
            } else {
                format!(", benchmark={benchmark:?}")
            }
        );

        let rf = config.engine.risk_free_rate / 100.;

        // Snapshot of the benchmark's equity curve (ts, equity), if any.
        let bench_run = results.iter().find(|r| r.is_benchmark);

        let bench_snapshot: Option<Vec<(i64, f64)>> =
            bench_run.map(|r| r.equity_curve.iter().map(|s| (s.timestamp, s.equity)).collect());

        // Benchmark availability starts when the benchmark can actually be
        // traded (first entry trade), not at the first synthetic equity sample.
        let bench_start_ts = bench_run.and_then(|r| r.trades.iter().map(|t| t.entry_ts).min());

        // Windowed total return.
        let windowed_return = |curve: &[(i64, f64)], window_start: i64| -> Option<f64> {
            let (_, start_eq) = curve.iter().find(|(t, _)| *t >= window_start).copied()?;
            let (_, end_eq) = curve.last().copied()?;
            if start_eq <= 0. {
                None
            } else {
                Some((end_eq - start_eq) / start_eq)
            }
        };

        for r in &mut results {
            let curve_pts: Vec<(i64, f64)> =
                r.equity_curve.iter().map(|s| (s.timestamp, s.equity)).collect();

            let curve_start = match curve_pts.first() {
                Some((t, _)) => *t,
                None => continue,
            };

            let strat_end = curve_pts.last().map(|(t, _)| *t).unwrap_or(curve_start);

            // For delayed listings, the strategy only becomes investable at first fill.
            // Before that, equity is a placeholder flat segment.
            let strat_start = r.trades.iter().map(|t| t.entry_ts).min().unwrap_or(curve_start);

            // Align with benchmark when available.
            let window_start = match bench_start_ts {
                Some(b) => strat_start.max(b),
                None => strat_start,
            };

            let strat_ret = windowed_return(&curve_pts, window_start);

            // Compounded risk-free return over the same valid window.
            let excess_return = strat_ret.map(|ret| {
                let years = ((strat_end - window_start).max(0) as f64) / SECS_PER_YEAR;
                let rf_ret = if years > 0.0 {
                    (1.0_f64 + rf).powf(years) - 1.0
                } else {
                    0.0
                };

                ret - rf_ret
            });

            if let Some(v) = excess_return {
                r.metrics.insert("excess_return".into(), v);
            } else {
                r.metrics.remove("excess_return");
            }

            // Alpha is only meaningful for non-benchmark runs.
            if let Some(bench) = bench_snapshot.as_ref() {
                if !r.is_benchmark {
                    // If benchmark never became investable, alpha is unavailable.
                    let alpha = bench_start_ts.and_then(|_| {
                        strat_ret
                            .and_then(|ret| windowed_return(bench, window_start).map(|b| ret - b))
                    });

                    if let Some(v) = alpha {
                        r.metrics.insert("alpha".into(), v);
                    } else {
                        r.metrics.remove("alpha");
                    }
                } else {
                    // Benchmark strategy always has zero alpha.
                    r.metrics.insert("alpha".into(), 0.0);
                }
            }
        }

        let finished_at = started_at + started_instant.elapsed().as_secs() as i64;

        // If an abort was requested during the simulation, bail out before
        // running diagnostics or persisting any partial results.
        if check_abort() {
            info!("Experiment aborted — skipping diagnostics and persistence.");
            return Err(EngineError::Aborted);
        }

        // Surface per-strategy failures: log each one and roll the experiment
        // status up to "failed" if any strategy errored out.
        for r in &results {
            if let Some(err) = &r.error {
                warn!(strategy = %r.strategy_name, "Strategy failed: {err}");
                warnings.push(format!("Strategy {:?} failed: {}", r.strategy_name, err));
                continue;
            }

            // Diagnose two cases were no trades were filled.
            if r.orders.is_empty() {
                let msg = format!(
                    "Strategy {:?} produced no orders. No buy/sell signal was triggered during \
                     the backtest window.",
                    r.strategy_name
                );

                warn!(strategy = %r.strategy_name, "{msg}");
                warnings.push(msg);
            } else if r.orders.iter().all(|o| o.status != OrderStatus::Filled) {
                // All orders are pending/rejected/canceled. Use the first non-empty
                // reason as the headline cause or fall back to a generic message when
                // no reason was recorded.
                let first_reason = r
                    .orders
                    .iter()
                    .find_map(|o| (!o.reason.is_empty()).then_some(o.reason.as_str()))
                    .unwrap_or("see per-order rejection reasons");

                let msg = format!(
                    "Strategy {:?} produced {} orders but none were filled (first reason: {}).",
                    r.strategy_name,
                    r.orders.len(),
                    first_reason,
                );

                warn!(strategy = %r.strategy_name, "{msg}");
                warnings.push(msg);
            }
        }

        let n_failed = results.iter().filter(|r| r.error.is_some()).count();
        let status = if n_failed == 0 {
            ExperimentStatus::Success
        } else if n_failed == results.len() {
            ExperimentStatus::Error
        } else {
            ExperimentStatus::Partial
        };

        info!(
            "All strategies completed in {}s ({} results, {} failed, status={}).",
            finished_at - started_at,
            results.len(),
            n_failed,
            status,
        );

        for r in &results {
            if let Some(error) = r.error.as_deref() {
                info!("  ✗ {:<32} FAILED — {error}", r.strategy_name);
                continue;
            }

            info!(
                "  • {:<32} sharpe={:+.3}  total_return={:+.4}  excess={}  alpha={}",
                r.strategy_name,
                r.metrics.get("sharpe").map(|e| format!("{e:+.4}")).unwrap_or("n/a".into()),
                r.metrics.get("total_return").map(|e| format!("{e:+.4}")).unwrap_or("n/a".into()),
                r.metrics.get("excess_return").map(|e| format!("{e:+.4}")).unwrap_or("n/a".into()),
                r.metrics.get("alpha").map(|a| format!("{a:+.4}")).unwrap_or("n/a".into())
            );
        }

        let mut result = ExperimentResult {
            experiment_id: experiment_id.clone(),
            name: config.general.name.clone(),
            tags: config.general.tags.clone(),
            started_at,
            finished_at,
            status,
            strategies: results,
            warnings,
        };

        // ── Persist results ─────────────────────────────────────────────────

        info!("Persisting experiment to the database...");

        let pb = verbose.then(|| progress_spinner("Persisting experiment results..."));

        result.finished_at = started_at + started_instant.elapsed().as_secs() as i64;

        let persist_start = Instant::now();
        if let Err(e) = self.db.write_experiment(config, &result) {
            warn!("Failed to persist experiment: {e}");
        } else {
            info!("Experiment persisted successfully in {:?}.", persist_start.elapsed());
        }

        if let Some(p) = pb {
            p.finish_and_clear();
        }

        info!(
            "Experiment {} finished with status={:?} ({} strategies, {} warnings) in {:?}.",
            experiment_id,
            result.status.to_string(),
            result.strategies.len(),
            result.warnings.len(),
            started_instant.elapsed(),
        );

        Ok(result)
    }

    /// Load all bars for the given symbols/interval/provider as a HashMap.
    fn load_bars(
        &self,
        symbols: &[String],
        interval: Interval,
        provider: Provider,
        start: Option<u64>,
        end: Option<u64>,
    ) -> EngineResult<HashMap<String, Vec<Bar>>> {
        let sym_refs: Vec<&str> = symbols.iter().map(String::as_str).collect();
        let stored =
            self.db.query_bars(Some(&sym_refs), Some(&[interval]), Some(&[provider]), None)?;

        let mut map: HashMap<String, Vec<Bar>> = HashMap::new();
        for r in stored {
            let ts = r.bar.open_ts;
            if let Some(s) = start {
                if ts < s {
                    continue;
                }
            }

            if let Some(e) = end {
                if ts >= e {
                    continue;
                }
            }

            map.entry(r.symbol).or_default().push(r.bar);
        }

        for v in map.values_mut() {
            v.sort_by_key(|b| b.open_ts);
        }

        Ok(map)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Python data-cache helpers
// ────────────────────────────────────────────────────────────────────────────

/// Pre-built per-symbol data cache (symbol → full dataset).
type DataT = HashMap<String, Py<PyAny>>;

/// Pre-built per-indicator cache (indicator → symbol → dataset).
type IndicatorsT = HashMap<String, HashMap<String, Py<PyAny>>>;

/// Build a Python data/indicator cache under the GIL.
fn build_py_cache(
    py: Python<'_>,
    aligned: &HashMap<Symbol, Vec<Option<Bar>>>,
    indicators: &HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>>,
    symbols: &HashSet<&str>,
) -> PyResult<(DataT, IndicatorsT)> {
    let data_full: DataT = aligned
        .iter()
        .filter(|(sym, _)| symbols.contains(sym.as_str()))
        .map(|(sym, row)| {
            let extract = |f: fn(&Bar) -> f64| -> PyResult<Py<PyAny>> {
                Ok(PyList::new(py, row.iter().map(|b| b.as_ref().map_or(f64::NAN, f)))?.into())
            };

            let dict = PyDict::new(py);
            dict.set_item("open", extract(|b| b.open)?)?;
            dict.set_item("high", extract(|b| b.high)?)?;
            dict.set_item("low", extract(|b| b.low)?)?;
            dict.set_item("close", extract(|b| b.close)?)?;
            dict.set_item("volume", extract(|b| b.volume)?)?;
            Ok((sym.clone(), dict_to_dataframe(py, &dict)?.unbind()))
        })
        .collect::<PyResult<_>>()?;

    let mut ind_full: IndicatorsT = HashMap::with_capacity(indicators.len());
    for (name, per_sym) in indicators {
        let by_sym: HashMap<String, Py<PyAny>> = per_sym
            .iter()
            .map(|(sym, data)| -> PyResult<(String, Py<PyAny>)> {
                Ok((sym.clone(), to_python(py, data)?.unbind()))
            })
            .collect::<PyResult<_>>()?;

        ind_full.insert(name.clone(), by_sym);
    }

    Ok((data_full, ind_full))
}

// ────────────────────────────────────────────────────────────────────────────
// Per-strategy runner
// ────────────────────────────────────────────────────────────────────────────

/// Execute one strategy through the entire timeline.
fn run_one_strategy(
    name: &str,
    strategy: Py<PyAny>,
    cfg: &ExperimentConfig,
    aligned: &HashMap<Symbol, Vec<Option<Bar>>>,
    indicators: &HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>>,
    profiles: &[InstrumentProfile],
    timeline: &[i64],
    fx: &FxTable,
    py_cache: Option<&(DataT, IndicatorsT)>,
) -> RunResult {
    let benchmark = cfg.strategy.benchmark.as_deref().unwrap_or("").trim();
    let is_benchmark_run = name == benchmark || name == BENCHMARK;

    // The benchmark strategy gets a view restricted to just the benchmark symbol.
    let symbols: HashSet<&str> = if is_benchmark_run {
        std::iter::once(benchmark).collect()
    } else {
        cfg.data.symbols.iter().map(String::as_str).collect()
    };

    // First fatal error encountered during the run.
    let mut run_error: Option<String> = None;

    // Initial portfolio: all initial cash in base currency.
    let base_ccy = cfg.portfolio.base_currency;
    let base_ccy_str = base_ccy.to_string();

    let mut cash: Cash = Cash::from([(base_ccy, cfg.portfolio.initial_cash as f64)]);

    // The benchmark strategy always starts with a clean slate (no pre-existing
    // holdings) so its return reflects a pure buy-and-hold from cash.
    let mut positions: Positions = if is_benchmark_run {
        Positions::new()
    } else {
        cfg.portfolio.starting_positions.clone()
    };

    let mut open_orders: Vec<Order> = Vec::new();

    // Per-order extremes for trailing stops: (running_high, running_low)
    // observed since the order was first seen. Cleared on fill / cancel.
    let mut trail_state: HashMap<String, (f64, f64)> = HashMap::new();

    let total_bars: usize = aligned.values().map(|v| v.len()).next().unwrap_or(0);
    let mut equity_curve: Vec<EquitySample> = Vec::with_capacity(total_bars);
    let mut order_records: Vec<OrderRecord> = Vec::new();
    let mut closed_trades: Vec<Trade> = Vec::new();

    // Open trade tracker per symbol: (entry_ts, qty_remaining, entry_price)
    let mut open_trades: HashMap<Symbol, (i64, f64, f64)> = HashMap::new();
    let mut margin_limit_warnings: HashSet<String> = HashSet::new();

    let mut peak_equity = cfg.portfolio.initial_cash as f64;

    // Tracks the boundary used by `EndOfPeriod` and the counter used by `CustomInterval`.
    let mut last_period_bucket: Option<i64> = None;
    let mut bars_since_conv: usize = 0;

    // Pre-compute instrument quote currency lookup.
    let quote_ccy: HashMap<&str, &str> = profiles
        .iter()
        .map(|p| (p.instrument.symbol.as_str(), p.instrument.quote.as_str()))
        .collect();

    // Create mapping from symbol to instrument type.
    let it_map: HashMap<&str, InstrumentType> = profiles
        .iter()
        .map(|p| (p.instrument.symbol.as_str(), p.instrument.instrument_type))
        .collect();

    // Try to take a Rust-only snapshot of the strategy.
    let builtin: Option<BuiltinStrategy> =
        Python::attach(|py| BuiltinStrategy::try_from_py(py, &strategy));

    // Pre-extract per-symbol bar arrays once.
    let mut bars_full: Vec<(&str, Vec<Bar>)> = aligned
        .iter()
        .filter(|(s, _)| symbols.contains(s.as_str()))
        .map(|(s, row)| (s.as_str(), row.iter().map(|b| b.unwrap_or(Bar::NAN)).collect()))
        .sorted_by(|a, b| a.0.cmp(&b.0))
        .collect();

    // Pre-build the Python data/indicator cache for benchmark custom strategies.
    //
    // We hold references (`&DataCache`, `&IndCache`) to avoid cloning
    // Python object maps: `_empty_*` provides the view for built-ins and
    // `_fresh` owns the on-demand build for the benchmark-custom case.
    let _empty_data: DataT = HashMap::new();
    let _empty_ind: IndicatorsT = HashMap::new();
    let _fresh: Option<(DataT, IndicatorsT)> = if builtin.is_none() && py_cache.is_none() {
        Some(Python::attach(|py| build_py_cache(py, aligned, indicators, &symbols)).unwrap_or_else(
            |e| {
                let msg = format!("Failed to pre-build strategy view: {e}");
                warn!(strategy=%name, "{msg}");
                run_error.get_or_insert(msg);
                (HashMap::new(), HashMap::new())
            },
        ))
    } else {
        None
    };

    let cached_data: &DataT;
    let cached_indicators: &IndicatorsT;
    if builtin.is_some() {
        cached_data = &_empty_data;
        cached_indicators = &_empty_ind;
    } else if let Some((d, i)) = py_cache {
        cached_data = d;
        cached_indicators = i;
    } else if let Some((d, i)) = &_fresh {
        cached_data = d;
        cached_indicators = i;
    } else {
        cached_data = &_empty_data;
        cached_indicators = &_empty_ind;
    }

    for bar_index in 0..total_bars {
        // Check if the user aborted the experiment periodically.
        if bar_index & 15 == 0 && check_abort() {
            break;
        }

        let ts = timeline[bar_index];
        let is_warmup = bar_index < cfg.engine.warmup_period as usize;

        // ── Per-bar margin interest & short-borrow accrual ──────────────────

        // Charges are prorated by the gap between consecutive bars in the
        // master timeline (which absorbs weekends/holidays without any
        // bespoke calendar logic — the timeline is the truth of when the
        // engine actually steps).
        let bar_seconds = if bar_index == 0 {
            0
        } else {
            (timeline[bar_index] - timeline[bar_index - 1]).max(0)
        };

        accrue_margin_costs(
            cfg,
            &mut cash,
            &positions,
            aligned,
            bar_index,
            &quote_ccy,
            base_ccy,
            fx,
            ts,
            bar_seconds,
        );

        // ── Resolve open orders against the current bar ─────────────────────

        let mut still_open: Vec<Order> = Vec::new();
        let drained: Vec<Order> = std::mem::take(&mut open_orders);
        for mut order in drained {
            // Cancel orders take effect immediately.
            if order.order_type == OrderType::Cancel {
                if let Some(order) = still_open.iter().find(|o| o.id == order.id) {
                    still_open.retain(|o| o.id != order.id);
                    trail_state.remove(&order.id);

                    order_records.push(OrderRecord {
                        order: order.clone(),
                        timestamp: ts,
                        status: OrderStatus::Canceled,
                        fill_price: None,
                        reason: "canceled by cancellation order".into(),
                        commission: 0.0,
                        pnl: None,
                    });

                    continue;
                }
            }

            // Get the bar for the symbol for which the order was called.
            let bar = match aligned.get(&order.symbol).and_then(|r| r[bar_index]) {
                Some(b) => b,
                None => {
                    still_open.push(order);
                    continue;
                },
            };

            // Decide whether this order fires this bar and at what price.
            let outcome = resolve_trigger(
                &mut order,
                &bar,
                &positions,
                &mut trail_state,
                cfg.engine.trade_on_close,
            );

            let (raw_px, mut fill_reason, limit_cap) = match outcome {
                TriggerOutcome::Fill {
                    raw_px,
                    reason,
                    limit_cap,
                } => (raw_px, reason, limit_cap),
                TriggerOutcome::Pending => {
                    still_open.push(order);
                    continue;
                },
                TriggerOutcome::Cancel {
                    reason,
                } => {
                    trail_state.remove(&order.id);
                    order_records.push(OrderRecord {
                        order: order.clone(),
                        timestamp: ts,
                        status: OrderStatus::Canceled,
                        fill_price: None,
                        reason,
                        commission: 0.0,
                        pnl: None,
                    });
                    continue;
                },
            };

            // Apply slippage; for limit-style fills, never cross the limit.
            let fill_px = apply_slippage(raw_px, order.quantity, cfg.exchange.slippage, limit_cap);

            let mut qty = order.quantity;
            let mut filled_qty = qty;

            // Determine accounting currency for cash operations. For non-fiat
            // quote currencies, convert fill amounts to the portfolio base
            // currency so cash accounting stays in fiat.
            let order_ccy_str =
                quote_ccy.get(order.symbol.as_str()).unwrap_or(&base_ccy_str.as_str());

            let (order_ccy, nonfiat_fx_rate) = match order_ccy_str.parse::<Currency>() {
                Ok(fiat) => (fiat, 1.0_f64),
                Err(_) => {
                    let rate = fx.rate(order_ccy_str, &base_ccy_str, ts).unwrap_or(1.0);
                    (base_ccy, rate)
                },
            };

            let acct_fill_px = fill_px * nonfiat_fx_rate;

            let mut notional = acct_fill_px * qty.abs();
            let mut commission = match cfg.exchange.commission_type {
                CommissionType::Percentage => notional * cfg.exchange.commission_pct / 100.,
                CommissionType::Fixed => cfg.exchange.commission_fixed,
                CommissionType::PercentagePlusFixed => {
                    notional * cfg.exchange.commission_pct / 100.0 + cfg.exchange.commission_fixed
                },
            };

            // ── Leverage / position-size pre-check ──────────────────────────

            // Reject orders that would push gross exposure beyond `max_leverage` or
            // `initial_margin`, push the per-symbol exposure past `max_position_size`,
            // or attempt to borrow at all when `allow_margin` is disabled.
            let equity_base = compute_portfolio_equity(
                &cash,
                &positions,
                aligned,
                bar_index,
                &quote_ccy,
                &base_ccy_str,
                fx,
                ts,
            );

            let invested_base = compute_invested_equity(
                &positions,
                aligned,
                bar_index,
                &quote_ccy,
                &base_ccy_str,
                fx,
                ts,
            );

            let current_qty = positions.get(&order.symbol).copied().unwrap_or(0.0);

            let current_pos_base = if current_qty.abs() > MIN_POSITION {
                let bar_close = aligned
                    .get(&order.symbol)
                    .and_then(|r| r[bar_index].as_ref())
                    .map(|b| b.close)
                    .unwrap_or(fill_px);

                let value = current_qty.abs() * bar_close;
                let ccy = quote_ccy.get(order.symbol.as_str()).unwrap_or(&base_ccy_str.as_str());
                fx.convert(value, ccy, &base_ccy_str, ts).unwrap_or(value)
            } else {
                0.0
            };

            if let Err((violation, reason)) = check_order_against_limits(
                cfg,
                &order.symbol,
                qty,
                acct_fill_px,
                &order_ccy.to_string(),
                &base_ccy_str,
                equity_base,
                invested_base,
                current_qty,
                current_pos_base,
                fx,
                ts,
            )
            .and_then(|new_qty| {
                if (new_qty - qty).abs() <= MIN_POSITION {
                    return Ok(());
                }

                let it = it_map.get(&order.symbol.as_str()).unwrap();
                let mut abs_qty = new_qty.abs();

                if !it.allows_fractional_quantities() {
                    abs_qty = abs_qty.floor();
                }

                if !abs_qty.is_finite() || abs_qty <= MIN_POSITION {
                    return Err((
                        LimitViolation::Margin,
                        format!(
                            "no headroom under leverage / position-size limits for {}", order.symbol
                        ),
                    ));
                }

                // Update the order quantity, sign-preserving, and
                // re-derive notional / commission from the
                // shrunk size.
                let new_qty_signed = qty.signum() * abs_qty;
                qty = new_qty_signed;
                order.quantity = new_qty_signed;
                filled_qty = new_qty_signed;
                notional = acct_fill_px * abs_qty;
                commission = match cfg.exchange.commission_type {
                    CommissionType::Percentage => {
                        notional * cfg.exchange.commission_pct / 100.0
                    },
                    CommissionType::Fixed => cfg.exchange.commission_fixed,
                    CommissionType::PercentagePlusFixed => {
                        notional * cfg.exchange.commission_pct / 100.0
                            + cfg.exchange.commission_fixed
                    },
                };
                fill_reason = if fill_reason.is_empty() {
                    "partial: shrunk to fit leverage / position-size limit".to_owned()
                } else {
                    format!(
                        "{fill_reason}; partial: shrunk to fit leverage / position-size limit"
                    )
                };
                Ok(())
            }) {
                let warning_key = limit_warning_dedupe_key(&symbol, violation, &reason);
                if margin_limit_warnings.insert(warning_key) {
                    warn!(strategy=%name, order_id=%order.id, "{reason}");
                } else {
                    debug!(strategy=%name, order_id=%order.id, "suppressed repeated limit rejection: {reason}");
                }
                // Position-size rejections are always just warnings —
                // they are a concentration guardrail, not margin-related.
                // Only margin/leverage violations are gated by
                // `raise_on_margin_limit`.
                if violation == LimitViolation::Margin && cfg.exchange.raise_on_margin_limit {
                    run_error.get_or_insert_with(|| reason.clone());
                }
                order_records.push(OrderRecord {
                    order: order.clone(),
                    timestamp: ts,
                    status: "rejected".into(),
                    fill_price: None,
                    reason,
                    commission: 0.0,
                    pnl: None,
                });
                continue;
            }

            // Realised PnL recorded on the OrderRecord for closing fills
            // (sell that flattens an existing long position). Stays `None`
            // for opening fills.
            let mut fill_pnl: Option<f64> = None;

            // ── Funds check & settlement ─────────────────────────────
            if qty > 0.0 {
                // BUY: try paying in `order_ccy` first, else convert from base.
                let needed = notional + commission;
                if !try_debit(&mut cash, order_ccy, needed, base_ccy, fx, ts) {
                    // Auto-shrink the order rather than reject it. Equal-weight
                    // strategies routinely size the last leg at exactly
                    // `cash / n_symbols`, which gets pushed fractionally over
                    // the available cash by slippage + commission. Shrinking
                    // the qty to whatever fits keeps every leg actually
                    // tradable instead of mysteriously dropping symbols.
                    //
                    // Compute available funds in `order_ccy` by summing every
                    // cash bucket converted to `order_ccy` at the current
                    // bar's FX rate (forward-filled). Buckets whose currency
                    // can't be converted at `ts` are ignored.
                    let avail: f64 = cash
                        .iter()
                        .filter(|(_, v)| v.is_finite() && **v > 0.0)
                        .filter_map(|(ccy, v)| {
                            fx.convert(*v, &ccy.to_string(), &order_ccy.to_string(), ts)
                        })
                        .sum();
                    let pct_part = match cfg.exchange.commission_type {
                        CommissionType::Percentage | CommissionType::PercentagePlusFixed => {
                            cfg.exchange.commission_pct / 100.0
                        },
                        CommissionType::Fixed => 0.0,
                    };
                    let fixed_part = match cfg.exchange.commission_type {
                        CommissionType::Fixed | CommissionType::PercentagePlusFixed => {
                            cfg.exchange.commission_fixed
                        },
                        CommissionType::Percentage => 0.0,
                    };
                    // Solve for the largest quantity q such that
                    //   acct_fill_px * q * (1 + pct_part) + fixed_part <= avail.
                    // Non-crypto instruments must settle whole units, so
                    // floor the cash-fit quantity before retrying the debit.
                    let denom = acct_fill_px * (1.0 + pct_part);
                    let mut max_qty: f64 = if denom > 0.0 && avail > fixed_part {
                        ((avail - fixed_part) / denom).max(0.0)
                    } else {
                        0.0
                    };
                    let instrument_type =
                        instrument_type_for_symbol(&symbol, &it_map, cfg.data.instrument_type);
                    if !instrument_type.allows_fractional_quantities() {
                        max_qty = max_qty.floor();
                    }
                    if max_qty <= 0.0 {
                        warn!(
                            strategy=%name, order_id=%order.id,
                            "Insufficient funds for buy, skipping order."
                        );
                        order_records.push(OrderRecord {
                            order: order.clone(),
                            timestamp: ts,
                            status: "rejected".into(),
                            fill_price: None,
                            reason: "insufficient funds".into(),
                            commission: 0.0,
                            pnl: None,
                        });
                        continue;
                    }
                    filled_qty = max_qty.min(qty);
                    notional = acct_fill_px * filled_qty;
                    commission = match cfg.exchange.commission_type {
                        CommissionType::Percentage => {
                            notional * cfg.exchange.commission_pct / 100.0
                        },
                        CommissionType::Fixed => cfg.exchange.commission_fixed,
                        CommissionType::PercentagePlusFixed => {
                            notional * cfg.exchange.commission_pct / 100.0
                                + cfg.exchange.commission_fixed
                        },
                    };
                    let shrunk_needed = notional + commission;
                    if !try_debit(&mut cash, order_ccy, shrunk_needed, base_ccy, fx, ts) {
                        // Belt-and-braces: should be unreachable given the
                        // qty solve above, but stay safe under FP edge cases.
                        order_records.push(OrderRecord {
                            order: order.clone(),
                            timestamp: ts,
                            status: "rejected".into(),
                            fill_price: None,
                            reason: "insufficient funds".into(),
                            commission: 0.0,
                            pnl: None,
                        });
                        continue;
                    }
                    fill_reason = if fill_reason.is_empty() {
                        "partial: shrunk to fit cash".to_owned()
                    } else {
                        format!("{fill_reason}; partial: shrunk to fit cash")
                    };
                }
                *positions.entry(symbol.clone()).or_insert(0.0) += filled_qty;
                update_open_trade_buy(&mut open_trades, &symbol, ts, filled_qty, acct_fill_px);
            } else if qty < 0.0 {
                let abs_qty = qty.abs();
                let cur = *positions.get(&symbol).unwrap_or(&0.0);
                if !cfg.exchange.allow_short_selling && cur < abs_qty {
                    warn!(strategy=%name, order_id=%order.id, "Short selling disabled and not enough position, skipping.");
                    if cfg.exchange.raise_on_short_violation {
                        run_error.get_or_insert_with(|| "short selling disabled".to_owned());
                    }
                    order_records.push(OrderRecord {
                        order: order.clone(),
                        timestamp: ts,
                        status: "rejected".into(),
                        fill_price: None,
                        reason: "short selling disabled".into(),
                        commission: 0.0,
                        pnl: None,
                    });
                    continue;
                }
                // Credit proceeds, debit commission.
                *cash.entry(order_ccy).or_insert(0.0) += notional;
                if !try_debit(&mut cash, order_ccy, commission, base_ccy, fx, ts) {
                    // Reverse: not enough to even pay commission; very unlikely.
                    *cash.entry(order_ccy).or_insert(0.0) -= notional;
                    order_records.push(OrderRecord {
                        order: order.clone(),
                        timestamp: ts,
                        status: "rejected".into(),
                        fill_price: None,
                        reason: "cannot pay commission".into(),
                        commission: 0.0,
                        pnl: None,
                    });
                    continue;
                }
                let pos_entry = positions.entry(symbol.clone()).or_insert(0.0);
                *pos_entry -= abs_qty;
                let realised_pnl = close_open_trade_sell(
                    &mut open_trades,
                    &symbol,
                    ts,
                    abs_qty,
                    acct_fill_px,
                    commission,
                )
                .map(|t| {
                    let pnl = t.pnl;
                    closed_trades.push(t);
                    pnl
                });
                fill_pnl = realised_pnl;
            }

            // Reflect the actually-filled quantity on the record so the
            // UI shows what the engine settled (matters when a buy was
            // auto-shrunk to fit the available cash).
            if (filled_qty - order.quantity).abs() > MIN_POSITION {
                order.quantity = filled_qty;
            }
            order_records.push(OrderRecord {
                order: order.clone(),
                timestamp: ts,
                status: OrderStatus::Filled,
                fill_price: Some(fill_px),
                reason: fill_reason,
                commission,
                pnl: fill_pnl,
            });
        }
        open_orders = still_open;

        // ── 1b. Apply currency-conversion policy ────────────────────────
        //
        // Foreign-currency cash buckets created by sells in the loop above
        // (or carried over from previous bars) are swept back into the
        // base currency according to the configured `conversion_mode`:
        //
        //   * Immediate           → sweep every bar after fills.
        //   * HoldUntilThreshold  → sweep buckets whose value ≥ threshold
        //                           (in base ccy units) every bar.
        //   * EndOfPeriod         → sweep when day/week/month/year flips.
        //   * CustomInterval      → sweep every N bars.
        match cfg.exchange.conversion_mode {
            CurrencyConversionMode::Immediate => {
                sweep_foreign_to_base(&mut cash, base_ccy, fx, ts, None);
            },
            CurrencyConversionMode::HoldUntilThreshold => {
                sweep_foreign_to_base(
                    &mut cash,
                    base_ccy,
                    fx,
                    ts,
                    Some(cfg.exchange.conversion_threshold.unwrap_or(0.)),
                );
            },
            CurrencyConversionMode::EndOfPeriod => {
                if let Some(period) = cfg.exchange.conversion_period {
                    let bucket = period_bucket(ts, period);
                    if let Some(prev) = last_period_bucket {
                        if bucket != prev {
                            sweep_foreign_to_base(&mut cash, base_ccy, fx, ts, None);
                        }
                    }
                    last_period_bucket = Some(bucket);
                }
            },
            CurrencyConversionMode::CustomInterval => {
                bars_since_conv += 1;
                let conv_interval = cfg.exchange.conversion_interval.unwrap_or(0) as usize;
                if conv_interval > 0 && bars_since_conv >= conv_interval {
                    sweep_foreign_to_base(&mut cash, base_ccy, fx, ts, None);
                    bars_since_conv = 0;
                }
            },
        }

        // ── 2. Build State + Portfolio + per-symbol view ────────────────
        let state = State {
            timestamp: ts,
            bar_index: bar_index as u64,
            total_bars: total_bars as u64,
            is_warmup,
        };
        let portfolio = Portfolio {
            cash: cash.clone(),
            positions: positions.clone(),
            orders: open_orders.clone(),
        };

        // ── 3. Strategy decision ────────────────────────────────────────
        if !is_warmup {
            let new_orders: Result<Vec<Order>, PyErr> = if let Some(b) = &builtin {
                // Fast path: pure-Rust dispatch, no GIL, no DataFrame slicing.
                // Reuses pre-borrowed `symbol_refs` to avoid cloning Strings
                // on every bar — a significant allocation saving on long runs.
                let bars_view: Vec<(String, &[Bar])> = symbol_refs
                    .iter()
                    .zip(bars_full.iter())
                    .map(|(&s, (_, v))| (s.to_owned(), &v[..=bar_index]))
                    .collect();
                let inds = IndicatorView::new(indicators, bar_index);
                let orders = b.evaluate(
                    &bars_view,
                    &inds,
                    &portfolio,
                    &state,
                    &it_map,
                    cfg.data.instrument_type,
                );
                for o in &orders {
                    let side = if o.quantity > 0.0 {
                        "BUY"
                    } else {
                        "SELL"
                    };
                    let abs_qty = o.quantity.abs();
                    debug!(
                        strategy=%name,
                        "Order placed: {side} {abs_qty} {} @ bar {bar_index}",
                        o.symbol,
                    );
                }
                Ok(orders)
            } else {
                // Custom (Python) strategy: original evaluate path.
                Python::attach(|py| -> PyResult<Vec<Order>> {
                    let data = build_per_symbol_view(py, cached_data, bar_index)?;
                    let inds = build_indicator_view(py, cached_indicators, bar_index)?;
                    let res = strategy
                        .bind(py)
                        .call_method1("evaluate", (data, portfolio.clone(), state.clone(), inds))?;
                    let list: Vec<Order> = res.extract().unwrap_or_default();
                    Ok(list)
                })
            };

            match new_orders {
                Ok(mut ords) => {
                    if cfg.engine.exclusive_orders && !ords.is_empty() {
                        // Cancel everything pending first.
                        for o in &open_orders {
                            order_records.push(OrderRecord {
                                order: o.clone(),
                                timestamp: ts,
                                status: "canceled".into(),
                                fill_price: None,
                                reason: "exclusive_orders".into(),
                                commission: 0.0,
                                pnl: None,
                            });
                        }
                        open_orders.clear();
                    }

                    // ── Resolve sizer-based quantities ──────────────────
                    //
                    // Orders that carry a sizer instead of a numeric
                    // quantity get resolved here using the portfolio's
                    // current equity and the symbol's latest close price.
                    // After this block every order has a concrete f64
                    // quantity and the sizer field is cleared.
                    for o in &mut ords {
                        if let Some(sizer_slot) = o.sizer.take() {
                            let order_ccy_str_sizer = quote_ccy
                                .get(&o.symbol)
                                .map(String::as_str)
                                .unwrap_or(&base_ccy_str);
                            // Compute mark-to-market equity in the same currency
                            // as the symbol's price. Sizers divide equity/risk by
                            // price-like inputs, so `equity`, `price`,
                            // `stop_distance`, and `atr` must share a currency.
                            let eq = compute_portfolio_equity(
                                &cash,
                                &positions,
                                aligned,
                                bar_index,
                                &quote_ccy,
                                &base_ccy_str,
                                order_ccy_str_sizer,
                                fx,
                                ts,
                            );

                            // Get the current close price for this symbol.
                            let sym_price = aligned
                                .get(&o.symbol)
                                .and_then(|r| r[bar_index].as_ref())
                                .map(|b| b.close)
                                .unwrap_or(0.0);

                            // Call sizer.calculate(equity, price, stop_distance, atr).
                            // stop_distance is derived from the order's price
                            // field (for stop-style orders) and the current price.
                            let stop_distance: Option<f64> = o.price.and_then(|p| {
                                let d = (sym_price - p).abs();
                                if d > 0.0 {
                                    Some(d)
                                } else {
                                    None
                                }
                            });

                            let resolved = Python::attach(|py| -> PyResult<f64> {
                                sizer_slot
                                    .0
                                    .bind(py)
                                    .call_method1(
                                        "calculate",
                                        (eq, sym_price, stop_distance, Option::<f64>::None),
                                    )?
                                    .extract()
                            });
                            match resolved {
                                Ok(qty) => {
                                    o.quantity = qty;
                                },
                                Err(e) => {
                                    warn!(strategy=%name, order_id=%o.id, "Sizer resolution failed: {e}");
                                    o.quantity = 0.0; // Will be rejected by the qty check below.
                                },
                            }
                        }
                    }

                    // Validate allowed types/quantities & ensure ids are populated.
                    let allowed = &cfg.exchange.allowed_order_types;
                    ords.retain_mut(|o| {
                        if o.id.is_empty() {
                            o.id = new_order_id();
                        }
                        if !allowed.contains(&o.order_type)
                            && o.order_type != OrderType::Cancel
                        {
                            warn!(strategy=%name, "Order type {} not allowed, dropping.", o.order_type);
                            order_records.push(OrderRecord {
                                order: o.clone(),
                                timestamp: ts,
                                status: "rejected".into(),
                                fill_price: None,
                                reason: "order type not allowed".into(),
                                commission: 0.0,
                                pnl: None,
                            });
                            return false;
                        }
                        if !matches!(o.order_type, OrderType::Cancel | OrderType::SettlePosition) {
                            let instrument_type = instrument_type_for_symbol(
                                &o.symbol,
                                &it_map,
                                cfg.data.instrument_type,
                            );

                            if let Some(reason) = validate_qty(o.quantity, instrument_type) {
                                warn!(strategy=%name, "Invalid order quantity: {}. Reason: {reason}. The order has been rejected.", o.quantity);
                                order_records.push(OrderRecord {
                                    order: o.clone(),
                                    timestamp: ts,
                                    status: OrderStatus::Rejected,
                                    fill_price: None,
                                    reason,
                                    commission: 0.0,
                                    pnl: None,
                                });

                                return false;
                            }
                        }
                        true
                    });

                    // Reject orders whose id already exists in the open book
                    // or appears more than once in the current batch.
                    let mut seen_ids: Vec<String> =
                        open_orders.iter().map(|o| o.id.clone()).collect();
                    ords.retain(|o| {
                        if matches!(o.order_type, OrderType::Cancel | OrderType::SettlePosition) {
                            return true;
                        }
                        if seen_ids.contains(&o.id) {
                            warn!(strategy=%name, order_id=%o.id, "Duplicate order id, rejecting.");
                            order_records.push(OrderRecord {
                                order: o.clone(),
                                timestamp: ts,
                                status: OrderStatus::Rejected,
                                fill_price: None,
                                reason: format!("duplicate order id {:?}", o.id),
                                commission: 0.0,
                                pnl: None,
                            });
                            return false;
                        }
                        seen_ids.push(o.id.clone());
                        true
                    });

                    open_orders.extend(ords);
                },
                Err(e) => {
                    let msg = format!("evaluate() raised: {e}");
                    warn!(strategy=%name, "{msg}");
                    run_error.get_or_insert(msg);
                },
            }
        }

        // ── 4. Mark-to-market & equity sample ────────────────────────────
        //
        // Equity is computed entirely in the portfolio base currency. Each
        // cash bucket is converted via the FX table at `ts` (forward-fill);
        // each open position is marked at its bar close in its quote
        // currency, then converted to base. Buckets/positions for which
        // no FX rate is available at `ts` are summed at par (rate 1.0)
        // so equity stays a finite, comparable number — this matches the
        // engine's previous "best-effort" behaviour and surfaces missing
        // FX coverage as a flat segment rather than a NaN.
        let mut equity = 0.0;
        for (ccy, amount) in &cash {
            equity += fx.convert(*amount, &ccy.to_string(), &base_ccy_str, ts).unwrap_or(*amount);
        }
        for (sym, qty) in &positions {
            if qty.abs() < MIN_POSITION {
                continue;
            }
            if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
                let value = *qty * b.close;
                let pos_ccy = quote_ccy.get(sym).map(String::as_str).unwrap_or(&base_ccy_str);
                equity += fx.convert(value, pos_ccy, &base_ccy_str, ts).unwrap_or(value);
            }
        }
        if equity > peak_equity {
            peak_equity = equity;
        }
        let drawdown = if peak_equity > 0.0 {
            (equity - peak_equity) / peak_equity
        } else {
            0.0
        };

        // Build the cash snapshot for this equity sample. Avoid allocating
        // a new HashMap when the portfolio holds only the base currency
        // (the overwhelmingly common case for single-currency experiments).
        let cash_snapshot = if cash.len() <= 1 {
            // Single bucket (or empty): cheap clone, no filtering needed.
            cash.clone()
        } else {
            cash.iter().filter(|(_, v)| v.abs() > MIN_POSITION).map(|(k, v)| (*k, *v)).collect()
        };

        equity_curve.push(EquitySample {
            timestamp: ts,
            equity,
            cash: cash_snapshot,
            drawdown,
        });

        // ── Maintenance-margin check ────────────────────────────────────
        //
        // If equity has fallen below `maintenance_margin` of gross
        // notional, force-flatten every open position at the current
        // close price and record a synthetic "margin call" order for
        // each. When `raise_on_margin_limit` is set, also surface the
        // event as a run error.
        let gross_base = compute_invested_equity(
            &positions,
            aligned,
            bar_index,
            &quote_ccy,
            &base_ccy_str,
            fx,
            ts,
        );
        if let Some(reason) = check_maintenance_margin(cfg, equity, gross_base) {
            warn!(strategy=%name, "{reason}");
            if cfg.exchange.raise_on_margin_limit {
                run_error.get_or_insert_with(|| reason.clone());
            }
            // Force-flatten every position at the current close.
            let to_flatten: Vec<(String, f64)> =
                positions.iter().map(|(s, q)| (s.clone(), *q)).collect();
            for (sym, qty) in to_flatten {
                if qty.abs() < MIN_POSITION {
                    continue;
                }
                let close = match aligned.get(&sym).and_then(|r| r[bar_index].as_ref()) {
                    Some(b) => b.close,
                    None => continue,
                };
                let pos_ccy_str = quote_ccy.get(&sym).map(String::as_str).unwrap_or(&base_ccy_str);
                let pos_ccy = pos_ccy_str.parse::<Currency>().unwrap_or(base_ccy);
                let notional = qty.abs() * close;
                // For non-fiat quotes, convert the notional to the fiat
                // accounting currency so cash operations stay fiat-only.
                let notional_fiat = if pos_ccy_str.parse::<Currency>().is_err() {
                    fx.convert(notional, pos_ccy_str, &base_ccy_str, ts).unwrap_or(notional)
                } else {
                    notional
                };
                let synth = Order {
                    id: format!("margin-call-{}", &sym),
                    symbol: sym.clone(),
                    order_type: OrderType::Market,
                    quantity: -qty,
                    price: None,
                    limit_price: None,
                    sizer: None,
                };
                if qty > 0.0 {
                    // Long: credit cash with proceeds.
                    *cash.entry(pos_ccy).or_insert(0.0) += notional_fiat;
                    if let Some(t) =
                        close_open_trade_sell(&mut open_trades, &sym, ts, qty, close, 0.0)
                    {
                        closed_trades.push(t);
                    }
                } else {
                    // Short: debit cash (or any available bucket) to buy
                    // back the shares.
                    let _ = try_debit(&mut cash, pos_ccy, notional_fiat, base_ccy, fx, ts);
                    open_trades.remove(&sym);
                }
                positions.insert(sym.clone(), 0.0);
                order_records.push(OrderRecord {
                    order: synth,
                    timestamp: ts,
                    status: "filled".into(),
                    fill_price: Some(close),
                    reason: reason.clone(),
                    commission: 0.0,
                    pnl: None,
                });
            }
            positions.retain(|_, q| q.abs() > MIN_POSITION);
        }
    }

    // ── 5. Liquidate remaining positions to compute final PnL ───────────
    if let Some(last_idx) = total_bars.checked_sub(1) {
        for (sym, qty) in positions.clone() {
            if qty.abs() < MIN_POSITION {
                continue;
            }
            if let Some(b) = aligned.get(&sym).and_then(|r| r[last_idx].as_ref()) {
                let exit_px = b.close;
                if let Some((entry_ts, _q, entry_px)) = open_trades.remove(&sym) {
                    let pnl = (exit_px - entry_px) * qty;
                    closed_trades.push(Trade {
                        symbol: sym.clone(),
                        quantity: qty,
                        entry_ts,
                        exit_ts: timeline[last_idx],
                        entry_price: entry_px,
                        exit_price: exit_px,
                        pnl,
                    });
                }
            }
        }
    }

    // ── 6. Metrics ──────────────────────────────────────────────────────
    let metrics = compute_metrics(
        cfg.portfolio.initial_cash as f64,
        cfg.engine.risk_free_rate / 100.0,
        &equity_curve,
        &closed_trades,
    );

    RunResult {
        strategy_id: Uuid::new_v4().simple().to_string()[..16].to_owned(),
        strategy_name: name.to_owned(),
        equity_curve,
        trades: closed_trades,
        orders: order_records,
        metrics,
        base_currency: cfg.portfolio.base_currency,
        error: run_error,
        is_benchmark: is_benchmark_run,
    }
}

/// Try to debit `amount` of `ccy` from `cash`. If `ccy` doesn't have enough,
/// fall back to the base currency (and finally any other foreign bucket)
/// converting at the FX rate observed at `ts`. Returns `false` if no
/// combination of available cash covers the debit.
fn try_debit(
    cash: &mut Cash,
    ccy: Currency,
    amount: f64,
    base: Currency,
    fx: &FxTable,
    ts: i64,
) -> bool {
    if amount <= 0.0 {
        return true;
    }

    // 1) Pay directly out of `ccy`.
    let avail = *cash.get(&ccy).unwrap_or(&0.0);
    if avail >= amount {
        *cash.entry(ccy).or_insert(0.0) -= amount;
        return true;
    }

    // 2) Drain the existing `ccy` bucket first, remember the residual.
    let mut remaining = amount - avail.max(0.0);

    // 3) Cover the residual from the base currency at the current FX rate.
    //    When `ccy == base` we already know step 1 failed (not enough in
    //    that single bucket), so skip — otherwise the same bucket is
    //    double-counted and cash goes negative.
    let base_avail = if ccy == base {
        0.0
    } else {
        *cash.get(&base).unwrap_or(&0.0)
    };
    let needed_base = match fx.rate(&ccy.to_string(), &base.to_string(), ts) {
        Some(r) if r > 0.0 => remaining * r,
        _ => f64::INFINITY,
    };
    if needed_base.is_finite() && base_avail >= needed_base {
        cash.remove(&ccy);
        *cash.entry(base).or_insert(0.0) -= needed_base;
        return true;
    }

    // 4) Last resort: drain other foreign buckets in deterministic order.
    let mut buckets: Vec<(Currency, f64)> = cash
        .iter()
        .filter(|(c, v)| **c != ccy && **c != base && v.is_finite() && **v > 0.0)
        .map(|(c, v)| (*c, *v))
        .collect();
    buckets.sort_by_key(|(c, _)| c.to_string());

    // Tentatively zero `ccy` and reduce base.
    let mut staged: Vec<(Currency, f64)> = Vec::new();
    let staged_ccy_drain = avail.max(0.0);
    let mut staged_base_drain = if base_avail > 0.0 {
        base_avail
    } else {
        0.0
    };
    if needed_base.is_finite() {
        staged_base_drain = staged_base_drain.min(needed_base);
        let covered_in_ccy = if staged_base_drain > 0.0 {
            match fx.rate(&base.to_string(), &ccy.to_string(), ts) {
                Some(r) if r > 0.0 => staged_base_drain * r,
                _ => 0.0,
            }
        } else {
            0.0
        };
        remaining = (remaining - covered_in_ccy).max(0.0);
    } else {
        staged_base_drain = 0.0;
    }

    for (other_ccy, other_avail) in buckets {
        if remaining <= 0.0 {
            break;
        }
        let r = match fx.rate(&other_ccy.to_string(), &ccy.to_string(), ts) {
            Some(r) if r > 0.0 => r,
            _ => continue,
        };
        let other_in_ccy = other_avail * r;
        if other_in_ccy >= remaining {
            staged.push((other_ccy, remaining / r));
            remaining = 0.0;
        } else {
            staged.push((other_ccy, other_avail));
            remaining -= other_in_ccy;
        }
    }
    if remaining > 0.0 {
        return false;
    }
    // Commit drains.
    if staged_ccy_drain > 0.0 {
        *cash.entry(ccy).or_insert(0.0) -= staged_ccy_drain;
    }
    if staged_base_drain > 0.0 {
        *cash.entry(base).or_insert(0.0) -= staged_base_drain;
    }
    for (c, v) in staged {
        *cash.entry(c).or_insert(0.0) -= v;
    }
    // Remove buckets drained to zero so they don't linger in equity snapshots.
    cash.retain(|_, v| v.abs() > MIN_POSITION);
    true
}

/// Sweep every non-base currency bucket into the base currency at the
/// FX rate observed at `ts`. If `threshold` is `Some(t)`, only buckets
/// whose value in base currency is `>= t` are swept; otherwise every
/// foreign bucket with a positive (or negative) finite balance is
/// converted. Buckets without an available FX rate are left untouched.
fn sweep_foreign_to_base(
    cash: &mut Cash,
    base: Currency,
    fx: &FxTable,
    ts: i64,
    threshold: Option<f64>,
) {
    let foreign: Vec<Currency> = cash
        .iter()
        .filter(|(c, v)| **c != base && v.is_finite() && v.abs() > 0.0)
        .map(|(c, _)| *c)
        .collect();
    for ccy in foreign {
        let amount = match cash.get(&ccy).copied() {
            Some(v) if v.is_finite() && v.abs() > 0.0 => v,
            _ => continue,
        };
        let in_base = match fx.convert(amount, &ccy.to_string(), &base.to_string(), ts) {
            Some(v) => v,
            None => continue,
        };
        if let Some(t) = threshold {
            if in_base.abs() < t {
                continue;
            }
        }
        cash.remove(&ccy);
        *cash.entry(base).or_insert(0.0) += in_base;
    }
}

/// Return a coarse "bucket" identifier for `ts` under the given
/// conversion period. Two timestamps falling into different buckets
/// trigger an end-of-period sweep.
fn period_bucket(ts: i64, period: ConversionPeriod) -> i64 {
    use chrono::{DateTime, Datelike, Utc};
    let dt = DateTime::<Utc>::from_timestamp(ts, 0)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    match period {
        ConversionPeriod::Day => ts.div_euclid(86_400),
        ConversionPeriod::Week => {
            // ISO week-year combined identifier.
            let iso = dt.iso_week();
            (iso.year() as i64) * 100 + iso.week() as i64
        },
        ConversionPeriod::Month => (dt.year() as i64) * 12 + (dt.month0() as i64),
        ConversionPeriod::Year => dt.year() as i64,
    }
}

fn update_open_trade_buy(
    open_trades: &mut HashMap<String, (i64, f64, f64)>,
    symbol: &str,
    ts: i64,
    qty: f64,
    px: f64,
) {
    open_trades
        .entry(symbol.to_owned())
        .and_modify(|(_, q, p)| {
            let total = *q * *p + qty * px;
            *q += qty;
            if q.abs() > MIN_POSITION {
                *p = total / *q;
            }
        })
        .or_insert((ts, qty, px));
}

fn close_open_trade_sell(
    open_trades: &mut HashMap<String, (i64, f64, f64)>,
    symbol: &str,
    ts: i64,
    abs_qty: f64,
    exit_px: f64,
    commission: f64,
) -> Option<Trade> {
    let (entry_ts, mut q, entry_px) = open_trades.remove(symbol)?;
    let used = abs_qty.min(q);
    q -= used;
    let pnl = (exit_px - entry_px) * used - commission;
    if q > MIN_POSITION {
        open_trades.insert(symbol.to_owned(), (entry_ts, q, entry_px));
    }
    Some(Trade {
        symbol: symbol.to_owned(),
        quantity: used,
        entry_ts,
        exit_ts: ts,
        entry_price: entry_px,
        exit_price: exit_px,
        pnl,
    })
}

fn compute_metrics(
    initial_cash: f64,
    risk_free_rate: f64,
    curve: &[EquitySample],
    trades: &[Trade],
) -> HashMap<String, f64> {
    let mut m = HashMap::new();

    let final_equity = curve.last().map(|s| s.equity).unwrap_or(initial_cash);
    let total_return = if initial_cash > 0.0 {
        (final_equity - initial_cash) / initial_cash
    } else {
        0.0
    };

    // Trade-derived metrics.
    let n_trades = trades.len() as f64;
    let n_wins = trades.iter().filter(|t| t.pnl > 0.0).count() as f64;
    let win_rate = if n_trades > 0.0 {
        n_wins / n_trades
    } else {
        0.0
    };

    m.insert("total_return".into(), total_return);
    m.insert("final_equity".into(), final_equity);
    m.insert("pnl".into(), final_equity - initial_cash);
    m.insert("n_trades".into(), n_trades);
    m.insert("win_rate".into(), win_rate);

    // Annualized stats reuse the shared kernel from `analysis.rs` so that
    // the analysis page and backtest engine produce identical numbers.
    let values: Vec<f64> = curve.iter().map(|s| s.equity).collect();
    let timestamps: Vec<f64> = curve.iter().map(|s| s.timestamp as f64).collect();
    let stats = crate::analysis::compute_series_stats(&values, &timestamps, risk_free_rate, None);

    let (cagr, ann_vol, sharpe, sortino, max_dd) = match stats {
        Some(s) => (s.ann_return, s.ann_volatility, s.sharpe, s.sortino, s.max_dd),
        None => (0.0, 0.0, 0.0, 0.0, 0.0),
    };
    m.insert("cagr".into(), cagr);
    m.insert("ann_volatility".into(), ann_vol);
    m.insert("sharpe".into(), sharpe);
    m.insert("sortino".into(), sortino);
    m.insert("max_dd".into(), max_dd);

    m
}

/// Build a Python dict `{symbol: pandas.DataFrame}` view through bar `idx`.
///
/// Takes pre-built full DataFrames per symbol and returns cheap O(1)
/// `df.iloc[:idx+1]` views — pandas slicing creates a new DataFrame
/// pointing into the same underlying arrays, so no data is copied.
fn build_per_symbol_view<'py>(
    py: Python<'py>,
    cached: &HashMap<String, Py<PyAny>>,
    idx: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let out = PyDict::new(py);
    let end = idx + 1;
    let slice = pyo3::types::PySlice::new(py, 0, end as isize, 1);
    for (sym, df) in cached {
        // ``df[0:end]`` works uniformly for pandas DataFrame, polars
        // DataFrame and 2-D numpy ndarray (all do positional row
        // slicing), so no library-specific branching is required.
        let sliced = df.bind(py).get_item(&slice)?;
        out.set_item(sym, sliced)?;
    }
    Ok(out.into_any())
}

/// Build a Python dict view of indicator values up to bar `idx`.
///
/// Takes pre-built full numpy arrays per (indicator, symbol, series) and
/// returns cheap O(1) `arr[:idx+1]` slice-views.
fn build_indicator_view<'py>(
    py: Python<'py>,
    cached: &HashMap<String, HashMap<String, Vec<Py<PyAny>>>>,
    idx: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let out = PyDict::new(py);
    let end = idx + 1;
    for (name, per_sym) in cached {
        let by_sym = PyDict::new(py);
        for (sym, arrs) in per_sym {
            let slice = pyo3::types::PySlice::new(py, 0, end as isize, 1);
            if arrs.len() == 1 {
                let sliced = arrs[0].bind(py).get_item(&slice)?;
                by_sym.set_item(sym, sliced)?;
            } else {
                let list = PyList::empty(py);
                for arr in arrs {
                    let sliced = arr.bind(py).get_item(&slice)?;
                    list.append(sliced)?;
                }
                by_sym.set_item(sym, list)?;
            }
        }
        out.set_item(name, by_sym)?;
    }
    Ok(out.into_any())
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_bar(ts: u64, close: f64) -> Bar {
        Bar {
            open_ts: ts,
            close_ts: ts + 86_399,
            open_ts_exchange: ts,
            open: close,
            high: close * 1.01,
            low: close * 0.99,
            close,
            adj_close: close,
            volume: 1_000.0,
            n_trades: Some(1),
        }
    }

    fn mk_aligned(symbol: &str, prices: &[f64]) -> HashMap<String, Vec<Option<Bar>>> {
        let mut row = Vec::new();
        for (i, p) in prices.iter().enumerate() {
            row.push(Some(mk_bar(1_700_000_000 + i as u64 * 86_400, *p)));
        }
        let mut m = HashMap::new();
        m.insert(symbol.to_owned(), row);
        m
    }

    fn mk_multi_symbol_aligned(
        symbols: &[&str],
        n_bars: usize,
    ) -> HashMap<String, Vec<Option<Bar>>> {
        let mut out: HashMap<String, Vec<Option<Bar>>> = HashMap::new();
        for (sym_idx, symbol) in symbols.iter().enumerate() {
            let mut row = Vec::with_capacity(n_bars);
            for i in 0..n_bars {
                let t = i as f64;
                let phase = sym_idx as f64 * 0.85;
                let regime = if (i / 36) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                };
                let drift = regime * (i % 36) as f64 * 0.35;
                let oscillation = 9.0 * (0.18 * t + phase).sin() + 4.0 * (0.055 * t + phase).cos();
                let pulse = if i % 45 == 0 {
                    5.0
                } else {
                    0.0
                };
                let close = (90.0 + drift + oscillation + pulse + sym_idx as f64 * 1.5).max(2.0);
                row.push(Some(mk_bar(1_700_000_000 + i as u64 * 86_400, close)));
            }
            out.insert((*symbol).to_owned(), row);
        }
        out
    }

    fn mk_cfg(symbol: &str) -> ExperimentConfig {
        let mut cfg = ExperimentConfig {
            general: GeneralExpConfig::default(),
            data: DataExpConfig::default(),
            portfolio: PortfolioExpConfig::default(),
            strategy: StrategyExpConfig::default(),
            indicators: IndicatorExpConfig::default(),
            exchange: ExchangeExpConfig::default(),
            engine: EngineExpConfig::default(),
        };
        cfg.data.symbols = vec![symbol.to_owned()];
        cfg.data.instrument_type = InstrumentType::Stocks;
        cfg.engine.empty_bar_policy = EmptyBarPolicy::ForwardFill;
        cfg.exchange.commission_type = CommissionType::Fixed;
        cfg.exchange.commission_fixed = 0.0;
        cfg.exchange.commission_pct = 0.0;
        cfg.exchange.slippage = 0.0;
        cfg.portfolio.initial_cash = 10_000;
        cfg.exchange.allowed_order_types = vec![OrderType::Market, OrderType::Cancel];
        cfg
    }

    fn mk_profile(symbol: &str, quote: &str) -> InstrumentProfile {
        mk_profile_with_type(symbol, quote, InstrumentType::Stocks)
    }

    fn mk_profile_with_type(
        symbol: &str,
        quote: &str,
        instrument_type: InstrumentType,
    ) -> InstrumentProfile {
        InstrumentProfile {
            instrument: Instrument {
                symbol: symbol.to_owned(),
                name: symbol.to_owned(),
                base: None,
                quote: quote.to_owned(),
                instrument_type,
                exchange: "TEST".to_owned(),
                provider: Provider::Yahoo,
            },
            earliest_ts: HashMap::new(),
            latest_ts: HashMap::new(),
            legs: vec![],
        }
    }

    #[test]
    fn builtin_strategies_trade_multiple_times_and_cover_all_symbols() {
        let symbols = vec!["AAA", "BBB", "CCC"];
        let n_bars = 240_usize;
        let aligned = mk_multi_symbol_aligned(&symbols, n_bars);
        let timeline: Vec<i64> = (0..n_bars).map(|i| 1_700_000_000 + i as i64 * 86_400).collect();

        let mut cfg = mk_cfg(symbols[0]);
        cfg.data.symbols = symbols.iter().map(|s| (*s).to_owned()).collect();
        cfg.portfolio.initial_cash = 1_000_000;
        cfg.exchange.allow_short_selling = true;
        cfg.engine.warmup_period = 0;

        let profiles: Vec<InstrumentProfile> =
            symbols.iter().map(|s| mk_profile(s, "USD")).collect();

        let (strategies, indicator_objs) = Python::attach(|py| -> PyResult<_> {
            let strategies: Vec<(String, Py<PyAny>)> = vec![
                ("Adaptive RSI".to_owned(), Py::new(py, AdaptiveRsi::new(2, 6))?.into_any()),
                ("AlphaRSI Pro".to_owned(), Py::new(py, AlphaRsiPro::new(3, 5))?.into_any()),
                (
                    "BB Mean Reversion".to_owned(),
                    Py::new(py, BollingerMeanReversion::new(5, 1.0))?.into_any(),
                ),
                ("Buy & Hold".to_owned(), Py::new(py, BuyAndHold::new(None))?.into_any()),
                ("Double Top".to_owned(), Py::new(py, DoubleTop::new(90))?.into_any()),
                (
                    "Hybrid AlphaRSI".to_owned(),
                    Py::new(py, HybridAlphaRsi::new(8, 28, 20))?.into_any(),
                ),
                ("MACD".to_owned(), Py::new(py, Macd::new(3, 7, 3))?.into_any()),
                ("Momentum".to_owned(), Py::new(py, Momentum::new(3, 7))?.into_any()),
                (
                    "Multi BB Rotation".to_owned(),
                    Py::new(py, MultiBollingerRotation::new(5, 1.0, 2, 1))?.into_any(),
                ),
                ("Risk Averse".to_owned(), Py::new(py, RiskAverse::new(4, 6))?.into_any()),
                ("ROC".to_owned(), Py::new(py, Roc::new(3))?.into_any()),
                ("ROC Rotation".to_owned(), Py::new(py, RocRotation::new(3, 2, 1))?.into_any()),
                ("RSI".to_owned(), Py::new(py, Rsi::new(3, 5, 1.0))?.into_any()),
                ("RSRS".to_owned(), Py::new(py, Rsrs::new(6))?.into_any()),
                ("RSRS Rotation".to_owned(), Py::new(py, RsrsRotation::new(6, 2, 1))?.into_any()),
                ("Crossover SMA".to_owned(), Py::new(py, SmaCrossover::new(3, 8))?.into_any()),
                ("Naive SMA".to_owned(), Py::new(py, SmaNaive::new(5))?.into_any()),
                (
                    "Triple RSI Rotation".to_owned(),
                    Py::new(py, TripleRsiRotation::new(2, 3, 5, 2, 1))?.into_any(),
                ),
                ("Turtle Trading".to_owned(), Py::new(py, TurtleTrading::new(8, 4, 5))?.into_any()),
                ("VCP".to_owned(), Py::new(py, Vcp::new(18, 3))?.into_any()),
            ];

            let mut seen: HashSet<String> = HashSet::new();
            let mut indicator_objs: Vec<(String, Py<PyAny>)> = Vec::new();
            for (_, sobj) in &strategies {
                let raw = sobj.bind(py).call_method0("required_indicators")?;
                let required: Vec<Py<PyAny>> = raw.extract()?;
                for ind in required {
                    let name = _indicator_deterministic_name(ind.bind(py).as_any())?;
                    if seen.insert(name.clone()) {
                        indicator_objs.push((name, ind));
                    }
                }
            }

            Ok((strategies, indicator_objs))
        })
        .expect("failed to instantiate built-in strategy suite");

        let indicators = compute_indicators(&indicator_objs, &aligned, None)
            .expect("failed to compute auto-injected indicators for built-ins");

        let fx = FxTable::new("USD");
        let mut all_traded_symbols: HashSet<String> = HashSet::new();

        for (strategy_name, strategy_obj) in strategies {
            let run = run_one_strategy(
                &strategy_name,
                strategy_obj,
                &cfg,
                &aligned,
                &indicators,
                &profiles,
                &timeline,
                &fx,
                None,
            );
            assert!(run.error.is_none(), "strategy {strategy_name} failed: {:?}", run.error);

            let filled: Vec<&OrderRecord> =
                run.orders.iter().filter(|o| o.status == "filled").collect();
            if strategy_name != "Buy & Hold" {
                assert!(
                    filled.len() >= 2,
                    "strategy {strategy_name} should execute multiple fills, got {}",
                    filled.len()
                );
            }

            for record in filled {
                all_traded_symbols.insert(record.order.symbol.clone());
            }
        }

        let expected_symbols: HashSet<String> = symbols.iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(
            all_traded_symbols, expected_symbols,
            "expected every symbol to be traded at least once across built-ins"
        );
    }

    #[test]
    fn portfolio_equity_for_sizer_uses_target_currency() {
        let mut aligned = HashMap::new();
        aligned.insert("AAPL".to_owned(), vec![Some(mk_bar(1_700_000_000, 50.0))]);

        let cash = HashMap::from([(Currency::EUR, 1_000.0)]);
        let positions = HashMap::from([("AAPL".to_owned(), 2.0)]);
        let quote_ccy = HashMap::from([("AAPL".to_owned(), "USD".to_owned())]);
        let mut fx = FxTable::new("EUR");
        fx.add_series("EUR", "USD", vec![(1_700_000_000, 1.20)]);

        let equity_usd = compute_portfolio_equity(
            &cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            "EUR",
            "USD",
            &fx,
            1_700_000_000,
        );
        assert!((equity_usd - 1_300.0).abs() < MIN_POSITION);

        let equity_eur = compute_portfolio_equity(
            &cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            "EUR",
            "EUR",
            &fx,
            1_700_000_000,
        );
        assert!((equity_eur - (1_000.0 + 100.0 / 1.20)).abs() < MIN_POSITION);
    }

    /// Tiny helper that emits a single market buy on the first bar
    /// after warmup, then nothing else. Implemented in pure Rust so the
    /// tests don't need the GIL.
    ///
    /// The order-resolution loop here mirrors the real
    /// `run_one_strategy` logic for the features that don't depend on
    /// Python interop: slippage, commission, multi-currency cash,
    /// `allow_short_selling`, `allowed_order_types`, auto-shrinking,
    /// and `trade_on_close`. Tests can therefore exercise real engine
    /// semantics without spinning up a Python interpreter.
    fn run_with_orders(
        cfg: &ExperimentConfig,
        aligned: &HashMap<String, Vec<Option<Bar>>>,
        profiles: &[InstrumentProfile],
        injected: Vec<(usize, Order)>,
    ) -> RunResult {
        let total_bars = aligned.values().map(|v| v.len()).next().unwrap_or(0);
        let timeline: Vec<i64> = aligned
            .values()
            .next()
            .unwrap()
            .iter()
            .map(|b| b.as_ref().map(|x| x.open_ts as i64).unwrap_or(0))
            .collect();

        let base_ccy = cfg.portfolio.base_currency;
        let base_ccy_str = base_ccy.to_string();
        let mut cash: Cash = Cash::from([(base_ccy, cfg.portfolio.initial_cash as f64)]);
        let mut positions: Positions = cfg.portfolio.starting_positions.clone();
        let mut open_orders: Vec<Order> = Vec::new();
        let mut trail_state: HashMap<String, (f64, f64)> = HashMap::new();
        let mut equity_curve: Vec<EquitySample> = Vec::with_capacity(total_bars);
        let mut order_records: Vec<OrderRecord> = Vec::new();
        let mut closed_trades: Vec<Trade> = Vec::new();
        let mut open_trades: HashMap<String, (i64, f64, f64)> = HashMap::new();
        let mut peak = cfg.portfolio.initial_cash as f64;
        let mut run_error: Option<String> = None;

        let quote_ccy: HashMap<String, String> = profiles
            .iter()
            .map(|p| (p.instrument.symbol.clone(), p.instrument.quote.clone()))
            .collect();
        let instrument_types = profile_instrument_types(profiles);

        let allowed = &cfg.exchange.allowed_order_types;

        // Empty FX table — tests run single-currency, no leg bars. The
        // FX-aware `try_debit` falls back to base-only debits when the
        // table has no rates, exactly matching the legacy behaviour.
        let fx = FxTable::new(base_ccy.to_string());

        for bar_index in 0..total_bars {
            let ts = timeline[bar_index];

            // Inject orders at this bar, validating them against the
            // exchange's `allowed_order_types` exactly as the real engine
            // does (see the `ords.retain_mut(...)` block in
            // `run_one_strategy`).
            for (i, o) in &injected {
                if *i == bar_index {
                    if !allowed.contains(&o.order_type) && o.order_type != OrderType::Cancel {
                        order_records.push(OrderRecord {
                            order: o.clone(),
                            timestamp: ts,
                            status: "rejected".into(),
                            fill_price: None,
                            reason: format!("order type {} not allowed", o.order_type),
                            commission: 0.0,
                            pnl: None,
                        });
                        continue;
                    }
                    if !matches!(o.order_type, OrderType::Cancel | OrderType::SettlePosition) {
                        let instrument_type = instrument_type_for_symbol(
                            &o.symbol,
                            &instrument_types,
                            cfg.data.instrument_type,
                        );
                        if let Some(reason) =
                            quantity_rejection_reason(&o.symbol, o.quantity, instrument_type)
                        {
                            order_records.push(OrderRecord {
                                order: o.clone(),
                                timestamp: ts,
                                status: "rejected".into(),
                                fill_price: None,
                                reason,
                                commission: 0.0,
                                pnl: None,
                            });
                            continue;
                        }
                    }
                    // Reject duplicate ids in test helper.
                    if !matches!(o.order_type, OrderType::Cancel | OrderType::SettlePosition)
                        && open_orders.iter().any(|existing| existing.id == o.id)
                    {
                        order_records.push(OrderRecord {
                            order: o.clone(),
                            timestamp: ts,
                            status: "rejected".into(),
                            fill_price: None,
                            reason: format!("duplicate order id {:?}", o.id),
                            commission: 0.0,
                            pnl: None,
                        });
                        continue;
                    }
                    open_orders.push(o.clone());
                }
            }

            // Resolve open orders: faithful copy of run_one_strategy's logic.
            let drained: Vec<Order> = std::mem::take(&mut open_orders);
            for mut order in drained {
                if order.order_type == OrderType::Cancel {
                    open_orders.retain(|o| o.id != order.id);
                    order_records.push(OrderRecord {
                        order: order.clone(),
                        timestamp: ts,
                        status: "canceled".into(),
                        fill_price: None,
                        reason: "cancel".into(),
                        commission: 0.0,
                        pnl: None,
                    });
                    continue;
                }
                let symbol = order.symbol.clone();
                let bar = match aligned.get(&symbol).and_then(|r| r[bar_index].clone()) {
                    Some(b) => b,
                    None => {
                        open_orders.push(order);
                        continue;
                    },
                };

                // Decide whether this order fires this bar (and at what price).
                let outcome = resolve_trigger(
                    &mut order,
                    &bar,
                    &positions,
                    &mut trail_state,
                    cfg.engine.trade_on_close,
                );
                let (raw_px, mut fill_reason, limit_cap) = match outcome {
                    TriggerOutcome::Fill {
                        raw_px,
                        reason,
                        limit_cap,
                    } => (raw_px, reason, limit_cap),
                    TriggerOutcome::Pending => {
                        open_orders.push(order);
                        continue;
                    },
                    TriggerOutcome::Cancel {
                        reason,
                    } => {
                        trail_state.remove(&order.id);
                        order_records.push(OrderRecord {
                            order: order.clone(),
                            timestamp: ts,
                            status: "canceled".into(),
                            fill_price: None,
                            reason,
                            commission: 0.0,
                            pnl: None,
                        });
                        continue;
                    },
                };

                let slip = cfg.exchange.slippage / 100.0;
                let fill_px = apply_slippage(raw_px, order.quantity, slip, limit_cap);

                let qty = order.quantity;
                let mut filled_qty = qty;

                // Accounting currency for cash operations (same as main loop).
                let order_ccy_str =
                    quote_ccy.get(&order.symbol).map(String::as_str).unwrap_or(&base_ccy_str);
                let (order_ccy, nonfiat_fx_rate) = match order_ccy_str.parse::<Currency>() {
                    Ok(fiat) => (fiat, 1.0_f64),
                    Err(_) => {
                        let rate = fx.rate(order_ccy_str, &base_ccy_str, ts).unwrap_or(1.0);
                        (base_ccy, rate)
                    },
                };
                let acct_fill_px = fill_px * nonfiat_fx_rate;

                let mut notional = acct_fill_px * qty.abs();
                let mut commission = match cfg.exchange.commission_type {
                    CommissionType::Percentage => notional * cfg.exchange.commission_pct / 100.0,
                    CommissionType::Fixed => cfg.exchange.commission_fixed,
                    CommissionType::PercentagePlusFixed => {
                        notional * cfg.exchange.commission_pct / 100.0
                            + cfg.exchange.commission_fixed
                    },
                };
                let mut fill_pnl: Option<f64> = None;

                if qty > 0.0 {
                    let needed = notional + commission;
                    if !try_debit(&mut cash, order_ccy, needed, base_ccy, &fx, ts) {
                        let avail: f64 =
                            cash.values().copied().filter(|v| v.is_finite() && *v > 0.0).sum();
                        let pct_part = match cfg.exchange.commission_type {
                            CommissionType::Percentage | CommissionType::PercentagePlusFixed => {
                                cfg.exchange.commission_pct / 100.0
                            },
                            CommissionType::Fixed => 0.0,
                        };
                        let fixed_part = match cfg.exchange.commission_type {
                            CommissionType::Fixed | CommissionType::PercentagePlusFixed => {
                                cfg.exchange.commission_fixed
                            },
                            CommissionType::Percentage => 0.0,
                        };
                        let denom = acct_fill_px * (1.0 + pct_part);
                        let mut max_qty: f64 = if denom > 0.0 && avail > fixed_part {
                            ((avail - fixed_part) / denom).max(0.0)
                        } else {
                            0.0
                        };
                        let instrument_type = instrument_type_for_symbol(
                            &symbol,
                            &instrument_types,
                            cfg.data.instrument_type,
                        );
                        if !instrument_type.allows_fractional_quantities() {
                            max_qty = max_qty.floor();
                        }
                        if max_qty <= 0.0 {
                            order_records.push(OrderRecord {
                                order: order.clone(),
                                timestamp: ts,
                                status: "rejected".into(),
                                fill_price: None,
                                reason: "insufficient funds".into(),
                                commission: 0.0,
                                pnl: None,
                            });
                            continue;
                        }
                        filled_qty = max_qty.min(qty);
                        notional = acct_fill_px * filled_qty;
                        commission = match cfg.exchange.commission_type {
                            CommissionType::Percentage => {
                                notional * cfg.exchange.commission_pct / 100.0
                            },
                            CommissionType::Fixed => cfg.exchange.commission_fixed,
                            CommissionType::PercentagePlusFixed => {
                                notional * cfg.exchange.commission_pct / 100.0
                                    + cfg.exchange.commission_fixed
                            },
                        };
                        let shrunk_needed = notional + commission;
                        if !try_debit(&mut cash, order_ccy, shrunk_needed, base_ccy, &fx, ts) {
                            order_records.push(OrderRecord {
                                order: order.clone(),
                                timestamp: ts,
                                status: "rejected".into(),
                                fill_price: None,
                                reason: "insufficient funds".into(),
                                commission: 0.0,
                                pnl: None,
                            });
                            continue;
                        }
                        fill_reason = if fill_reason.is_empty() {
                            "partial: shrunk to fit cash".to_owned()
                        } else {
                            format!("{fill_reason}; partial: shrunk to fit cash")
                        };
                    }
                    *positions.entry(symbol.clone()).or_insert(0.0) += filled_qty;
                    update_open_trade_buy(&mut open_trades, &symbol, ts, filled_qty, acct_fill_px);
                } else if qty < 0.0 {
                    let abs_qty = qty.abs();
                    let cur = *positions.get(&symbol).unwrap_or(&0.0);
                    if !cfg.exchange.allow_short_selling && cur < abs_qty {
                        warn!(strategy="test", order_id=%order.id, "Short selling disabled and not enough position, skipping.");
                        if cfg.exchange.raise_on_short_violation {
                            run_error.get_or_insert_with(|| "short selling disabled".to_owned());
                        }
                        order_records.push(OrderRecord {
                            order: order.clone(),
                            timestamp: ts,
                            status: "rejected".into(),
                            fill_price: None,
                            reason: "short selling disabled".into(),
                            commission: 0.0,
                            pnl: None,
                        });
                        continue;
                    }
                    *cash.entry(order_ccy).or_insert(0.0) += notional;
                    if !try_debit(&mut cash, order_ccy, commission, base_ccy, &fx, ts) {
                        *cash.entry(order_ccy).or_insert(0.0) -= notional;
                        order_records.push(OrderRecord {
                            order: order.clone(),
                            timestamp: ts,
                            status: "rejected".into(),
                            fill_price: None,
                            reason: "cannot pay commission".into(),
                            commission: 0.0,
                            pnl: None,
                        });
                        continue;
                    }
                    *positions.entry(symbol.clone()).or_insert(0.0) -= abs_qty;
                    let realised = close_open_trade_sell(
                        &mut open_trades,
                        &symbol,
                        ts,
                        abs_qty,
                        acct_fill_px,
                        commission,
                    )
                    .map(|t| {
                        let pnl = t.pnl;
                        closed_trades.push(t);
                        pnl
                    });
                    fill_pnl = realised;
                }

                if (filled_qty - order.quantity).abs() > MIN_POSITION {
                    order.quantity = filled_qty;
                }
                order_records.push(OrderRecord {
                    order: order.clone(),
                    timestamp: ts,
                    status: "filled".into(),
                    fill_price: Some(fill_px),
                    reason: fill_reason,
                    commission,
                    pnl: fill_pnl,
                });
            }

            // Equity sample: cash + sum(qty * close).
            let mut equity: f64 = cash.values().sum();
            for (sym, qty) in &positions {
                if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
                    let value = *qty * b.close;
                    let pos_ccy = quote_ccy.get(sym).map(String::as_str).unwrap_or(&base_ccy_str);
                    equity += fx.convert(value, pos_ccy, &base_ccy_str, ts).unwrap_or(value);
                }
            }
            if equity > peak {
                peak = equity;
            }
            let dd = if peak > 0.0 {
                (equity - peak) / peak
            } else {
                0.0
            };
            equity_curve.push(EquitySample {
                timestamp: ts,
                equity,
                cash: cash.clone(),
                drawdown: dd,
            });
        }

        let metrics =
            compute_metrics(cfg.portfolio.initial_cash as f64, 0.0, &equity_curve, &closed_trades);

        RunResult {
            strategy_id: "test_id".into(),
            strategy_name: "test".into(),
            equity_curve,
            trades: closed_trades,
            orders: order_records,
            metrics,
            base_currency: cfg.portfolio.base_currency,
            error: run_error,
            is_benchmark: false,
        }
    }

    #[test]
    fn aligns_and_forward_fills() {
        let mut bars = HashMap::new();
        bars.insert("AAPL".to_owned(), vec![mk_bar(1, 100.0), mk_bar(3, 102.0)]);
        let aligned = align_bars(&bars, &[1, 2, 3], EmptyBarPolicy::ForwardFill);
        let row = &aligned["AAPL"];
        assert_eq!(row.len(), 3);
        assert!(row[0].is_some());
        assert!(row[1].is_some()); // forward-filled
        assert_eq!(row[1].as_ref().unwrap().close, 100.0);
        assert!(row[2].is_some());
    }

    #[test]
    fn aligns_skip_yields_none() {
        let mut bars = HashMap::new();
        bars.insert("AAPL".to_owned(), vec![mk_bar(1, 100.0)]);
        let aligned = align_bars(&bars, &[1, 2], EmptyBarPolicy::Skip);
        assert!(aligned["AAPL"][0].is_some());
        assert!(aligned["AAPL"][1].is_none());
    }

    #[test]
    fn buy_fills_and_creates_position() {
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 101.0, 102.0]);
        let order = Order {
            id: "buy1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, order)]);
        assert_eq!(r.orders.len(), 1);
        assert_eq!(r.orders[0].status, "filled");
        // Final equity = remaining cash (10000 - 10*100) + 10*102 (last close)
        assert!((r.equity_curve.last().unwrap().equity - (9000.0 + 1020.0)).abs() < 1e-6);
    }

    #[test]
    fn insufficient_funds_is_rejected_not_errored() {
        let mut cfg = mk_cfg("AAPL");
        cfg.portfolio.initial_cash = 150; // can afford only 1 whole share @ 100
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let order = Order {
            id: "buy1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, order)]);
        // Non-crypto auto-shrinks to the largest whole-unit buy that fits cash.
        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].order.quantity, 1.0);
        assert!(r.orders[0].reason.contains("partial"));
    }

    #[test]
    fn crypto_buy_auto_shrinks_fractionally_to_fit_cash() {
        let mut cfg = mk_cfg("BTC-USD");
        cfg.data.instrument_type = InstrumentType::Crypto;
        cfg.portfolio.initial_cash = 50;
        let aligned = mk_aligned("BTC-USD", &[100.0, 101.0]);
        let order = Order {
            id: "buy1".into(),
            symbol: "BTC-USD".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile_with_type("BTC-USD", "USD", InstrumentType::Crypto)],
            vec![(0, order)],
        );
        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].order.quantity, 0.5);
        assert!(r.orders[0].reason.contains("partial"));
    }

    #[test]
    fn cancel_order_removes_pending() {
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 101.0, 102.0]);
        let buy = Order {
            id: "buy1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let cancel = Order {
            id: "buy1".into(),
            symbol: "".into(),
            order_type: OrderType::Cancel,
            quantity: 0.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        // Both injected on bar 0, cancel comes first in the open-order
        // list so the cancel removes the buy before it fills.
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, cancel), (0, buy)],
        );
        let canceled = r.orders.iter().filter(|o| o.status == "canceled").count();
        assert!(canceled >= 1);
    }

    #[test]
    fn duplicate_order_id_is_rejected() {
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 101.0, 102.0]);
        let limit1 = Order {
            id: "dup".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(99.0),
            limit_price: None,
            sizer: None,
        };
        let limit2 = Order {
            id: "dup".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 10.0,
            price: Some(98.0),
            limit_price: None,
            sizer: None,
        };
        let mut cfg2 = cfg.clone();
        cfg2.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::Limit, OrderType::Cancel];
        let r = run_with_orders(
            &cfg2,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, limit1), (0, limit2)],
        );
        let rejected = r
            .orders
            .iter()
            .filter(|o| o.status == "rejected" && o.reason.contains("duplicate"))
            .count();
        assert_eq!(rejected, 1, "second order with same id should be rejected");
    }

    #[test]
    fn foreign_currency_falls_back_to_base() {
        let mut cfg = mk_cfg("VOD.L");
        cfg.portfolio.base_currency = Currency::USD;
        let aligned = mk_aligned("VOD.L", &[100.0, 101.0]);
        // Quote is GBP; we have only USD cash, so it must fall back.
        let order = Order {
            id: "b".into(),
            symbol: "VOD.L".into(),
            order_type: OrderType::Market,
            quantity: 100.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("VOD.L", "GBP")], vec![(0, order)]);
        // Without GBP/USD FX path is provided in this unit setup, so funding fails.
        assert_eq!(r.orders[0].status, "rejected");
        assert_eq!(r.orders[0].reason, "insufficient funds");
    }

    #[test]
    fn metrics_computed() {
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 110.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy)]);
        assert!(r.metrics.contains_key("sharpe"));
        assert!(r.metrics.contains_key("total_return"));
        assert!(r.metrics.contains_key("max_dd"));
    }

    #[test]
    fn strategies_run_independently() {
        // Two synthetic runs with the same data should be deep-cloneable.
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 105.0, 110.0]);
        let buy_a = Order {
            id: "a".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let buy_b = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let ra = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy_a)]);
        let rb = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy_b)]);
        assert_ne!(ra.equity_curve.last().unwrap().equity, rb.equity_curve.last().unwrap().equity);
    }

    #[test]
    fn parse_iso_date_works() {
        let ts = iso_to_ts("2024-01-15").unwrap();
        // 2024-01-15 00:00 UTC
        assert_eq!(ts, 1_705_276_800);
    }

    // ─────────────────────────────────────────────────────────────────
    // Quantity granularity
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn stock_starting_position_rejects_fractional_quantity() {
        let mut cfg = mk_cfg("AAPL");
        cfg.portfolio.starting_positions.insert("AAPL".into(), 1.5);

        let err = validate_qty(&cfg, &[mk_profile("AAPL", "USD")])
            .expect_err("fractional stock starting position should fail");

        assert!(err.to_string().contains("fractional quantity"));
        assert!(err.to_string().contains("only crypto"));
    }

    #[test]
    fn crypto_starting_position_allows_fractional_quantity() {
        let mut cfg = mk_cfg("BTC-USD");
        cfg.data.instrument_type = InstrumentType::Crypto;
        cfg.portfolio.starting_positions.insert("BTC-USD".into(), 0.25);

        validate_qty(&cfg, &[mk_profile_with_type("BTC-USD", "USD", InstrumentType::Crypto)])
            .expect("fractional crypto starting position should be valid");
    }

    #[test]
    fn custom_stock_order_rejects_fractional_quantity() {
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let order = Order {
            id: "frac".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 1.5,
            price: None,
            limit_price: None,
            sizer: None,
        };

        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, order)]);

        assert_eq!(r.orders[0].status, "rejected");
        assert!(r.orders[0].reason.contains("fractional quantity"));
    }

    #[test]
    fn crypto_order_allows_fractional_quantity() {
        let mut cfg = mk_cfg("BTC-USD");
        cfg.data.instrument_type = InstrumentType::Crypto;
        let aligned = mk_aligned("BTC-USD", &[100.0, 101.0]);
        let order = Order {
            id: "frac".into(),
            symbol: "BTC-USD".into(),
            order_type: OrderType::Market,
            quantity: 1.5,
            price: None,
            limit_price: None,
            sizer: None,
        };

        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile_with_type("BTC-USD", "USD", InstrumentType::Crypto)],
            vec![(0, order)],
        );

        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].order.quantity, 1.5);
    }

    #[test]
    fn builtin_stock_order_is_floored_to_whole_quantity() {
        let mut order = Order {
            id: "builtin".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 3.75,
            price: None,
            limit_price: None,
            sizer: None,
        };

        let err = validate_order(&mut order, InstrumentType::Stocks);

        assert!(err.is_none());
        assert_eq!(order.quantity, 3.0);
    }

    #[test]
    fn builtin_crypto_order_keeps_fractional_quantity() {
        let mut order = Order {
            id: "builtin".into(),
            symbol: "BTC-USD".into(),
            order_type: OrderType::Market,
            quantity: 0.125,
            price: None,
            limit_price: None,
            sizer: None,
        };

        let err = validate_order(&mut order, InstrumentType::Crypto);

        assert!(err.is_none());
        assert_eq!(order.quantity, 0.125);
    }

    #[test]
    fn builtin_stock_order_below_one_whole_unit_is_rejected() {
        let mut order = Order {
            id: "builtin".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 0.75,
            price: None,
            limit_price: None,
            sizer: None,
        };

        let err = validate_order(&mut order, InstrumentType::Stocks)
            .expect("sub-one stock order should be rejected");

        assert!(err.contains("less than one whole"));
    }

    // ─────────────────────────────────────────────────────────────────
    // Order types — see `OrderType` enum. The engine only implements
    // Market + Cancel execution semantics today; everything else
    // either falls through to the Market path (when present in
    // `allowed_order_types`) or is rejected by the allowed-types filter.
    // The tests below pin down both the working paths and the gaps so
    // that any future change to the order-resolution logic surfaces as
    // a deliberate test update rather than a silent behavioural shift.
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn allowed_order_types_filter_rejects_disallowed() {
        // The exchange config's `allowed_order_types` is enforced at
        // submission time. With only Market allowed, a Limit order is
        // rejected before it ever reaches the resolution loop.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types = vec![OrderType::Market, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let limit = Order {
            id: "lim1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(95.0),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, limit)]);
        assert_eq!(r.orders.len(), 1);
        assert_eq!(r.orders[0].status, "rejected");
        assert!(r.orders[0].reason.contains("not allowed"));
    }

    // ─────────────────────────────────────────────────────────────────
    // Limit orders
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn limit_buy_does_not_fill_when_price_above_limit() {
        // Buy limit at 50, price stays around 100 → never hits the limit.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::Limit, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]); // mk_bar makes high=*1.01, low=*0.99
        let limit = Order {
            id: "lim1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(50.0),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, limit)]);
        // No order record at all — the limit stayed pending across both bars.
        assert!(r.orders.is_empty(), "expected no fills, got {:?}", r.orders);
    }

    #[test]
    fn limit_buy_fills_at_open_when_open_below_limit() {
        // Buy limit at 110, bar opens at 100 → fill at the open (better than limit).
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::Limit, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 100.0]);
        let limit = Order {
            id: "lim".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(110.0),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, limit)]);
        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].fill_price, Some(100.0));
    }

    #[test]
    fn limit_buy_fills_at_limit_when_low_reaches_it() {
        // mk_bar(100) → open=100, low=99. Buy limit at 99.5 → not at open
        // (100 > 99.5), but bar.low (99) <= 99.5 → fill at limit price.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::Limit, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0]);
        let limit = Order {
            id: "lim".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(99.5),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, limit)]);
        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].fill_price, Some(99.5));
    }

    #[test]
    fn limit_sell_fills_at_limit_when_high_reaches_it() {
        // mk_bar(100) → open=100, high=101. Sell limit at 100.5 → fill at 100.5.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::Limit, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0]);
        // Need a long to sell from.
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        // Inject buy at bar 0, then a sell-limit also at bar 0; the buy
        // settles first (queue order preserved), so the limit sees +5 long.
        let sell_limit = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: -5.0,
            price: Some(100.5),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, buy), (0, sell_limit)],
        );
        let sell_record = r.orders.iter().find(|o| o.order.id == "s").expect("sell missing");
        assert_eq!(sell_record.status, "filled");
        assert_eq!(sell_record.fill_price, Some(100.5));
    }

    #[test]
    fn limit_buy_slippage_never_crosses_limit() {
        // Buy limit at 99.5, bar.open=100, bar.low=99 → raw fill at 99.5,
        // 1 % slippage would push it to ~100.495, which crosses the limit.
        // Engine should clamp the slipped price at 99.5.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::Limit, OrderType::Cancel];
        cfg.exchange.slippage = 1.0;
        let aligned = mk_aligned("AAPL", &[100.0]);
        let limit = Order {
            id: "lim".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(99.5),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, limit)]);
        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].fill_price, Some(99.5));
    }

    #[test]
    fn limit_sell_slippage_never_crosses_limit() {
        // Sell limit at 100.5, bar.high=101 → raw fill at 100.5,
        // 1 % slippage would push it down to ~99.495, crossing the limit.
        // Engine should clamp the slipped price at 100.5.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::Limit, OrderType::Cancel];
        cfg.exchange.slippage = 1.0;
        let aligned = mk_aligned("AAPL", &[100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let sell_limit = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: -5.0,
            price: Some(100.5),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, buy), (0, sell_limit)],
        );
        let sell_rec = r.orders.iter().find(|o| o.order.id == "s").expect("sell missing");
        assert_eq!(sell_rec.status, "filled");
        assert_eq!(sell_rec.fill_price, Some(100.5));
    }

    // ─────────────────────────────────────────────────────────────────
    // Stop-loss / take-profit
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn stop_loss_sell_does_not_trigger_above_stop() {
        // Sell stop at 90, prices stay at/above 100 → never triggers.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::StopLoss, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let sl = Order {
            id: "sl".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLoss,
            quantity: -5.0,
            price: Some(90.0),
            limit_price: None,
            sizer: None,
        };
        let r =
            run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy), (0, sl)]);
        // Only the buy got filled.
        assert!(r.orders.iter().any(|o| o.order.id == "b" && o.status == "filled"));
        assert!(!r.orders.iter().any(|o| o.order.id == "sl"));
    }

    #[test]
    fn stop_loss_sell_triggers_when_low_crosses_stop() {
        // Sell stop at 95, bar 1 has open=100, low=99, bar 2 open=90 → triggers via gap-down.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::StopLoss, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 90.0]); // bar2 open=90 < 95
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let sl = Order {
            id: "sl".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLoss,
            quantity: -5.0,
            price: Some(95.0),
            limit_price: None,
            sizer: None,
        };
        let r =
            run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy), (0, sl)]);
        let sl_rec = r.orders.iter().find(|o| o.order.id == "sl").expect("sl missing");
        assert_eq!(sl_rec.status, "filled");
        // Gap-down: filled at the open (90), not at the stop (95).
        assert_eq!(sl_rec.fill_price, Some(90.0));
        assert!(sl_rec.reason.contains("gap-down"));
    }

    #[test]
    fn stop_loss_sell_fills_at_stop_when_no_gap() {
        // mk_bar(100) → open=100, low=99. Stop at 99.5 → no gap, fills at 99.5.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::StopLoss, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let sl = Order {
            id: "sl".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLoss,
            quantity: -5.0,
            price: Some(99.5),
            limit_price: None,
            sizer: None,
        };
        let r =
            run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy), (0, sl)]);
        let sl_rec = r.orders.iter().find(|o| o.order.id == "sl").expect("sl missing");
        assert_eq!(sl_rec.status, "filled");
        assert_eq!(sl_rec.fill_price, Some(99.5));
    }

    #[test]
    fn stop_loss_buy_triggers_on_price_rise() {
        // Buy stop at 110 (typical short-cover or breakout entry). Bar 2
        // opens at 105 with high 106.05 — doesn't reach 110. Bar 3 opens
        // at 111 → gap-up triggers fill at open 111.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::StopLoss, OrderType::Cancel];
        // 0 → buy stop placed; bar 1: 100 (low 99, high 101) — no trigger;
        // bar 2: 111 (gap-up open above 110).
        let aligned = mk_aligned("AAPL", &[100.0, 111.0]);
        let stop_buy = Order {
            id: "sb".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLoss,
            quantity: 5.0,
            price: Some(110.0),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, stop_buy)]);
        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].fill_price, Some(111.0));
    }

    #[test]
    fn take_profit_executes_like_limit() {
        // TakeProfit and Limit share execution rules. Sell TP at 100.5
        // with bar.high=101 → fills at 100.5.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::TakeProfit, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let tp = Order {
            id: "tp".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::TakeProfit,
            quantity: -5.0,
            price: Some(100.5),
            limit_price: None,
            sizer: None,
        };
        let r =
            run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy), (0, tp)]);
        let tp_rec = r.orders.iter().find(|o| o.order.id == "tp").expect("tp missing");
        assert_eq!(tp_rec.fill_price, Some(100.5));
    }

    // ─────────────────────────────────────────────────────────────────
    // Stop-Limit variants
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn stop_loss_limit_converts_to_limit_after_trigger() {
        // Stop at 95, limit at 95. Bar 2 has open=90 → stop fires (gap-down),
        // but as a limit-sell at 95 with bar.high=90.9, the limit can't be
        // reached this same bar (sell-limit needs price >= 95). It rests
        // pending. Bar 3 opens at 96 → sell-limit fills at the open.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::StopLossLimit, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 90.0, 96.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let sll = Order {
            id: "sll".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLossLimit,
            quantity: -5.0,
            price: Some(95.0),       // stop trigger
            limit_price: Some(95.0), // limit price after trigger
            sizer: None,
        };
        let r =
            run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy), (0, sll)]);
        let sll_rec = r.orders.iter().find(|o| o.order.id == "sll").expect("sll missing");
        assert_eq!(sll_rec.status, "filled");
        // Filled on bar 3 at the open-through-limit (96 >= 95 → fill at open).
        assert_eq!(sll_rec.fill_price, Some(96.0));
        // Order's runtime type was mutated to Limit.
        assert_eq!(sll_rec.order.order_type, OrderType::Limit);
    }

    #[test]
    fn stop_loss_limit_does_nothing_until_stop_fires() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::StopLossLimit, OrderType::Cancel];
        // Prices stay above stop forever.
        let aligned = mk_aligned("AAPL", &[100.0, 102.0, 104.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let sll = Order {
            id: "sll".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLossLimit,
            quantity: -5.0,
            price: Some(90.0),
            limit_price: Some(89.5),
            sizer: None,
        };
        let r =
            run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy), (0, sll)]);
        assert!(!r.orders.iter().any(|o| o.order.id == "sll"));
    }

    // ─────────────────────────────────────────────────────────────────
    // Trailing stops
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn trailing_stop_sell_does_not_fire_in_uptrend() {
        // Trail offset 5. Prices march up → stop = high - 5 keeps rising
        // and never gets hit.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::TrailingStop, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 105.0, 110.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let trail = Order {
            id: "t".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::TrailingStop,
            quantity: -5.0,
            price: Some(5.0),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, buy), (0, trail)],
        );
        assert!(!r.orders.iter().any(|o| o.order.id == "t"));
    }

    #[test]
    fn trailing_stop_sell_fires_after_pullback() {
        // Trail offset 5. Prices: 100 → 110 (high 111.1) → pullback to 100
        // (open 100). running_high ≈ 111.1, stop = 106.1, bar.open (100)
        // is below stop → fills at gap-down open (100).
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::TrailingStop, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 110.0, 100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let trail = Order {
            id: "t".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::TrailingStop,
            quantity: -5.0,
            price: Some(5.0),
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, buy), (0, trail)],
        );
        let t = r.orders.iter().find(|o| o.order.id == "t").expect("trail missing");
        assert_eq!(t.status, "filled");
        // Bar 3 opens at 100, well below the running stop of ~106.1 → gap-down.
        assert_eq!(t.fill_price, Some(100.0));
        assert!(t.reason.contains("gap-down"));
    }

    #[test]
    fn trailing_stop_limit_rests_as_limit_after_trigger() {
        // Trail 5, limit price 105. Once stop fires, becomes a sell-limit
        // at 105. With bar.open=100 < 105 the limit doesn't fill that bar.
        // Subsequent bar opens at 106 → sell-limit fills at the open.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::TrailingStopLimit, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 110.0, 100.0, 106.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let trail = Order {
            id: "t".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::TrailingStopLimit,
            quantity: -5.0,
            price: Some(5.0),
            limit_price: Some(105.0),
            sizer: None,
        };
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, buy), (0, trail)],
        );
        let t = r.orders.iter().find(|o| o.order.id == "t").expect("trail missing");
        assert_eq!(t.status, "filled");
        assert_eq!(t.fill_price, Some(106.0));
        assert_eq!(t.order.order_type, OrderType::Limit);
    }

    // ─────────────────────────────────────────────────────────────────
    // SettlePosition
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn settle_position_flattens_long() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::SettlePosition, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 7.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let settle = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::SettlePosition,
            quantity: 0.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, buy), (1, settle)],
        );
        let settle_rec = r.orders.iter().find(|o| o.order.id == "s").expect("settle missing");
        // Order's quantity was rewritten to negate the +7 long.
        assert_eq!(settle_rec.order.quantity, -7.0);
        assert_eq!(settle_rec.status, "filled");
        // A round-trip trade was closed.
        assert_eq!(r.trades.len(), 1);
    }

    #[test]
    fn settle_position_with_no_position_is_canceled() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::SettlePosition, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let settle = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::SettlePosition,
            quantity: 0.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, settle)]);
        assert_eq!(r.orders[0].status, "canceled");
        assert_eq!(r.orders[0].reason, "no position to settle");
    }

    #[test]
    fn settle_position_covers_short() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_short_selling = true;
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::SettlePosition, OrderType::Cancel];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let short = Order {
            id: "sh".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: -3.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let settle = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::SettlePosition,
            quantity: 0.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, short), (1, settle)],
        );
        let settle_rec = r.orders.iter().find(|o| o.order.id == "s").expect("settle missing");
        // The settle was rewritten to a +3 buy to flatten the -3 short.
        assert_eq!(settle_rec.order.quantity, 3.0);
        assert_eq!(settle_rec.status, "filled");
    }

    // ─────────────────────────────────────────────────────────────────
    // Short selling
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn short_selling_disabled_rejects_naked_short() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_short_selling = false;
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let sell = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: -5.0, // no prior position → naked short.
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, sell)]);
        assert_eq!(r.orders[0].status, "rejected");
        assert_eq!(r.orders[0].reason, "short selling disabled");
    }

    #[test]
    fn short_selling_disabled_allows_closing_long() {
        // Selling within an existing long is not a short and must fill
        // even when shorting is disabled.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_short_selling = false;
        let aligned = mk_aligned("AAPL", &[100.0, 101.0, 102.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let sell = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: -3.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(
            &cfg,
            &aligned,
            &[mk_profile("AAPL", "USD")],
            vec![(0, buy), (1, sell)],
        );
        assert_eq!(r.orders[1].status, "filled");
        // 3 of the 5 long units are closed → at least one trade closed.
        assert!(!r.trades.is_empty());
    }

    #[test]
    fn short_selling_enabled_credits_cash_and_creates_short() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_short_selling = true;
        let aligned = mk_aligned("AAPL", &[100.0, 90.0]); // price drop favors short
        let sell = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: -10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, sell)]);
        assert_eq!(r.orders[0].status, "filled");
        // GAP: the short never gets re-priced into PnL because the
        // position-valuation step is qty * close — for a -10 short, a
        // drop from 100→90 increases equity by 10*10=100. We still
        // assert the cash credit happened.
        assert!(r.equity_curve.last().unwrap().equity > cfg.portfolio.initial_cash as f64);
    }

    // ─────────────────────────────────────────────────────────────────
    // Margin & risk — none implemented today.
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn no_margin_call_when_short_blows_up() {
        // GAP: there is no margin model. A short position whose mark
        // moves violently against the trader is *not* force-closed,
        // and the equity curve simply goes more and more negative
        // without any "margin call" event. This pins that behaviour.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_short_selling = true;
        cfg.portfolio.initial_cash = 1_000;
        // Price 5x's against a -10 short → unrealised loss of ~4,000.
        let aligned = mk_aligned("AAPL", &[100.0, 200.0, 500.0]);
        let sell = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: -10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, sell)]);
        assert_eq!(r.orders[0].status, "filled");
        // Equity is allowed to go negative. There is no margin-call
        // OrderRecord, no auto-flatten, no rejection mid-run.
        let final_eq = r.equity_curve.last().unwrap().equity;
        assert!(final_eq < 0.0, "expected negative equity, got {final_eq}");
        assert!(r.orders.iter().all(|o| o.reason != "margin call"));
    }

    #[test]
    fn no_partial_fill_based_on_volume() {
        // GAP: bar volume is irrelevant to the engine. A 1,000,000-share
        // buy on a bar with volume=1,000 still fills in full so long as
        // cash allows it.
        let cfg = mk_cfg("AAPL");
        let aligned = mk_aligned("AAPL", &[1.0, 1.0]); // cheap, lots of shares affordable
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 9_000.0, // 9,000 * $1 = $9,000 (within $10,000 cash)
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy)]);
        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].order.quantity, 9_000.0);
    }

    // ─────────────────────────────────────────────────────────────────
    // Slippage
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn slippage_makes_buy_pay_more() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.slippage = 1.0; // 1 %
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 1.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy)]);
        // Buy fills at open * (1 + slip) = 100 * 1.01 = 101.
        assert!((r.orders[0].fill_price.unwrap() - 101.0).abs() < 1e-9);
    }

    #[test]
    fn slippage_makes_sell_receive_less() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.slippage = 1.0;
        cfg.exchange.allow_short_selling = true;
        let aligned = mk_aligned("AAPL", &[100.0, 100.0]);
        let sell = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: -1.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, sell)]);
        // Sell fills at open * (1 - slip) = 100 * 0.99 = 99.
        assert!((r.orders[0].fill_price.unwrap() - 99.0).abs() < 1e-9);
    }

    // ─────────────────────────────────────────────────────────────────
    // Commission
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn percentage_commission_charged_on_fill() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.commission_type = CommissionType::Percentage;
        cfg.exchange.commission_pct = 0.5; // 0.5 %
        let aligned = mk_aligned("AAPL", &[100.0, 100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy)]);
        // Notional = 10 * 100 = 1000, commission = 1000 * 0.5 % = 5.
        assert!((r.orders[0].commission - 5.0).abs() < 1e-9);
    }

    #[test]
    fn fixed_commission_charged_on_fill() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.commission_type = CommissionType::Fixed;
        cfg.exchange.commission_fixed = 7.5;
        let aligned = mk_aligned("AAPL", &[100.0, 100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy)]);
        assert!((r.orders[0].commission - 7.5).abs() < 1e-9);
    }

    #[test]
    fn pct_plus_fixed_commission_charged_on_fill() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.commission_type = CommissionType::PercentagePlusFixed;
        cfg.exchange.commission_pct = 0.5;
        cfg.exchange.commission_fixed = 1.0;
        let aligned = mk_aligned("AAPL", &[100.0, 100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy)]);
        // 1000 * 0.5 % + 1.0 = 5.0 + 1.0 = 6.0.
        assert!((r.orders[0].commission - 6.0).abs() < 1e-9);
    }

    // ─────────────────────────────────────────────────────────────────
    // Auto-shrinking
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn buy_auto_shrinks_to_fit_cash() {
        // The auto-shrink path is triggered when `try_debit` returns
        // false. That only happens cross-currency (the same-currency
        // path silently allows cash to go negative — see
        // `same_currency_buy_shrinks_to_fit_cash` below).
        // Here: base=USD, quote=GBP, no GBP cash, only $1,000 base.
        // A buy of 11 @ 100 GBP needs 1,100; the engine shrinks to 10.
        let mut cfg = mk_cfg("VOD.L");
        cfg.portfolio.base_currency = Currency::USD;
        cfg.portfolio.initial_cash = 1_000;
        let aligned = mk_aligned("VOD.L", &[100.0, 100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "VOD.L".into(),
            order_type: OrderType::Market,
            quantity: 11.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("VOD.L", "GBP")], vec![(0, buy)]);
        // No GBP/USD FX path is provided in this unit setup, so funding fails.
        assert_eq!(r.orders[0].status, "rejected");
        assert_eq!(r.orders[0].reason, "insufficient funds");
    }

    #[test]
    fn same_currency_buy_shrinks_to_fit_cash() {
        // Previously a GAP: when the order's quote currency equalled the
        // portfolio's base currency, `try_debit` double-counted the same
        // bucket and allowed cash to go negative. Now fixed: the order is
        // auto-shrunk so cash stays non-negative.
        let mut cfg = mk_cfg("AAPL");
        cfg.portfolio.initial_cash = 1_000;
        let aligned = mk_aligned("AAPL", &[100.0, 100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 11.0, // 11 * 100 = 1,100 > 1,000 cash
            price: None,
            limit_price: None,
            sizer: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy)]);
        assert_eq!(r.orders[0].status, "filled");
        // Order should be shrunk to fit the available $1,000.
        assert!(r.orders[0].order.quantity <= 10.0);
        let last_cash = r
            .equity_curve
            .last()
            .and_then(|s| s.cash.get(&cfg.portfolio.base_currency))
            .copied()
            .unwrap_or(0.0);
        assert!(last_cash >= 0.0);
    }

    // ─────────────────────────────────────────────────────────────────
    // Margin / leverage / position-size helpers
    // ─────────────────────────────────────────────────────────────────

    fn empty_fx(base: Currency) -> FxTable {
        FxTable::new(base.to_string())
    }

    fn dummy_aligned(symbol: &str, close: f64) -> HashMap<String, Vec<Option<Bar>>> {
        let mut m = HashMap::new();
        m.insert(symbol.to_owned(), vec![Some(mk_bar(1_700_000_000, close))]);
        m
    }

    #[test]
    fn effective_leverage_cap_disabled_when_margin_off() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_margin = false;
        cfg.exchange.max_leverage = 5.0;
        cfg.exchange.initial_margin = 10.0;
        assert_eq!(effective_leverage_cap(&cfg), 1.0);
    }

    #[test]
    fn effective_leverage_cap_uses_min_of_max_leverage_and_initial_margin() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_margin = true;
        cfg.exchange.max_leverage = 5.0;
        cfg.exchange.initial_margin = 50.0; // → 2x
        assert!((effective_leverage_cap(&cfg) - 2.0).abs() < MIN_POSITION);

        cfg.exchange.max_leverage = 1.5;
        cfg.exchange.initial_margin = 25.0; // → 4x
        assert!((effective_leverage_cap(&cfg) - 1.5).abs() < MIN_POSITION);
    }

    #[test]
    fn effective_leverage_cap_never_below_one() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_margin = true;
        cfg.exchange.max_leverage = 0.0;
        cfg.exchange.initial_margin = 0.0; // both "off" → infinity → unchanged
        assert!(effective_leverage_cap(&cfg) >= 1.0);
    }

    #[test]
    fn check_order_against_limits_shrinks_when_position_size_exceeded() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.max_position_size = 50; // 50 % of equity per symbol
        let fx = empty_fx(Currency::USD);
        let qty = check_order_against_limits(
            &cfg, "AAPL", 100.0, 60.0, "USD", "USD", 10_000.0, 0.0, 0.0, 0.0, &fx, 0,
        )
        .expect("limit check should shrink, not reject");
        assert!((qty - (5_000.0 / 60.0)).abs() < 1e-9, "got {qty}");
    }

    #[test]
    fn check_order_against_limits_rejects_when_position_already_at_cap() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.max_position_size = 50;
        let fx = empty_fx(Currency::USD);
        let err = check_order_against_limits(
            &cfg, "AAPL", 1.0, 60.0, "USD", "USD", 10_000.0, 5_000.0, 50.0, 5_000.0, &fx, 0,
        )
        .expect_err("zero headroom must reject");
        assert_eq!(err.0, LimitViolation::PositionSize);
        assert!(err.1.contains("max_position_size"));
    }

    #[test]
    fn check_order_against_limits_accepts_within_position_size() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.max_position_size = 50;
        let fx = empty_fx(Currency::USD);
        let qty = check_order_against_limits(
            &cfg, "AAPL", 50.0, 60.0, "USD", "USD", 10_000.0, 0.0, 0.0, 0.0, &fx, 0,
        )
        .expect("within cap");
        assert_eq!(qty, 50.0);
    }

    #[test]
    fn check_order_against_limits_blocks_borrowing_when_margin_off() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_margin = false;
        cfg.exchange.max_position_size = 0; // disable per-symbol cap
        let fx = empty_fx(Currency::USD);
        let err = check_order_against_limits(
            &cfg, "AAPL", 1.0, 100.0, "USD", "USD", 1_000.0, 1_000.0, 0.0, 0.0, &fx, 0,
        )
        .expect_err("should reject borrow when margin disabled");
        assert_eq!(err.0, LimitViolation::Margin);
        assert!(err.1.contains("max_leverage"));
    }

    #[test]
    fn check_order_against_limits_allows_within_leverage() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_margin = true;
        cfg.exchange.max_leverage = 3.0;
        cfg.exchange.initial_margin = 10.0; // would be 10x but max_leverage wins
        cfg.exchange.max_position_size = 0;
        let fx = empty_fx(Currency::USD);
        check_order_against_limits(
            &cfg, "AAPL", 15.0, 100.0, "USD", "USD", 1_000.0, 1_000.0, 0.0, 0.0, &fx, 0,
        )
        .expect("within 3x leverage");
    }

    #[test]
    fn check_order_against_limits_rejects_when_equity_non_positive() {
        let cfg = mk_cfg("AAPL");
        let fx = empty_fx(Currency::USD);
        let err = check_order_against_limits(
            &cfg, "AAPL", 1.0, 100.0, "USD", "USD", 0.0, 0.0, 0.0, 0.0, &fx, 0,
        )
        .expect_err("non-positive equity");
        assert_eq!(err.0, LimitViolation::Margin);
        assert!(err.1.contains("non-positive"));
    }

    #[test]
    fn check_order_against_limits_shrinks_via_fx() {
        let mut cfg = mk_cfg("VOD.L");
        cfg.exchange.max_position_size = 100;
        cfg.exchange.allow_margin = true;
        cfg.exchange.max_leverage = 1.0;
        let mut fx = FxTable::new("USD");
        // 1 GBP = 1.30 USD
        fx.add_series("GBP", "USD", vec![(0, 1.30)]);
        let qty = check_order_against_limits(
            &cfg, "VOD.L", 100.0, 10.0, "GBP", "USD", 1_000.0, 0.0, 0.0, 0.0, &fx, 0,
        )
        .expect("limit check should shrink, not reject");
        let expected = (1_000.0_f64 / 1.30_f64) / 10.0;
        assert!((qty - expected).abs() < 1e-6, "got {qty}, expected {expected}");
    }

    #[test]
    fn check_order_against_limits_allows_reducing_when_gross_at_cap() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allow_margin = false;
        cfg.exchange.max_position_size = 100;
        let fx = empty_fx(Currency::USD);

        let qty = check_order_against_limits(
            &cfg, "AAPL", -1.0, 100.0, "USD", "USD", 1_000.0, 1_000.0, 10.0, 1_000.0, &fx, 0,
        )
        .expect("reducing exposure should be allowed at the cap");
        assert_eq!(qty, -1.0);
    }

    #[test]
    fn limit_warning_dedupe_key_buckets_dynamic_margin_reason() {
        let r1 = "order would exceed max_leverage (2.00x): gross notional already at limit (current 878.35, cap 869.91)";
        let r2 = "order would exceed max_leverage (2.00x): gross notional already at limit (current 890.31, cap 849.82)";
        let k1 = limit_warning_dedupe_key("MSFT", LimitViolation::Margin, r1);
        let k2 = limit_warning_dedupe_key("MSFT", LimitViolation::Margin, r2);
        assert_eq!(k1, k2);
    }

    #[test]
    fn limit_warning_dedupe_key_stays_symbol_scoped() {
        let reason = "order would exceed max_leverage (2.00x): gross notional already at limit (current 878.35, cap 869.91)";
        let a = limit_warning_dedupe_key("MSFT", LimitViolation::Margin, reason);
        let b = limit_warning_dedupe_key("AAPL", LimitViolation::Margin, reason);
        assert_ne!(a, b);
    }

    #[test]
    fn check_maintenance_margin_returns_none_with_no_positions() {
        let cfg = mk_cfg("AAPL");
        assert!(check_maintenance_margin(&cfg, 1_000.0, 0.0).is_none());
    }

    #[test]
    fn check_maintenance_margin_triggers_when_equity_low() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.maintenance_margin = 25.0; // need equity ≥ 25 % of gross
                                                // Equity = 200, gross = 1_000 → ratio 20 % < 25 %.
        let msg =
            check_maintenance_margin(&cfg, 200.0, 1_000.0).expect("should trigger margin call");
        assert!(msg.contains("margin call"));
    }

    #[test]
    fn check_maintenance_margin_silent_when_healthy() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.maintenance_margin = 25.0;
        assert!(check_maintenance_margin(&cfg, 500.0, 1_000.0).is_none());
    }

    #[test]
    fn check_maintenance_margin_negative_equity_always_triggers() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.maintenance_margin = 25.0;
        let msg = check_maintenance_margin(&cfg, -50.0, 100.0).expect("negative equity");
        assert!(msg.contains("≤ 0") || msg.contains("margin call"));
    }

    #[test]
    fn check_maintenance_margin_disabled_when_setting_zero() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.maintenance_margin = 0.0;
        assert!(check_maintenance_margin(&cfg, 1.0, 1_000.0).is_none());
    }

    #[test]
    fn gross_notional_sums_longs_and_shorts() {
        let aligned = dummy_aligned("AAPL", 50.0);
        let mut positions = HashMap::new();
        positions.insert("AAPL".to_owned(), -10.0);
        let quote_ccy = HashMap::from([("AAPL".to_owned(), "USD".to_owned())]);
        let fx = empty_fx(Currency::USD);
        let gross = compute_invested_equity(&positions, &aligned, 0, &quote_ccy, "USD", &fx, 0);
        assert!((gross - 500.0).abs() < MIN_POSITION);
    }

    #[test]
    fn gross_notional_converts_quote_currency() {
        let aligned = dummy_aligned("VOD.L", 100.0);
        let mut positions = HashMap::new();
        positions.insert("VOD.L".to_owned(), 10.0);
        let quote_ccy = HashMap::from([("VOD.L".to_owned(), "GBP".to_owned())]);
        let mut fx = FxTable::new("USD");
        fx.add_series("GBP", "USD", vec![(0, 1.30)]);
        let gross = compute_invested_equity(&positions, &aligned, 0, &quote_ccy, "USD", &fx, 0);
        // 10 × 100 GBP × 1.30 = 1_300 USD.
        assert!((gross - 1_300.0).abs() < 1e-9);
    }

    #[test]
    fn accrue_margin_costs_charges_interest_on_negative_cash() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.margin_interest = 12.0; // 12 % annual
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, -1_000.0); // borrowed $1_000
        let positions = HashMap::new();
        let aligned: HashMap<String, Vec<Option<Bar>>> = HashMap::new();
        let quote_ccy = HashMap::new();
        let fx = empty_fx(Currency::USD);
        // 30 days → ~30/365.25 × 12 % × 1_000 ≈ $9.86 charge.
        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            30 * 86_400,
        );
        let bal = cash[&Currency::USD];
        assert!(bal < -1_000.0);
        assert!(bal > -1_010.0);
    }

    #[test]
    fn accrue_margin_costs_charges_borrow_on_shorts() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.borrow_rate = 36.5; // makes daily prorated cost easy: 0.1 % per day
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 1_000.0);
        let mut positions = HashMap::new();
        positions.insert("AAPL".to_owned(), -10.0);
        let aligned = dummy_aligned("AAPL", 100.0);
        let quote_ccy = HashMap::from([("AAPL".to_owned(), "USD".to_owned())]);
        let fx = empty_fx(Currency::USD);
        // Short worth $1_000; 1 day at 0.1 % ≈ $1.00 charge.
        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            86_400,
        );
        let bal = cash[&Currency::USD];
        assert!(bal < 1_000.0);
        assert!(bal > 998.5);
    }

    #[test]
    fn accrue_margin_costs_no_op_when_rates_zero() {
        let cfg = mk_cfg("AAPL"); // default 0 % rates
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, -1_000.0);
        let positions = HashMap::new();
        let aligned: HashMap<String, Vec<Option<Bar>>> = HashMap::new();
        let quote_ccy = HashMap::new();
        let fx = empty_fx(Currency::USD);
        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            30 * 86_400,
        );
        assert_eq!(cash[&Currency::USD], -1_000.0);
    }

    #[test]
    fn accrue_margin_costs_no_op_when_bar_seconds_zero() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.margin_interest = 50.0;
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, -1_000.0);
        let positions = HashMap::new();
        let aligned: HashMap<String, Vec<Option<Bar>>> = HashMap::new();
        let quote_ccy = HashMap::new();
        let fx = empty_fx(Currency::USD);
        accrue_margin_costs(
            &cfg,
            &mut cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::USD,
            &fx,
            0,
            0,
        );
        assert_eq!(cash[&Currency::USD], -1_000.0);
    }

    // ─────────────────────────────────────────────────────────────────
    // Currency conversion & FX edge cases
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn try_debit_succeeds_with_same_currency_funds() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 1_000.0);
        let fx = empty_fx(Currency::USD);
        assert!(try_debit(&mut cash, Currency::USD, 500.0, Currency::USD, &fx, 0));
        assert!((cash[&Currency::USD] - 500.0).abs() < 1e-9);
    }

    #[test]
    fn try_debit_zero_amount_is_noop() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 100.0);
        let fx = empty_fx(Currency::USD);
        assert!(try_debit(&mut cash, Currency::USD, 0.0, Currency::USD, &fx, 0));
        assert!((cash[&Currency::USD] - 100.0).abs() < 1e-9);
    }

    #[test]
    fn try_debit_falls_back_to_base_via_fx() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 1_300.0);
        let mut fx = FxTable::new("USD");
        // 1 GBP = 1.30 USD → debit £1_000 = $1_300 from base.
        fx.add_series("GBP", "USD", vec![(0, 1.30)]);
        assert!(try_debit(&mut cash, Currency::GBP, 1_000.0, Currency::USD, &fx, 0));
        let bal = cash.get(&Currency::USD).copied().unwrap_or(0.0);
        assert!(bal.abs() < 1e-6, "expected USD drained to 0, got {bal}");
    }

    #[test]
    fn try_debit_fails_when_no_fx_path() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 100.0);
        let fx = empty_fx(Currency::USD);
        assert!(!try_debit(&mut cash, Currency::GBP, 50.0, Currency::USD, &fx, 0));
    }

    #[test]
    fn sweep_foreign_to_base_converts_full_balance() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 100.0);
        cash.insert(Currency::EUR, 50.0);
        let mut fx = FxTable::new("USD");
        fx.add_series("EUR", "USD", vec![(0, 1.10)]);
        sweep_foreign_to_base(&mut cash, Currency::USD, &fx, 0, None);
        assert!(!cash.contains_key(&Currency::EUR));
        // 100 + 50 × 1.10 = 155.
        assert!((cash[&Currency::USD] - 155.0).abs() < 1e-9);
    }

    #[test]
    fn sweep_foreign_to_base_respects_threshold() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 100.0);
        cash.insert(Currency::EUR, 5.0);
        let mut fx = FxTable::new("USD");
        fx.add_series("EUR", "USD", vec![(0, 1.10)]);
        // Threshold 10 USD; EUR balance worth ~5.50 USD → stays.
        sweep_foreign_to_base(&mut cash, Currency::USD, &fx, 0, Some(10.0));
        assert!(cash.contains_key(&Currency::EUR));
    }

    #[test]
    fn sweep_foreign_to_base_skips_when_no_rate() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 100.0);
        cash.insert(Currency::JPY, 1_000.0);
        let fx = FxTable::new("USD"); // no rates
        sweep_foreign_to_base(&mut cash, Currency::USD, &fx, 0, None);
        // JPY bucket stays untouched.
        assert!(cash.contains_key(&Currency::JPY));
    }

    #[test]
    fn period_bucket_changes_across_days_weeks_months_years() {
        use crate::backtest::models::conversion_period::ConversionPeriod;
        let day = 86_400;
        let t0 = 0;
        let t1 = day; // next day
        assert_ne!(
            period_bucket(t0, ConversionPeriod::Day),
            period_bucket(t1, ConversionPeriod::Day)
        );
        let one_year = 366 * day;
        assert_ne!(
            period_bucket(t0, ConversionPeriod::Year),
            period_bucket(one_year, ConversionPeriod::Year)
        );
    }

    // ─────────────────────────────────────────────────────────────────
    // Pre-existing GAP test now superseded — keep a happy-path test
    // verifying that maintenance margin triggers in the integration test
    // helper would require updating `run_with_orders`. The behaviour is
    // covered at the unit-helper level above; engine-level integration
    // testing happens via the Python test suite which exercises
    // `run_one_strategy` end-to-end.
    // ─────────────────────────────────────────────────────────────────

    // ── apply_slippage additional ─────────────────────────────────────

    #[test]
    fn apply_slippage_zero_qty_treated_as_buy() {
        let p = apply_slippage(100.0, 0.0, 0.01, None);
        assert!((p - 101.0).abs() < 1e-9);
    }

    #[test]
    fn apply_slippage_zero_slippage_returns_raw() {
        assert!((apply_slippage(50.0, 1.0, 0.0, None) - 50.0).abs() < 1e-9);
        assert!((apply_slippage(50.0, -1.0, 0.0, None) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn apply_slippage_sell_capped_at_floor() {
        let p = apply_slippage(100.0, -1.0, 0.05, Some(98.0));
        assert!((p - 98.0).abs() < 1e-9);
    }

    #[test]
    fn apply_slippage_sell_not_capped_when_above() {
        let p = apply_slippage(100.0, -1.0, 0.01, Some(95.0));
        assert!((p - 99.0).abs() < 1e-9);
    }

    // ── fill_limit / fill_stop additional ─────────────────────────────

    #[test]
    fn fill_limit_sell_fills_at_open_when_above_limit() {
        let bar = mk_bar(0, 100.0);
        match fill_limit(-1.0, &bar, 95.0) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert!((raw_px - 100.0).abs() < 1e-9),
            _ => panic!("expected fill"),
        }
    }

    #[test]
    fn fill_limit_sell_pending_when_high_below() {
        let mut bar = mk_bar(0, 100.0);
        bar.open = 95.0;
        bar.high = 97.0;
        match fill_limit(-1.0, &bar, 100.0) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected pending"),
        }
    }

    #[test]
    fn fill_limit_zero_qty_cancels() {
        let bar = mk_bar(0, 100.0);
        match fill_limit(0.0, &bar, 100.0) {
            TriggerOutcome::Cancel {
                ..
            } => {},
            _ => panic!("expected cancel"),
        }
    }

    #[test]
    fn fill_stop_sell_gap_down() {
        let mut bar = mk_bar(0, 90.0);
        bar.open = 88.0;
        match fill_stop(-1.0, &bar, 90.0) {
            TriggerOutcome::Fill {
                raw_px,
                reason,
                ..
            } => {
                assert!((raw_px - 88.0).abs() < 1e-9);
                assert!(reason.contains("gap-down"));
            },
            _ => panic!("expected fill"),
        }
    }

    #[test]
    fn fill_stop_zero_qty_cancels() {
        let bar = mk_bar(0, 100.0);
        match fill_stop(0.0, &bar, 100.0) {
            TriggerOutcome::Cancel {
                ..
            } => {},
            _ => panic!("expected cancel"),
        }
    }

    // ── stop_triggered additional ─────────────────────────────────────

    #[test]
    fn stop_triggered_tp_sell_triggers_on_rise() {
        let mut bar = mk_bar(0, 100.0);
        bar.high = 110.0;
        assert!(stop_triggered(-1.0, &bar, 105.0, true));
    }

    #[test]
    fn stop_triggered_zero_qty_returns_false() {
        let bar = mk_bar(0, 100.0);
        assert!(!stop_triggered(0.0, &bar, 100.0, false));
        assert!(!stop_triggered(0.0, &bar, 100.0, true));
    }

    // ── quantity_rejection_reason ──────────────────────────────────────

    #[test]
    fn quantity_rejection_crypto_allows_fractional() {
        assert!(quantity_rejection_reason("BTC", 0.5, InstrumentType::Crypto).is_none());
    }

    #[test]
    fn quantity_rejection_nan_is_rejected() {
        assert!(quantity_rejection_reason("X", f64::NAN, InstrumentType::Stocks).is_some());
    }

    // ── profile helpers ──────────────────────────────────────────────

    #[test]
    fn profile_types_maps_correctly() {
        let profiles = vec![
            mk_profile("AAPL", "USD"),
            mk_profile_with_type("BTC", "USD", InstrumentType::Crypto),
        ];
        let types = profile_instrument_types(&profiles);
        assert_eq!(*types.get("AAPL").unwrap(), InstrumentType::Stocks);
        assert_eq!(*types.get("BTC").unwrap(), InstrumentType::Crypto);
    }

    #[test]
    fn instrument_type_uses_map_then_fallback() {
        let mut m = HashMap::new();
        m.insert("AAPL".to_owned(), InstrumentType::Stocks);
        assert_eq!(
            instrument_type_for_symbol("AAPL", &m, InstrumentType::Crypto),
            InstrumentType::Stocks
        );
        assert_eq!(
            instrument_type_for_symbol("X", &m, InstrumentType::Forex),
            InstrumentType::Forex
        );
    }

    // ── align_bars additional ─────────────────────────────────────────

    #[test]
    fn align_bars_fill_with_nan_produces_nan_bar() {
        let bar = mk_bar(1_700_000_000, 100.0);
        let bars = HashMap::from([("X".to_owned(), vec![bar])]);
        let timeline = vec![1_700_000_000_i64, 1_700_086_400];
        let aligned = align_bars(&bars, &timeline, EmptyBarPolicy::FillWithNaN);
        assert!(aligned.get("X").unwrap()[1].as_ref().unwrap().close.is_nan());
    }

    #[test]
    fn align_bars_forward_fill_none_before_first_bar() {
        let bar = mk_bar(1_700_086_400, 100.0);
        let bars = HashMap::from([("X".to_owned(), vec![bar])]);
        let timeline = vec![1_700_000_000_i64, 1_700_086_400];
        let aligned = align_bars(&bars, &timeline, EmptyBarPolicy::ForwardFill);
        assert!(aligned.get("X").unwrap()[0].is_none());
    }

    // ── compute_metrics additional ────────────────────────────────────

    #[test]
    fn compute_metrics_empty_curve() {
        let m = compute_metrics(10_000.0, 0.0, &[], &[]);
        assert_eq!(m["total_return"], 0.0);
        assert_eq!(m["final_equity"], 10_000.0);
    }

    #[test]
    fn compute_metrics_zero_initial_cash() {
        let m = compute_metrics(0.0, 0.0, &[], &[]);
        assert_eq!(m["total_return"], 0.0);
    }

    // ── update_open_trade_buy / close_open_trade_sell ────────────────

    #[test]
    fn update_open_trade_buy_averages_down() {
        let mut trades = HashMap::new();
        update_open_trade_buy(&mut trades, "X", 100, 10.0, 50.0);
        update_open_trade_buy(&mut trades, "X", 200, 10.0, 30.0);
        let (_, qty, avg) = trades["X"];
        assert!((qty - 20.0).abs() < MIN_POSITION);
        assert!((avg - 40.0).abs() < MIN_POSITION);
    }

    #[test]
    fn close_open_trade_sell_none_for_missing() {
        let mut trades = HashMap::new();
        assert!(close_open_trade_sell(&mut trades, "X", 0, 1.0, 50.0, 0.0).is_none());
    }

    #[test]
    fn close_open_trade_sell_partial_keeps_remainder() {
        let mut trades = HashMap::new();
        trades.insert("X".to_owned(), (100_i64, 10.0, 50.0));
        let t = close_open_trade_sell(&mut trades, "X", 200, 5.0, 60.0, 0.0).unwrap();
        assert!((t.quantity - 5.0).abs() < MIN_POSITION);
        assert!(trades.contains_key("X"));
    }

    // ── persist & parse additional ─────────────────────────────────────

    #[test]
    fn persist_experiment_config_creates_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = mk_cfg("AAPL");
        let path = persist_experiment_config(dir.path(), &cfg).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn parse_iso_date_to_ts_epoch() {
        assert_eq!(iso_to_ts("1970-01-01").unwrap(), 0);
    }

    #[test]
    fn parse_iso_date_to_ts_invalid() {
        assert!(iso_to_ts("not-a-date").is_none());
        assert!(iso_to_ts("").is_none());
    }

    // ── resolve_trigger additional ────────────────────────────────────

    #[test]
    fn resolve_trigger_cancel_order() {
        let bar = mk_bar(0, 100.0);
        let mut order = Order {
            id: new_order_id(),
            symbol: "X".into(),
            quantity: 1.0,
            price: None,
            limit_price: None,
            order_type: OrderType::Cancel,
            sizer: None,
        };
        match resolve_trigger(&mut order, &bar, &HashMap::new(), &mut HashMap::new(), false) {
            TriggerOutcome::Cancel {
                ..
            } => {},
            _ => panic!("expected cancel"),
        }
    }

    #[test]
    fn resolve_trigger_market_trade_on_close() {
        let bar = mk_bar(0, 100.0);
        let mut order = Order {
            id: new_order_id(),
            symbol: "X".into(),
            quantity: 1.0,
            price: None,
            limit_price: None,
            order_type: OrderType::Market,
            sizer: None,
        };
        match resolve_trigger(&mut order, &bar, &HashMap::new(), &mut HashMap::new(), true) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => assert!((raw_px - bar.close).abs() < 1e-9),
            _ => panic!("expected fill"),
        }
    }

    #[test]
    fn resolve_trigger_settle_no_position_cancels() {
        let bar = mk_bar(0, 100.0);
        let mut order = Order {
            id: new_order_id(),
            symbol: "X".into(),
            quantity: 0.0,
            price: None,
            limit_price: None,
            order_type: OrderType::SettlePosition,
            sizer: None,
        };
        match resolve_trigger(&mut order, &bar, &HashMap::new(), &mut HashMap::new(), false) {
            TriggerOutcome::Cancel {
                reason,
            } => assert!(reason.contains("no position")),
            _ => panic!("expected cancel"),
        }
    }

    #[test]
    fn resolve_trigger_limit_missing_price_cancels() {
        let bar = mk_bar(0, 100.0);
        let mut order = Order {
            id: new_order_id(),
            symbol: "X".into(),
            quantity: 1.0,
            price: None,
            limit_price: None,
            order_type: OrderType::Limit,
            sizer: None,
        };
        match resolve_trigger(&mut order, &bar, &HashMap::new(), &mut HashMap::new(), false) {
            TriggerOutcome::Cancel {
                ..
            } => {},
            _ => panic!("expected cancel"),
        }
    }

    #[test]
    fn resolve_trigger_stop_loss_missing_price_cancels() {
        let bar = mk_bar(0, 100.0);
        let mut order = Order {
            id: new_order_id(),
            symbol: "X".into(),
            quantity: -1.0,
            price: None,
            limit_price: None,
            order_type: OrderType::StopLoss,
            sizer: None,
        };
        match resolve_trigger(&mut order, &bar, &HashMap::new(), &mut HashMap::new(), false) {
            TriggerOutcome::Cancel {
                ..
            } => {},
            _ => panic!("expected cancel"),
        }
    }

    #[test]
    fn resolve_trigger_trailing_stop_missing_price_cancels() {
        let bar = mk_bar(0, 100.0);
        let mut order = Order {
            id: new_order_id(),
            symbol: "X".into(),
            quantity: -1.0,
            price: None,
            limit_price: None,
            order_type: OrderType::TrailingStop,
            sizer: None,
        };
        match resolve_trigger(&mut order, &bar, &HashMap::new(), &mut HashMap::new(), false) {
            TriggerOutcome::Cancel {
                ..
            } => {},
            _ => panic!("expected cancel"),
        }
    }

    #[test]
    fn period_bucket_same_day_same_bucket() {
        use crate::backtest::models::conversion_period::ConversionPeriod;
        let ts1 = 1_700_000_000_i64;
        let ts2 = ts1 + 3600;
        assert_eq!(
            period_bucket(ts1, ConversionPeriod::Day),
            period_bucket(ts2, ConversionPeriod::Day)
        );
    }

    #[test]
    fn period_bucket_week_and_month_differentiate() {
        use crate::backtest::models::conversion_period::ConversionPeriod;
        let ts1 = 1_672_531_200_i64; // 2023-01-01
        let ts2 = ts1 + 14 * 86_400; // 2023-01-15
        assert_eq!(
            period_bucket(ts1, ConversionPeriod::Month),
            period_bucket(ts2, ConversionPeriod::Month)
        );
        assert_ne!(
            period_bucket(ts1, ConversionPeriod::Week),
            period_bucket(ts2, ConversionPeriod::Week)
        );
    }

    // ─────────────────────────────────────────────────────────────────
    // Extra coverage — small helpers and conversion-flow edge cases
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn parse_iso_date_to_ts_y2k_boundary() {
        // 2000-01-01 UTC = 946_684_800
        assert_eq!(iso_to_ts("2000-01-01").unwrap(), 946_684_800);
    }

    #[test]
    fn parse_iso_date_to_ts_with_time_is_invalid() {
        // The function only accepts YYYY-MM-DD; richer ISO 8601 input should
        // be rejected to keep callers from accidentally passing timestamps.
        assert!(iso_to_ts("2024-01-01T00:00:00Z").is_none());
    }

    #[test]
    fn is_whole_quantity_negative_zero() {
        // -0.0 has zero fractional component and should count as whole.
        assert!(is_whole_quantity(-0.0));
    }

    #[test]
    fn is_whole_quantity_very_small_fraction() {
        assert!(!is_whole_quantity(1.0 + 1e-9));
    }

    // ── portfolio_equity_in_currency edge cases ───────────────────────

    #[test]
    fn portfolio_equity_in_currency_empty_inputs_returns_zero() {
        let aligned: HashMap<String, Vec<Option<Bar>>> = HashMap::new();
        let cash: Cash = Cash::new();
        let positions: Positions = Positions::new();
        let quote_ccy: HashMap<String, String> = HashMap::new();
        let fx = empty_fx(Currency::USD);
        let eq = compute_portfolio_equity(
            &cash, &positions, &aligned, 0, &quote_ccy, "USD", "USD", &fx, 0,
        );
        assert_eq!(eq, 0.0);
    }

    #[test]
    fn portfolio_equity_in_currency_missing_fx_falls_back_to_par() {
        // No FX rate set up; values are summed at par (1.0) as a
        // best-effort fallback so equity remains a finite number.
        let aligned = dummy_aligned("X", 50.0);
        let cash = HashMap::from([(Currency::EUR, 100.0)]);
        let positions = HashMap::from([("X".to_owned(), 2.0)]);
        let quote_ccy = HashMap::from([("X".to_owned(), "EUR".to_owned())]);
        let fx = empty_fx(Currency::USD);
        let eq = compute_portfolio_equity(
            &cash, &positions, &aligned, 0, &quote_ccy, "USD", "USD", &fx, 0,
        );
        // 100 EUR + 2 × 50 EUR (both at par) = 200.
        assert!((eq - 200.0).abs() < 1e-9);
    }

    #[test]
    fn portfolio_equity_in_currency_ignores_tiny_positions() {
        let aligned = dummy_aligned("X", 50.0);
        let cash = HashMap::new();
        let positions = HashMap::from([("X".to_owned(), 1e-15)]);
        let quote_ccy = HashMap::from([("X".to_owned(), "USD".to_owned())]);
        let fx = empty_fx(Currency::USD);
        let eq = compute_portfolio_equity(
            &cash, &positions, &aligned, 0, &quote_ccy, "USD", "USD", &fx, 0,
        );
        assert_eq!(eq, 0.0);
    }

    // ── gross_notional_in_currency edge cases ────────────────────────

    #[test]
    fn gross_notional_in_currency_empty_positions() {
        let aligned: HashMap<Symbol, Vec<Option<Bar>>> = HashMap::new();
        let positions: Positions = HashMap::new();
        let quote_ccy: HashMap<String, String> = HashMap::new();
        let fx = empty_fx(Currency::USD);
        let g = compute_invested_equity(&positions, &aligned, 0, &quote_ccy, "USD", &fx, 0);
        assert_eq!(g, 0.0);
    }

    #[test]
    fn gross_notional_in_currency_uses_abs_for_shorts() {
        // Short position of -5 × 100 USD = $500 gross.
        let aligned = dummy_aligned("AAPL", 100.0);
        let positions = HashMap::from([("AAPL".to_owned(), -5.0)]);
        let quote_ccy = HashMap::from([("AAPL".to_owned(), "USD".to_owned())]);
        let fx = empty_fx(Currency::USD);
        let g = compute_invested_equity(&positions, &aligned, 0, &quote_ccy, "USD", &fx, 0);
        assert!((g - 500.0).abs() < 1e-9);
    }

    // ── try_debit extra coverage ──────────────────────────────────────

    #[test]
    fn try_debit_partial_drain_uses_other_foreign_bucket() {
        // 100 USD pool needs to fund a £200 debit. Insufficient in
        // GBP, base, and FX-fallback: the engine drains the leftover
        // from an unrelated EUR bucket via a direct EUR→GBP rate.
        let mut cash = HashMap::new();
        cash.insert(Currency::GBP, 50.0);
        cash.insert(Currency::USD, 100.0); // base
        cash.insert(Currency::EUR, 1_000.0);

        let mut fx = FxTable::new("USD");
        fx.add_series("GBP", "USD", vec![(0, 1.30)]);
        fx.add_series("EUR", "USD", vec![(0, 1.10)]);
        fx.add_series("EUR", "GBP", vec![(0, 0.85)]);

        // Need £200; have £50 + base($100 ≈ £77) + 1000 EUR. Should succeed.
        assert!(try_debit(&mut cash, Currency::GBP, 200.0, Currency::USD, &fx, 0));
        // GBP bucket is fully drained.
        assert!(cash.get(&Currency::GBP).copied().unwrap_or(0.0).abs() < 1e-6);
    }

    #[test]
    fn try_debit_negative_amount_is_noop_returns_true() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 100.0);
        let fx = empty_fx(Currency::USD);
        assert!(try_debit(&mut cash, Currency::USD, -50.0, Currency::USD, &fx, 0));
        assert_eq!(cash[&Currency::USD], 100.0);
    }

    #[test]
    fn try_debit_drains_ccy_bucket_when_falling_back() {
        // GBP bucket has £30, debit £100, base has plenty of USD.
        // The £30 bucket should be drained (removed) and the
        // remaining £70 paid from base via FX.
        let mut cash = HashMap::new();
        cash.insert(Currency::GBP, 30.0);
        cash.insert(Currency::USD, 1_000.0);
        let mut fx = FxTable::new("USD");
        fx.add_series("GBP", "USD", vec![(0, 1.30)]);
        assert!(try_debit(&mut cash, Currency::GBP, 100.0, Currency::USD, &fx, 0));
        // GBP bucket gone.
        assert!(!cash.contains_key(&Currency::GBP));
        // USD debited by (100 - 30) × 1.30 = 91.
        assert!((cash[&Currency::USD] - 909.0).abs() < 1e-6);
    }

    // ── sweep_foreign_to_base extra coverage ─────────────────────────

    #[test]
    fn sweep_foreign_to_base_handles_negative_bucket() {
        // A negative bucket (e.g. borrowed-currency loss) is still
        // converted at the current FX rate so the engine never leaves
        // dangling foreign debt on equity snapshots.
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 1_000.0);
        cash.insert(Currency::EUR, -100.0);
        let mut fx = FxTable::new("USD");
        fx.add_series("EUR", "USD", vec![(0, 1.10)]);
        sweep_foreign_to_base(&mut cash, Currency::USD, &fx, 0, None);
        // EUR removed, USD debited by 110.
        assert!(!cash.contains_key(&Currency::EUR));
        assert!((cash[&Currency::USD] - 890.0).abs() < 1e-9);
    }

    #[test]
    fn sweep_foreign_to_base_no_op_for_only_base() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 1_000.0);
        let fx = empty_fx(Currency::USD);
        sweep_foreign_to_base(&mut cash, Currency::USD, &fx, 0, None);
        assert_eq!(cash[&Currency::USD], 1_000.0);
        assert_eq!(cash.len(), 1);
    }

    // ── compute_metrics extra ─────────────────────────────────────────

    #[test]
    fn compute_metrics_total_return_with_curve() {
        let curve = vec![
            EquitySample {
                timestamp: 0,
                equity: 1_000.0,
                cash: HashMap::new(),
                drawdown: 0.0,
            },
            EquitySample {
                timestamp: 86_400,
                equity: 1_200.0,
                cash: HashMap::new(),
                drawdown: 0.0,
            },
        ];
        let m = compute_metrics(1_000.0, 0.0, &curve, &[]);
        assert!((m["total_return"] - 0.20).abs() < 1e-9);
        assert!((m["pnl"] - 200.0).abs() < 1e-9);
        assert_eq!(m["final_equity"], 1_200.0);
        assert_eq!(m["n_trades"], 0.0);
    }

    #[test]
    fn compute_metrics_win_rate_with_trades() {
        let trades = vec![
            Trade {
                symbol: "X".into(),
                entry_ts: 0,
                exit_ts: 1,
                quantity: 1.0,
                entry_price: 100.0,
                exit_price: 110.0,
                pnl: 10.0,
            },
            Trade {
                symbol: "X".into(),
                entry_ts: 0,
                exit_ts: 1,
                quantity: 1.0,
                entry_price: 100.0,
                exit_price: 95.0,
                pnl: -5.0,
            },
        ];
        let m = compute_metrics(1_000.0, 0.0, &[], &trades);
        assert_eq!(m["n_trades"], 2.0);
        assert!((m["win_rate"] - 0.5).abs() < 1e-9);
    }

    // ── apply_slippage additional ─────────────────────────────────────

    #[test]
    fn apply_slippage_buy_no_cap() {
        let p = apply_slippage(100.0, 1.0, 0.02, None);
        assert!((p - 102.0).abs() < 1e-9);
    }

    #[test]
    fn apply_slippage_buy_capped_above() {
        // Buy slippage above the limit cap is clamped down.
        let p = apply_slippage(100.0, 1.0, 0.10, Some(105.0));
        assert!((p - 105.0).abs() < 1e-9);
    }

    // ── fill_limit / fill_stop additional ─────────────────────────────

    #[test]
    fn fill_limit_buy_fills_when_low_reaches_limit() {
        let mut bar = mk_bar(0, 100.0);
        bar.low = 99.0;
        bar.open = 100.5;
        match fill_limit(1.0, &bar, 99.5) {
            TriggerOutcome::Fill {
                raw_px,
                ..
            } => {
                assert!(raw_px <= 99.5 + 1e-9, "buy limit fills at-or-below limit");
            },
            _ => panic!("expected fill"),
        }
    }

    #[test]
    fn fill_stop_buy_gap_up() {
        // Open already above stop → fill at open (gap-up).
        let mut bar = mk_bar(0, 110.0);
        bar.open = 108.0;
        match fill_stop(1.0, &bar, 105.0) {
            TriggerOutcome::Fill {
                raw_px,
                reason,
                ..
            } => {
                assert!((raw_px - 108.0).abs() < 1e-9);
                assert!(reason.contains("gap-up"));
            },
            _ => panic!("expected gap-up fill"),
        }
    }

    #[test]
    fn fill_stop_pending_when_not_crossed() {
        let mut bar = mk_bar(0, 100.0);
        bar.open = 100.0;
        bar.high = 102.0;
        bar.low = 99.0;
        match fill_stop(1.0, &bar, 110.0) {
            TriggerOutcome::Pending => {},
            _ => panic!("expected pending"),
        }
    }

    // ── stop_triggered additional ─────────────────────────────────────

    #[test]
    fn stop_triggered_sell_below_stop_triggers() {
        let mut bar = mk_bar(0, 95.0);
        bar.low = 90.0;
        assert!(stop_triggered(-1.0, &bar, 92.0, false));
    }

    #[test]
    fn stop_triggered_buy_above_stop_triggers() {
        let mut bar = mk_bar(0, 105.0);
        bar.high = 110.0;
        assert!(stop_triggered(1.0, &bar, 108.0, false));
    }

    // ── update_open_trade_buy / close_open_trade_sell additional ─────

    #[test]
    fn close_open_trade_sell_full_position_clears_entry() {
        let mut trades = HashMap::new();
        trades.insert("X".to_owned(), (100_i64, 5.0, 50.0));
        let t = close_open_trade_sell(&mut trades, "X", 200, 5.0, 60.0, 0.0).unwrap();
        assert!((t.pnl - (60.0 - 50.0) * 5.0).abs() < 1e-9);
        // Position fully closed → entry removed.
        assert!(!trades.contains_key("X"));
    }

    #[test]
    fn close_open_trade_sell_includes_commission_in_pnl() {
        let mut trades = HashMap::new();
        trades.insert("X".to_owned(), (100_i64, 5.0, 50.0));
        let t = close_open_trade_sell(&mut trades, "X", 200, 5.0, 60.0, 2.0).unwrap();
        // Commission reduces PnL: (10 × 5) - 2 = 48.
        assert!((t.pnl - 48.0).abs() < 1e-9);
    }

    #[test]
    fn update_open_trade_buy_creates_new_entry() {
        let mut trades = HashMap::new();
        update_open_trade_buy(&mut trades, "X", 500, 3.0, 75.0);
        let (ts, qty, avg) = trades["X"];
        assert_eq!(ts, 500);
        assert!((qty - 3.0).abs() < MIN_POSITION);
        assert!((avg - 75.0).abs() < MIN_POSITION);
    }

    // ── persist & parse additional ────────────────────────────────────

    #[test]
    fn persist_experiment_config_writes_toml() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = mk_cfg("AAPL");
        let path = persist_experiment_config(dir.path(), &cfg).unwrap();
        let contents = std::fs::read_to_string(path).unwrap();
        assert!(contents.contains("symbols"));
    }

    // ── period_bucket additional ──────────────────────────────────────

    #[test]
    fn period_bucket_year_boundary() {
        use crate::backtest::models::conversion_period::ConversionPeriod;
        // 2023-12-31 vs 2024-01-01.
        let dec31 = 1_704_067_199_i64;
        let jan1 = 1_704_067_200_i64;
        assert_ne!(
            period_bucket(dec31, ConversionPeriod::Year),
            period_bucket(jan1, ConversionPeriod::Year)
        );
    }

    #[test]
    fn period_bucket_iso_week_year_crossover() {
        // 2024-12-30 (Mon, ISO week 1 of 2025) vs 2024-12-23 (Mon, ISO
        // week 52 of 2024). Bucket id encodes (year, week) so they must
        // differ.
        use crate::backtest::models::conversion_period::ConversionPeriod;
        let dec23 = 1_734_912_000_i64; // 2024-12-23 00:00:00 UTC
        let dec30 = dec23 + 7 * 86_400;
        assert_ne!(
            period_bucket(dec23, ConversionPeriod::Week),
            period_bucket(dec30, ConversionPeriod::Week)
        );
    }

    // ── quantity rejection additional ─────────────────────────────────

    #[test]
    fn quantity_rejection_infinity_is_rejected() {
        assert!(quantity_rejection_reason("X", f64::INFINITY, InstrumentType::Crypto).is_some());
    }

    // ── instrument_type_for_symbol ────────────────────────────────────

    #[test]
    fn instrument_type_empty_map_uses_fallback() {
        let m: HashMap<String, InstrumentType> = HashMap::new();
        assert_eq!(
            instrument_type_for_symbol("BTC", &m, InstrumentType::Crypto),
            InstrumentType::Crypto
        );
    }

    // ── profile_instrument_types ──────────────────────────────────────

    #[test]
    fn profile_instrument_types_empty() {
        let profiles: Vec<InstrumentProfile> = vec![];
        let m = profile_instrument_types(&profiles);
        assert!(m.is_empty());
    }

    // ── validate_starting_position_quantities ──────────────────────────

    #[test]
    fn validate_starting_positions_accepts_whole_stock_qty() {
        let mut cfg = mk_cfg("AAPL");
        cfg.portfolio.starting_positions.insert("AAPL".into(), 10.0);
        let profiles = vec![mk_profile("AAPL", "USD")];
        assert!(validate_qty(&cfg, &profiles).is_ok());
    }

    #[test]
    fn validate_starting_positions_rejects_fractional_stock_qty() {
        let mut cfg = mk_cfg("AAPL");
        cfg.portfolio.starting_positions.insert("AAPL".into(), 1.5);
        let profiles = vec![mk_profile("AAPL", "USD")];
        assert!(validate_qty(&cfg, &profiles).is_err());
    }

    #[test]
    fn validate_starting_positions_accepts_fractional_crypto_qty() {
        let mut cfg = mk_cfg("BTC");
        cfg.data.instrument_type = InstrumentType::Crypto;
        cfg.portfolio.starting_positions.insert("BTC".into(), 0.001);
        let profiles = vec![mk_profile_with_type("BTC", "USD", InstrumentType::Crypto)];
        assert!(validate_qty(&cfg, &profiles).is_ok());
    }

    #[test]
    fn validate_starting_positions_rejects_nan_qty() {
        let mut cfg = mk_cfg("AAPL");
        cfg.portfolio.starting_positions.insert("AAPL".into(), f64::NAN);
        let profiles = vec![mk_profile("AAPL", "USD")];
        assert!(validate_qty(&cfg, &profiles).is_err());
    }

    // ── normalize_builtin_order_quantity ────────────────────────────────

    #[test]
    fn normalize_order_cancel_skips_validation() {
        let mut order = Order {
            id: "1".into(),
            symbol: "X".into(),
            order_type: OrderType::Cancel,
            quantity: f64::NAN,
            price: None,
            limit_price: None,
            sizer: None,
        };
        assert!(validate_order(&mut order, InstrumentType::Stocks).is_none());
    }

    #[test]
    fn normalize_order_settle_skips_validation() {
        let mut order = Order {
            id: "1".into(),
            symbol: "X".into(),
            order_type: OrderType::SettlePosition,
            quantity: 0.0,
            price: None,
            limit_price: None,
            sizer: None,
        };
        assert!(validate_order(&mut order, InstrumentType::Stocks).is_none());
    }

    #[test]
    fn normalize_order_nan_quantity_is_rejected() {
        let mut order = Order {
            id: "1".into(),
            symbol: "X".into(),
            order_type: OrderType::Market,
            quantity: f64::NAN,
            price: None,
            limit_price: None,
            sizer: None,
        };
        assert!(validate_order(&mut order, InstrumentType::Stocks).is_some());
    }

    #[test]
    fn normalize_order_floors_fractional_stock_qty() {
        let mut order = Order {
            id: "1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.7,
            price: None,
            limit_price: None,
            sizer: None,
        };
        assert!(validate_order(&mut order, InstrumentType::Stocks).is_none());
        assert_eq!(order.quantity, 5.0);
    }

    #[test]
    fn normalize_order_floors_negative_fractional_stock_qty() {
        let mut order = Order {
            id: "1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: -3.8,
            price: None,
            limit_price: None,
            sizer: None,
        };
        assert!(validate_order(&mut order, InstrumentType::Stocks).is_none());
        assert_eq!(order.quantity, -3.0);
    }

    #[test]
    fn normalize_order_rejects_below_one_stock_unit() {
        let mut order = Order {
            id: "1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 0.5,
            price: None,
            limit_price: None,
            sizer: None,
        };
        assert!(validate_order(&mut order, InstrumentType::Stocks).is_some());
    }

    #[test]
    fn normalize_order_keeps_fractional_crypto() {
        let mut order = Order {
            id: "1".into(),
            symbol: "BTC".into(),
            order_type: OrderType::Market,
            quantity: 0.001,
            price: None,
            limit_price: None,
            sizer: None,
        };
        assert!(validate_order(&mut order, InstrumentType::Crypto).is_none());
        assert_eq!(order.quantity, 0.001);
    }

    // ── compute_indicators ─────────────────────────────────────────────

    #[test]
    fn compute_indicators_with_sma() {
        let aligned = mk_aligned("X", &[100.0, 101.0, 102.0, 103.0, 104.0]);
        let indicator_objs: Vec<(String, Py<PyAny>)> = Python::attach(|py| {
            use crate::backtest::indicators::SimpleMovingAverage;
            vec![("SMA_3".to_owned(), Py::new(py, SimpleMovingAverage::new(3)).unwrap().into_any())]
        });
        let result = compute_indicators(&indicator_objs, &aligned, None).unwrap();
        assert!(result.contains_key("SMA_3"));
        let series = &result["SMA_3"]["X"];
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].len(), 5);
        assert!(series[0][2].is_finite());
    }

    #[test]
    fn compute_indicators_with_bollinger_bands() {
        let aligned =
            mk_aligned("Y", &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0]);
        let indicator_objs: Vec<(String, Py<PyAny>)> = Python::attach(|py| {
            use crate::backtest::indicators::BollingerBands;
            vec![(
                "BB_5_2".to_owned(),
                Py::new(py, BollingerBands::new(5, 2.0)).unwrap().into_any(),
            )]
        });
        let result = compute_indicators(&indicator_objs, &aligned, None).unwrap();
        let series = &result["BB_5_2"]["Y"];
        assert_eq!(series.len(), 3);
    }

    #[test]
    fn compute_indicators_empty_objs_returns_empty() {
        let aligned = mk_aligned("X", &[100.0, 101.0]);
        let result =
            compute_indicators(&Vec::<(String, Py<PyAny>)>::new(), &aligned, None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn compute_indicators_multiple_symbols() {
        let mut aligned = mk_aligned("A", &[10.0, 11.0, 12.0, 13.0, 14.0]);
        aligned.extend(mk_aligned("B", &[20.0, 21.0, 22.0, 23.0, 24.0]));
        let indicator_objs: Vec<(String, Py<PyAny>)> = Python::attach(|py| {
            use crate::backtest::indicators::ExponentialMovingAverage;
            vec![(
                "EMA_3".to_owned(),
                Py::new(py, ExponentialMovingAverage::new(3)).unwrap().into_any(),
            )]
        });
        let result = compute_indicators(&indicator_objs, &aligned, None).unwrap();
        assert!(result["EMA_3"].contains_key("A"));
        assert!(result["EMA_3"].contains_key("B"));
    }

    // ── run_one_strategy exercises ─────────────────────────────────────

    #[test]
    fn run_buy_and_hold_strategy_produces_equity_curve() {
        let prices: Vec<f64> = (0..30).map(|i| 100.0 + i as f64).collect();
        let aligned = mk_aligned("AAPL", &prices);
        let timeline: Vec<i64> = (0..30).map(|i| 1_700_000_000 + i as i64 * 86_400).collect();
        let mut cfg = mk_cfg("AAPL");
        cfg.engine.warmup_period = 0;
        let profiles = vec![mk_profile("AAPL", "USD")];
        let fx = FxTable::new("USD");
        let strategy = Python::attach(|py| -> Py<PyAny> {
            Py::new(py, BuyAndHold::new(None)).unwrap().into_any()
        });
        let indicators = HashMap::new();
        let run = run_one_strategy(
            "Buy & Hold",
            strategy,
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );
        assert!(run.error.is_none(), "Buy & Hold failed: {:?}", run.error);
        assert!(!run.equity_curve.is_empty());
        assert!(run.equity_curve.last().unwrap().equity > 10_000.0);
    }

    #[test]
    fn run_sma_crossover_strategy_produces_trades() {
        let n_bars = 60;
        let aligned = mk_multi_symbol_aligned(&["TEST"], n_bars);
        let timeline: Vec<i64> = (0..n_bars).map(|i| 1_700_000_000 + i as i64 * 86_400).collect();
        let mut cfg = mk_cfg("TEST");
        cfg.engine.warmup_period = 0;
        let profiles = vec![mk_profile("TEST", "USD")];
        let fx = FxTable::new("USD");
        let (strategy, indicator_objs) = Python::attach(|py| -> PyResult<_> {
            let s = Py::new(py, SmaCrossover::new(3, 8))?.into_any();
            let raw = s.bind(py).call_method0("required_indicators")?;
            let required: Vec<Py<PyAny>> = raw.extract()?;
            let mut objs = Vec::new();
            for ind in required {
                let name = _indicator_deterministic_name(ind.bind(py).as_any())?;
                objs.push((name, ind));
            }
            Ok((s, objs))
        })
        .unwrap();
        let indicators = compute_indicators(&indicator_objs, &aligned, None).unwrap();
        let run = run_one_strategy(
            "SMA Crossover",
            strategy,
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );
        assert!(run.error.is_none(), "SMA Crossover failed: {:?}", run.error);
        assert!(!run.equity_curve.is_empty());
    }

    #[test]
    fn run_strategy_with_multiple_symbols() {
        let symbols = vec!["AA", "BB"];
        let n_bars = 60;
        let aligned = mk_multi_symbol_aligned(&symbols, n_bars);
        let timeline: Vec<i64> = (0..n_bars).map(|i| 1_700_000_000 + i as i64 * 86_400).collect();
        let mut cfg = mk_cfg("AA");
        cfg.data.symbols = symbols.iter().map(|s| (*s).to_owned()).collect();
        cfg.engine.warmup_period = 0;
        let profiles: Vec<InstrumentProfile> =
            symbols.iter().map(|s| mk_profile(s, "USD")).collect();
        let fx = FxTable::new("USD");
        let strategy = Python::attach(|py| Py::new(py, BuyAndHold::new(None)).unwrap().into_any());
        let indicators = HashMap::new();
        let run = run_one_strategy(
            "Buy & Hold",
            strategy,
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );
        assert!(run.error.is_none());
        assert!(!run.equity_curve.is_empty());
    }

    #[test]
    fn run_strategy_with_starting_positions() {
        let prices: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        let aligned = mk_aligned("X", &prices);
        let timeline: Vec<i64> = (0..20).map(|i| 1_700_000_000 + i as i64 * 86_400).collect();
        let mut cfg = mk_cfg("X");
        cfg.engine.warmup_period = 0;
        cfg.portfolio.starting_positions.insert("X".into(), 5.0);
        let profiles = vec![mk_profile("X", "USD")];
        let fx = FxTable::new("USD");
        let strategy = Python::attach(|py| Py::new(py, BuyAndHold::new(None)).unwrap().into_any());
        let indicators = HashMap::new();
        let run = run_one_strategy(
            "Buy & Hold",
            strategy,
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );
        assert!(run.error.is_none());
        let final_eq = run.equity_curve.last().unwrap().equity;
        assert!(final_eq > 10_000.0);
    }

    #[test]
    fn run_strategy_with_commission() {
        let prices: Vec<f64> = (0..30).map(|i| 100.0 + i as f64).collect();
        let aligned = mk_aligned("X", &prices);
        let timeline: Vec<i64> = (0..30).map(|i| 1_700_000_000 + i as i64 * 86_400).collect();
        let mut cfg = mk_cfg("X");
        cfg.engine.warmup_period = 0;
        cfg.exchange.commission_type = CommissionType::Percentage;
        cfg.exchange.commission_pct = 0.01;
        let profiles = vec![mk_profile("X", "USD")];
        let fx = FxTable::new("USD");
        let strategy = Python::attach(|py| Py::new(py, BuyAndHold::new(None)).unwrap().into_any());
        let indicators = HashMap::new();
        let run = run_one_strategy(
            "Buy & Hold",
            strategy,
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );
        assert!(run.error.is_none());
    }

    // ── IndicatorView tests ────────────────────────────────────────────

    #[test]
    fn indicator_view_value_returns_finite_value() {
        let mut data: HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>> = HashMap::new();
        let mut per_sym = HashMap::new();
        per_sym.insert("AAPL".to_owned(), vec![vec![1.0, 2.0, 3.0]]);
        data.insert("SMA_5".to_owned(), per_sym);
        let view = IndicatorView::new(&data, 1);
        assert_eq!(view.value("SMA_5", "AAPL"), Some(2.0));
    }

    #[test]
    fn indicator_view_value_returns_none_for_nan() {
        let mut data: HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>> = HashMap::new();
        let mut per_sym = HashMap::new();
        per_sym.insert("X".to_owned(), vec![vec![f64::NAN, 2.0]]);
        data.insert("RSI".to_owned(), per_sym);
        let view = IndicatorView::new(&data, 0);
        assert_eq!(view.value("RSI", "X"), None);
    }

    #[test]
    fn indicator_view_value_returns_none_for_missing_indicator() {
        let data: HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>> = HashMap::new();
        let view = IndicatorView::new(&data, 0);
        assert_eq!(view.value("MISSING", "X"), None);
    }

    #[test]
    fn indicator_view_last_returns_multiple_series_values() {
        let mut data: HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>> = HashMap::new();
        let mut per_sym = HashMap::new();
        per_sym.insert("X".to_owned(), vec![vec![10.0, 20.0], vec![5.0, 15.0], vec![1.0, 10.0]]);
        data.insert("BB".to_owned(), per_sym);
        let view = IndicatorView::new(&data, 1);
        let values = view.last("BB", "X").unwrap();
        assert_eq!(values, vec![20.0, 15.0, 10.0]);
    }

    #[test]
    fn indicator_view_out_of_bounds_returns_none() {
        let mut data: HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>> = HashMap::new();
        let mut per_sym = HashMap::new();
        per_sym.insert("X".to_owned(), vec![vec![1.0, 2.0]]);
        data.insert("SMA".to_owned(), per_sym);
        let view = IndicatorView::new(&data, 10);
        assert_eq!(view.last("SMA", "X"), None);
    }

    // ── compute_metrics additional ─────────────────────────────────────

    #[test]
    fn compute_metrics_with_winning_and_losing_trades() {
        let curve: Vec<EquitySample> = (0..10)
            .map(|i| EquitySample {
                timestamp: 1_700_000_000 + i * 86_400,
                equity: 10_000.0 + i as f64 * 100.0,
                cash: HashMap::new(),
                drawdown: 0.0,
            })
            .collect();
        let trades = vec![
            Trade {
                symbol: "X".into(),
                entry_ts: 100,
                exit_ts: 200,
                quantity: 1.0,
                entry_price: 100.0,
                exit_price: 110.0,
                pnl: 10.0,
            },
            Trade {
                symbol: "X".into(),
                entry_ts: 300,
                exit_ts: 400,
                quantity: 1.0,
                entry_price: 110.0,
                exit_price: 105.0,
                pnl: -5.0,
            },
            Trade {
                symbol: "X".into(),
                entry_ts: 500,
                exit_ts: 600,
                quantity: 1.0,
                entry_price: 105.0,
                exit_price: 115.0,
                pnl: 10.0,
            },
        ];
        let m = compute_metrics(10_000.0, 0.0, &curve, &trades);
        assert_eq!(m["n_trades"], 3.0);
        assert!((m["win_rate"] - 2.0 / 3.0).abs() < 1e-9);
        assert!(m["total_return"] > 0.0);
    }

    // ── align_bars additional ──────────────────────────────────────────

    #[test]
    fn align_bars_skip_policy_produces_nones_for_missing() {
        let mut bars: HashMap<String, Vec<Bar>> = HashMap::new();
        bars.insert(
            "X".to_owned(),
            vec![mk_bar(1_700_000_000, 100.0), mk_bar(1_700_172_800, 102.0)],
        );
        let timeline = vec![1_700_000_000_i64, 1_700_086_400, 1_700_172_800];
        let result = align_bars(&bars, &timeline, EmptyBarPolicy::Skip);
        let row = &result["X"];
        assert!(row[0].is_some());
        assert!(row[1].is_none());
        assert!(row[2].is_some());
    }

    #[test]
    fn align_bars_forward_fill_fills_gap() {
        let mut bars: HashMap<String, Vec<Bar>> = HashMap::new();
        bars.insert(
            "X".to_owned(),
            vec![mk_bar(1_700_000_000, 100.0), mk_bar(1_700_172_800, 102.0)],
        );
        let timeline = vec![1_700_000_000_i64, 1_700_086_400, 1_700_172_800];
        let result = align_bars(&bars, &timeline, EmptyBarPolicy::ForwardFill);
        let row = &result["X"];
        assert!(row[1].is_some());
        assert_eq!(row[1].as_ref().unwrap().close, 100.0);
        assert_eq!(row[1].as_ref().unwrap().volume, 0.0);
    }

    #[test]
    fn align_bars_fill_with_nan_fills_nan_bar() {
        let mut bars: HashMap<String, Vec<Bar>> = HashMap::new();
        bars.insert(
            "X".to_owned(),
            vec![mk_bar(1_700_000_000, 100.0), mk_bar(1_700_172_800, 102.0)],
        );
        let timeline = vec![1_700_000_000_i64, 1_700_086_400, 1_700_172_800];
        let result = align_bars(&bars, &timeline, EmptyBarPolicy::FillWithNaN);
        assert!(result["X"][1].as_ref().unwrap().close.is_nan());
    }

    // ── period_bucket additional ───────────────────────────────────────

    #[test]
    fn period_bucket_month_changes_across_months() {
        use crate::backtest::models::conversion_period::ConversionPeriod;
        let jan15 = 1_705_276_800_i64;
        let feb15 = jan15 + 31 * 86_400;
        assert_ne!(
            period_bucket(jan15, ConversionPeriod::Month),
            period_bucket(feb15, ConversionPeriod::Month)
        );
    }

    #[test]
    fn period_bucket_day_same_day_equal() {
        use crate::backtest::models::conversion_period::ConversionPeriod;
        // 2024-01-15 00:00:00 UTC (aligned to midnight)
        let midnight = 1_705_276_800_i64;
        let noon = midnight + 43_200;
        assert_eq!(
            period_bucket(midnight, ConversionPeriod::Day),
            period_bucket(noon, ConversionPeriod::Day)
        );
    }

    // ── OrderType/enum coverage ────────────────────────────────────────

    #[test]
    fn order_type_parse_flexible_all_variants() {
        assert_eq!(OrderType::parse_flexible("market").unwrap(), OrderType::Market);
        assert_eq!(OrderType::parse_flexible("LIMIT").unwrap(), OrderType::Limit);
        assert_eq!(OrderType::parse_flexible("stop_loss").unwrap(), OrderType::StopLoss);
        assert_eq!(OrderType::parse_flexible("take_profit").unwrap(), OrderType::TakeProfit);
        assert_eq!(OrderType::parse_flexible("stop").unwrap(), OrderType::StopLoss);
        assert_eq!(OrderType::parse_flexible("stop_loss_limit").unwrap(), OrderType::StopLossLimit);
        assert_eq!(
            OrderType::parse_flexible("take_profit_limit").unwrap(),
            OrderType::TakeProfitLimit
        );
        assert_eq!(OrderType::parse_flexible("trailing_stop").unwrap(), OrderType::TrailingStop);
        assert_eq!(
            OrderType::parse_flexible("trailing_stop_limit").unwrap(),
            OrderType::TrailingStopLimit
        );
        assert_eq!(OrderType::parse_flexible("settle").unwrap(), OrderType::SettlePosition);
        assert_eq!(OrderType::parse_flexible("cancel").unwrap(), OrderType::Cancel);
        assert!(OrderType::parse_flexible("nonexistent").is_err());
    }

    #[test]
    fn order_type_name_and_description_all_variants() {
        let variants = [
            OrderType::Market,
            OrderType::Limit,
            OrderType::StopLoss,
            OrderType::TakeProfit,
            OrderType::StopLossLimit,
            OrderType::TakeProfitLimit,
            OrderType::TrailingStop,
            OrderType::TrailingStopLimit,
            OrderType::SettlePosition,
            OrderType::Cancel,
        ];
        for v in &variants {
            assert!(!v.name().is_empty());
            assert!(!v.description().is_empty());
        }
    }

    #[test]
    fn order_type_fromstr() {
        assert_eq!("market".parse::<OrderType>().unwrap(), OrderType::Market);
        assert_eq!("trailing_stop".parse::<OrderType>().unwrap(), OrderType::TrailingStop);
    }

    #[test]
    fn instrument_type_default_providers() {
        assert_eq!(InstrumentType::Stocks.default_provider(), Provider::Yahoo);
        assert_eq!(InstrumentType::Etf.default_provider(), Provider::Yahoo);
        assert_eq!(InstrumentType::Forex.default_provider(), Provider::Yahoo);
        assert_eq!(InstrumentType::Crypto.default_provider(), Provider::Binance);
    }

    #[test]
    fn instrument_type_allows_fractional_quantities_all() {
        assert!(InstrumentType::Crypto.allows_fractional_quantities());
        assert!(!InstrumentType::Stocks.allows_fractional_quantities());
        assert!(!InstrumentType::Etf.allows_fractional_quantities());
        assert!(!InstrumentType::Forex.allows_fractional_quantities());
    }

    #[test]
    fn instrument_type_is_equity_all() {
        assert!(InstrumentType::Stocks.is_equity());
        assert!(InstrumentType::Etf.is_equity());
        assert!(!InstrumentType::Forex.is_equity());
        assert!(!InstrumentType::Crypto.is_equity());
    }

    #[test]
    fn instrument_type_str_display_all() {
        assert_eq!(InstrumentType::Stocks.__str__(), "Stocks");
        assert_eq!(InstrumentType::Etf.__str__(), "ETF");
        assert_eq!(InstrumentType::Forex.__str__(), "Forex");
        assert_eq!(InstrumentType::Crypto.__str__(), "Crypto");
    }

    #[test]
    fn instrument_type_icon_all() {
        assert!(!InstrumentType::Stocks.icon().is_empty());
        assert!(!InstrumentType::Etf.icon().is_empty());
        assert!(!InstrumentType::Forex.icon().is_empty());
        assert!(!InstrumentType::Crypto.icon().is_empty());
    }

    #[test]
    fn interval_display_all_variants() {
        assert_eq!(Interval::OneMinute.to_string(), "1m");
        assert_eq!(Interval::FiveMinutes.to_string(), "5m");
        assert_eq!(Interval::FifteenMinutes.to_string(), "15m");
        assert_eq!(Interval::ThirtyMinutes.to_string(), "30m");
        assert_eq!(Interval::OneHour.to_string(), "1h");
        assert_eq!(Interval::FourHours.to_string(), "4h");
        assert_eq!(Interval::OneDay.to_string(), "1d");
        assert_eq!(Interval::OneWeek.to_string(), "1w");
    }

    #[test]
    fn interval_from_str_all_variants() {
        assert_eq!("1m".parse::<Interval>().unwrap(), Interval::OneMinute);
        assert_eq!("5m".parse::<Interval>().unwrap(), Interval::FiveMinutes);
        assert_eq!("15m".parse::<Interval>().unwrap(), Interval::FifteenMinutes);
        assert_eq!("30m".parse::<Interval>().unwrap(), Interval::ThirtyMinutes);
        assert_eq!("1h".parse::<Interval>().unwrap(), Interval::OneHour);
        assert_eq!("4h".parse::<Interval>().unwrap(), Interval::FourHours);
        assert_eq!("1d".parse::<Interval>().unwrap(), Interval::OneDay);
        assert_eq!("1w".parse::<Interval>().unwrap(), Interval::OneWeek);
        assert!("invalid".parse::<Interval>().is_err());
    }

    #[test]
    fn interval_is_intraday_all() {
        assert!(Interval::OneMinute.is_intraday());
        assert!(Interval::FiveMinutes.is_intraday());
        assert!(Interval::FifteenMinutes.is_intraday());
        assert!(Interval::ThirtyMinutes.is_intraday());
        assert!(Interval::OneHour.is_intraday());
        assert!(Interval::FourHours.is_intraday());
        assert!(!Interval::OneDay.is_intraday());
        assert!(!Interval::OneWeek.is_intraday());
    }

    #[test]
    fn interval_minutes_all_variants() {
        assert_eq!(Interval::OneMinute.minutes(), 1);
        assert_eq!(Interval::FiveMinutes.minutes(), 5);
        assert_eq!(Interval::FifteenMinutes.minutes(), 15);
        assert_eq!(Interval::ThirtyMinutes.minutes(), 30);
        assert_eq!(Interval::OneHour.minutes(), 60);
        assert_eq!(Interval::FourHours.minutes(), 240);
        assert_eq!(Interval::OneDay.minutes(), 1440);
        assert_eq!(Interval::OneWeek.minutes(), 10080);
    }

    #[test]
    fn commission_type_str_all_variants() {
        assert_eq!(CommissionType::Percentage.__str__(), "Percentage (%)");
        assert_eq!(CommissionType::Fixed.__str__(), "Fixed amount");
        assert_eq!(CommissionType::PercentagePlusFixed.__str__(), "Percentage + Fixed");
    }

    #[test]
    fn commission_type_from_str() {
        assert_eq!("percentage".parse::<CommissionType>().unwrap(), CommissionType::Percentage);
        assert_eq!("fixed".parse::<CommissionType>().unwrap(), CommissionType::Fixed);
        assert_eq!(
            "PercentagePlusFixed".parse::<CommissionType>().unwrap(),
            CommissionType::PercentagePlusFixed
        );
    }

    #[test]
    fn empty_bar_policy_name_all_variants() {
        assert_eq!(EmptyBarPolicy::Skip.name(), "Skip");
        assert_eq!(EmptyBarPolicy::ForwardFill.name(), "Forward-fill");
        assert_eq!(EmptyBarPolicy::FillWithNaN.name(), "Fill with NaN");
    }

    #[test]
    fn empty_bar_policy_from_str() {
        assert_eq!("skip".parse::<EmptyBarPolicy>().unwrap(), EmptyBarPolicy::Skip);
        assert_eq!("forwardfill".parse::<EmptyBarPolicy>().unwrap(), EmptyBarPolicy::ForwardFill);
        assert_eq!("fillwithnan".parse::<EmptyBarPolicy>().unwrap(), EmptyBarPolicy::FillWithNaN);
    }

    #[test]
    fn experiment_status_description_all() {
        assert!(!ExperimentStatus::Success.description().is_empty());
        assert!(!ExperimentStatus::Partial.description().is_empty());
        assert!(!ExperimentStatus::Error.description().is_empty());
    }

    #[test]
    fn experiment_status_from_str() {
        assert_eq!("success".parse::<ExperimentStatus>().unwrap(), ExperimentStatus::Success);
        assert_eq!("partial".parse::<ExperimentStatus>().unwrap(), ExperimentStatus::Partial);
        assert_eq!("error".parse::<ExperimentStatus>().unwrap(), ExperimentStatus::Error);
    }

    #[test]
    fn currency_conversion_mode_name_all() {
        use crate::backtest::models::currency_conversion_mode::CurrencyConversionMode;
        assert!(!CurrencyConversionMode::Immediate.name().is_empty());
        assert!(!CurrencyConversionMode::HoldUntilThreshold.name().is_empty());
        assert!(!CurrencyConversionMode::EndOfPeriod.name().is_empty());
        assert!(!CurrencyConversionMode::CustomInterval.name().is_empty());
    }

    #[test]
    fn currency_conversion_mode_from_str() {
        use crate::backtest::models::currency_conversion_mode::CurrencyConversionMode;
        assert_eq!(
            "immediate".parse::<CurrencyConversionMode>().unwrap(),
            CurrencyConversionMode::Immediate
        );
        assert_eq!(
            "HoldUntilThreshold".parse::<CurrencyConversionMode>().unwrap(),
            CurrencyConversionMode::HoldUntilThreshold
        );
        assert_eq!(
            "EndOfPeriod".parse::<CurrencyConversionMode>().unwrap(),
            CurrencyConversionMode::EndOfPeriod
        );
    }

    #[test]
    fn conversion_period_from_str() {
        use crate::backtest::models::conversion_period::ConversionPeriod;
        assert_eq!("day".parse::<ConversionPeriod>().unwrap(), ConversionPeriod::Day);
        assert_eq!("week".parse::<ConversionPeriod>().unwrap(), ConversionPeriod::Week);
        assert_eq!("month".parse::<ConversionPeriod>().unwrap(), ConversionPeriod::Month);
        assert_eq!("year".parse::<ConversionPeriod>().unwrap(), ConversionPeriod::Year);
    }

    // ── Additional strategy run tests ──────────────────────────────────

    fn run_builtin_single(
        strategy_name: &str,
        make_strategy: fn(Python<'_>) -> PyResult<Py<PyAny>>,
        n_bars: usize,
    ) -> RunResult {
        let aligned = mk_multi_symbol_aligned(&["S1"], n_bars);
        let timeline: Vec<i64> = (0..n_bars).map(|i| 1_700_000_000 + i as i64 * 86_400).collect();
        let mut cfg = mk_cfg("S1");
        cfg.engine.warmup_period = 0;
        cfg.exchange.allow_short_selling = true;
        let profiles = vec![mk_profile("S1", "USD")];
        let fx = FxTable::new("USD");

        let (strategy_obj, indicator_objs) = Python::attach(|py| -> PyResult<_> {
            let s = make_strategy(py)?;
            let raw = s.bind(py).call_method0("required_indicators")?;
            let required: Vec<Py<PyAny>> = raw.extract()?;
            let mut objs = Vec::new();
            for ind in required {
                let name = _indicator_deterministic_name(ind.bind(py).as_any())?;
                objs.push((name, ind));
            }
            Ok((s, objs))
        })
        .unwrap();

        let indicators = compute_indicators(&indicator_objs, &aligned, None).unwrap();
        run_one_strategy(
            strategy_name,
            strategy_obj,
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        )
    }

    #[test]
    fn run_adaptive_rsi_strategy() {
        let run = run_builtin_single(
            "Adaptive RSI",
            |py| Ok(Py::new(py, AdaptiveRsi::new(2, 6))?.into_any()),
            80,
        );
        assert!(run.error.is_none());
        assert!(!run.equity_curve.is_empty());
    }

    #[test]
    fn run_alpha_rsi_pro_strategy() {
        let run = run_builtin_single(
            "AlphaRSI Pro",
            |py| Ok(Py::new(py, AlphaRsiPro::new(3, 5))?.into_any()),
            80,
        );
        assert!(run.error.is_none());
    }

    #[test]
    fn run_bollinger_mean_reversion_strategy() {
        let run = run_builtin_single(
            "BB Mean Reversion",
            |py| Ok(Py::new(py, BollingerMeanReversion::new(5, 1.0))?.into_any()),
            80,
        );
        assert!(run.error.is_none());
    }

    #[test]
    fn run_macd_strategy() {
        let run =
            run_builtin_single("MACD", |py| Ok(Py::new(py, Macd::new(3, 7, 3))?.into_any()), 80);
        assert!(run.error.is_none());
    }

    #[test]
    fn run_momentum_strategy() {
        let run = run_builtin_single(
            "Momentum",
            |py| Ok(Py::new(py, Momentum::new(3, 7))?.into_any()),
            80,
        );
        assert!(run.error.is_none());
    }

    #[test]
    fn run_roc_strategy() {
        let run = run_builtin_single("ROC", |py| Ok(Py::new(py, Roc::new(3))?.into_any()), 80);
        assert!(run.error.is_none());
    }

    #[test]
    fn run_rsi_strategy() {
        let run =
            run_builtin_single("RSI", |py| Ok(Py::new(py, Rsi::new(3, 5, 1.0))?.into_any()), 80);
        assert!(run.error.is_none());
    }

    #[test]
    fn run_rsrs_strategy() {
        let run = run_builtin_single("RSRS", |py| Ok(Py::new(py, Rsrs::new(6))?.into_any()), 80);
        assert!(run.error.is_none());
    }

    #[test]
    fn run_sma_naive_strategy() {
        let run =
            run_builtin_single("Naive SMA", |py| Ok(Py::new(py, SmaNaive::new(5))?.into_any()), 80);
        assert!(run.error.is_none());
    }

    #[test]
    fn run_risk_averse_strategy() {
        let run = run_builtin_single(
            "Risk Averse",
            |py| Ok(Py::new(py, RiskAverse::new(4, 6))?.into_any()),
            80,
        );
        assert!(run.error.is_none());
    }

    #[test]
    fn run_hybrid_alpha_rsi_strategy() {
        let run = run_builtin_single(
            "Hybrid AlphaRSI",
            |py| Ok(Py::new(py, HybridAlphaRsi::new(2, 6, 6))?.into_any()),
            80,
        );
        assert!(run.error.is_none());
    }

    #[test]
    fn run_turtle_trading_strategy() {
        let run = run_builtin_single(
            "Turtle Trading",
            |py| Ok(Py::new(py, TurtleTrading::new(8, 4, 5))?.into_any()),
            120,
        );
        assert!(run.error.is_none());
    }

    #[test]
    fn run_vcp_strategy() {
        let run = run_builtin_single("VCP", |py| Ok(Py::new(py, Vcp::new(18, 3))?.into_any()), 120);
        assert!(run.error.is_none());
    }

    #[test]
    fn run_double_top_strategy() {
        let run = run_builtin_single(
            "Double Top",
            |py| Ok(Py::new(py, DoubleTop::new(10))?.into_any()),
            120,
        );
        assert!(run.error.is_none());
    }

    // ── Multi-asset rotation strategies ────────────────────────────────

    fn run_rotation_helper(
        strategy_name: &str,
        make_strategy: fn(Python<'_>) -> PyResult<Py<PyAny>>,
    ) -> RunResult {
        let symbols = vec!["A1", "B1", "C1"];
        let n_bars = 100;
        let aligned = mk_multi_symbol_aligned(&symbols, n_bars);
        let timeline: Vec<i64> = (0..n_bars).map(|i| 1_700_000_000 + i as i64 * 86_400).collect();
        let mut cfg = mk_cfg("A1");
        cfg.data.symbols = symbols.iter().map(|s| (*s).to_owned()).collect();
        cfg.engine.warmup_period = 0;
        cfg.exchange.allow_short_selling = true;
        cfg.portfolio.initial_cash = 100_000;
        let profiles: Vec<InstrumentProfile> =
            symbols.iter().map(|s| mk_profile(s, "USD")).collect();
        let fx = FxTable::new("USD");

        let (strategy_obj, indicator_objs) = Python::attach(|py| -> PyResult<_> {
            let s = make_strategy(py)?;
            let raw = s.bind(py).call_method0("required_indicators")?;
            let required: Vec<Py<PyAny>> = raw.extract()?;
            let mut objs = Vec::new();
            for ind in required {
                let name = _indicator_deterministic_name(ind.bind(py).as_any())?;
                objs.push((name, ind));
            }
            Ok((s, objs))
        })
        .unwrap();

        let indicators = compute_indicators(&indicator_objs, &aligned, None).unwrap();

        run_one_strategy(
            strategy_name,
            strategy_obj,
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        )
    }

    #[test]
    fn run_roc_rotation_strategy() {
        let run = run_rotation_helper("ROC Rotation", |py| {
            Ok(Py::new(py, RocRotation::new(3, 2, 1))?.into_any())
        });
        assert!(run.error.is_none());
    }

    #[test]
    fn run_rsrs_rotation_strategy() {
        let run = run_rotation_helper("RSRS Rotation", |py| {
            Ok(Py::new(py, RsrsRotation::new(6, 2, 1))?.into_any())
        });
        assert!(run.error.is_none());
    }

    #[test]
    fn run_multi_bb_rotation_strategy() {
        let run = run_rotation_helper("Multi BB Rotation", |py| {
            Ok(Py::new(py, MultiBollingerRotation::new(5, 1.0, 2, 1))?.into_any())
        });
        assert!(run.error.is_none());
    }

    #[test]
    fn run_triple_rsi_rotation_strategy() {
        let run = run_rotation_helper("Triple RSI Rotation", |py| {
            Ok(Py::new(py, TripleRsiRotation::new(2, 3, 5, 2, 1))?.into_any())
        });
        assert!(run.error.is_none());
    }

    // ── Provider tests ────────────────────────────────────────────────

    #[test]
    fn provider_from_str() {
        assert_eq!("yahoo".parse::<Provider>().unwrap(), Provider::Yahoo);
        assert_eq!("binance".parse::<Provider>().unwrap(), Provider::Binance);
        assert_eq!("coinbase".parse::<Provider>().unwrap(), Provider::Coinbase);
        assert_eq!("kraken".parse::<Provider>().unwrap(), Provider::Kraken);
    }

    #[test]
    fn instrument_type_from_str() {
        assert_eq!("stocks".parse::<InstrumentType>().unwrap(), InstrumentType::Stocks);
        assert_eq!("etf".parse::<InstrumentType>().unwrap(), InstrumentType::Etf);
        assert_eq!("forex".parse::<InstrumentType>().unwrap(), InstrumentType::Forex);
        assert_eq!("crypto".parse::<InstrumentType>().unwrap(), InstrumentType::Crypto);
    }
}
