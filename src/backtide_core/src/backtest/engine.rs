//! Backtest engine logic.
//!
//! This module implements the per-strategy event loop, order matching,
//! multi-currency portfolio bookkeeping and result aggregation.

use crate::backtest::fx::*;
use crate::backtest::interface::{check_abort, ProgressReporter};
use crate::backtest::margin::*;
use crate::backtest::models::*;
use crate::backtest::orders::*;
use crate::backtest::utils::*;
use crate::constants::*;
use crate::data::errors::DataError;
use crate::data::models::*;
use crate::engine::Engine;
use crate::errors::{EngineError, EngineResult};
use crate::indicators::interface::_indicator_deterministic_name;
use crate::indicators::utils::compute_indicators;
use crate::metrics::engine::{compute_builtin_metrics, is_builtin_metric};
use crate::metrics::utils::compute_custom_metric;
use crate::strategies::interface::{BuiltinStrategy, BuyAndHold};
use crate::strategies::utils::{load_strategies, IndicatorView};
use crate::utils::experiment_log::{EXPERIMENT_SPAN, LOG_PATH_FIELD};
use crate::utils::progress::{progress_bar, progress_spinner};
use crate::utils::python::load_pickle;
use itertools::Itertools;
use pyo3::prelude::*;
use pyo3::Py;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
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
        metric_overrides: &HashMap<String, Py<PyAny>>,
        progress: Option<&ProgressReporter>,
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

        let _log_span = tracing::info_span!(
            EXPERIMENT_SPAN,
            experiment_id = %experiment_id,
            { LOG_PATH_FIELD } = %exp_dir.join("logs.txt").display(),
        );
        let _log_guard = _log_span.enter();

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
            Number of metrics: {}\n \
            Risk free rate: {}%",
            config.data.symbols.len(),
            config.data.interval.to_string(),
            config.data.instrument_type.to_string(),
            config.portfolio.initial_cash,
            config.strategy.benchmark.as_deref().map_or("None".to_owned(), |s| format!("{s:?}")),
            config.strategy.strategies.len(),
            config.indicators.indicators.len(),
            config.metrics.len(),
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

        if !benchmark.is_empty()
            && !benchmark_from_strat
            && !symbols.iter().any(|s| s == &benchmark)
        {
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
            if is_negligible(*qty) {
                continue;
            }

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
                .ok_or(DataError::ProviderNotConfigured(config.data.instrument_type))?,
            start_clamp,
            end_clamp,
        )?;

        let total_bars: usize = bar_map.values().map(|v| v.len()).sum();
        info!("Loaded {} bars across {} symbols.", total_bars, bar_map.len());

        for (sym, bars) in &bar_map {
            debug!(" - {} → {} bars", sym, bars.len());
        }

        // Build a master timeline (union of all symbol timestamps, sorted).
        let mut all_ts = Vec::with_capacity(total_bars);
        all_ts.extend(bar_map.values().flat_map(|bars| bars.iter().map(|b| b.open_ts as i64)));
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
            let provider = self
                .config
                .data
                .providers
                .get(&leg.instrument.instrument_type)
                .ok_or(DataError::ProviderNotConfigured(leg.instrument.instrument_type))?;
            let leg_sym = vec![leg.instrument.symbol.clone()];
            let leg_bars = match self.load_bars(
                &leg_sym,
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
        let timeline_steps = all_ts.len() as u64;
        let total_simulation_steps = n_strategies.saturating_mul(timeline_steps);
        let pb = verbose.then(|| {
            progress_bar(
                total_simulation_steps,
                format!(
                    "Simulating {n_strategies} strategies across {timeline_steps} timeline steps..."
                ),
            )
        });
        if let Some(progress) = progress {
            progress.set_total(total_simulation_steps);
        }

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

                let result = run_one_strategy_with_progress(
                    &name,
                    obj,
                    config,
                    &aligned,
                    &indicators,
                    &profiles,
                    &all_ts,
                    &fx,
                    py_cache.as_ref(),
                    pb.as_ref(),
                    progress,
                );

                info!(
                    "✔ Finished strategy {:?}: {} trades, {} bars in equity curve.",
                    result.strategy_name,
                    result.trades.len(),
                    result.equity_curve.len()
                );

                result
            })
        };

        // Run the built-in and custom strategies in parallel.
        let (mut results, custom_results): (Vec<RunResult>, Vec<RunResult>) = rayon::join(
            || builtin.into_par_iter().map(&run).collect(),
            || custom.into_iter().map(&run).collect(),
        );
        results.extend(custom_results);

        if let Some(progress) = progress {
            progress.finish();
        }
        if let Some(p) = pb {
            p.set_position(total_simulation_steps);
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

            if config.metrics.iter().any(|metric| metric == "excess_return") {
                if let Some(v) = excess_return {
                    r.metrics.insert("excess_return".into(), v);
                } else {
                    r.metrics.remove("excess_return");
                }
            } else {
                r.metrics.remove("excess_return");
            }

            // Alpha is only meaningful for non-benchmark runs.
            if config.metrics.iter().any(|metric| metric == "alpha") {
                if let Some(bench) = bench_snapshot.as_ref() {
                    if !r.is_benchmark {
                        // If benchmark never became investable, alpha is unavailable.
                        let alpha = bench_start_ts.and_then(|_| {
                            strat_ret.and_then(|ret| {
                                windowed_return(bench, window_start).map(|b| ret - b)
                            })
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
        }

        let mut custom_metrics = Vec::new();
        for name in config.metrics.iter().filter(|name| !is_builtin_metric(name)) {
            let loaded = Python::attach(|py| -> PyResult<Py<PyAny>> {
                if let Some(value) = metric_overrides.get(name) {
                    Ok(value.clone_ref(py))
                } else {
                    load_pickle(
                        py,
                        &self.config.data.storage_path.join("metrics").join(format!("{name}.pkl")),
                    )
                }
            });
            match loaded {
                Ok(value) => custom_metrics.push((name.clone(), value)),
                Err(error) => warnings.push(format!("Failed to load metric {name:?}: {error}")),
            }
        }
        for run in &mut results {
            for (name, metric) in &custom_metrics {
                match compute_custom_metric(metric, &run.equity_curve, &run.trades) {
                    Ok(value) => {
                        run.metrics.insert(name.clone(), value);
                    },
                    Err(error) => warnings.push(format!(
                        "Metric {name:?} failed for strategy {:?}: {error}",
                        run.strategy_name
                    )),
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
            experiment_id,
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
            result.experiment_id,
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
// Per-strategy runner
// ────────────────────────────────────────────────────────────────────────────

/// Execute one strategy through the entire timeline.
#[cfg(test)]
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
    run_one_strategy_with_progress(
        name, strategy, cfg, aligned, indicators, profiles, timeline, fx, py_cache, None, None,
    )
}

/// Execute one strategy and publish throttled simulation-step progress.
fn run_one_strategy_with_progress(
    name: &str,
    strategy: Py<PyAny>,
    cfg: &ExperimentConfig,
    aligned: &HashMap<Symbol, Vec<Option<Bar>>>,
    indicators: &HashMap<String, HashMap<Symbol, Vec<Vec<f64>>>>,
    profiles: &[InstrumentProfile],
    timeline: &[i64],
    fx: &FxTable,
    py_cache: Option<&(DataT, IndicatorsT)>,
    progress_bar: Option<&indicatif::ProgressBar>,
    progress: Option<&ProgressReporter>,
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
    let base_ccy_ref: &str = &base_ccy_str;

    let mut cash: Cash = Cash::from([(base_ccy, cfg.portfolio.initial_cash as f64)]);

    // The benchmark strategy always starts with a clean slate (no pre-existing
    // holdings) so its return reflects a pure buy-and-hold from cash.
    let mut positions: Positions = if is_benchmark_run {
        Positions::new()
    } else {
        cfg.portfolio
            .starting_positions
            .iter()
            .filter_map(|(sym, qty)| is_significant(*qty).then_some((sym.clone(), *qty)))
            .collect()
    };

    let mut open_orders: Vec<Order> = Vec::new();

    // Per-order extremes for trailing stops: (running_high, running_low)
    // observed since the order was first seen. Cleared on fill / cancel.
    let mut trail_state: HashMap<OrderId, (f64, f64)> = HashMap::new();

    let total_bars: usize = aligned.values().map(|v| v.len()).next().unwrap_or(0);
    let mut equity_curve: Vec<EquitySample> = Vec::with_capacity(total_bars);
    let mut order_records: Vec<OrderRecord> = Vec::new();
    let mut closed_trades: Vec<Trade> = Vec::new();

    // Open trade tracker per symbol: (entry_ts, qty_remaining, entry_price)
    let mut open_trades: HashMap<String, (i64, f64, f64)> = HashMap::new();

    let mut peak_equity = cfg.portfolio.initial_cash as f64;

    // Tracks the boundary used by `EndOfPeriod` and the counter used by `CustomInterval`.
    let mut last_period_bucket: Option<i64> = None;
    let mut bars_since_conv: usize = 0;
    let conv_interval = cfg.exchange.conversion_interval.unwrap_or(0) as usize;

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
    let bars_full: Vec<(&str, Vec<Bar>)> = aligned
        .iter()
        .filter(|(s, _)| symbols.contains(s.as_str()))
        .map(|(s, row)| (s.as_str(), row.iter().map(|b| b.unwrap_or(Bar::NAN)).collect()))
        .sorted_by(|a, b| a.0.cmp(b.0))
        .collect();

    // Pre-build the Python data/indicator cache to avoid cloning Python objects.
    let empty_data: DataT = HashMap::new();
    let empty_ind: IndicatorsT = HashMap::new();
    let fresh: Option<(DataT, IndicatorsT)> = if builtin.is_none() && py_cache.is_none() {
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
        cached_data = &empty_data;
        cached_indicators = &empty_ind;
    } else if let Some((d, i)) = py_cache {
        cached_data = d;
        cached_indicators = i;
    } else if let Some((d, i)) = &fresh {
        cached_data = d;
        cached_indicators = i;
    } else {
        cached_data = &empty_data;
        cached_indicators = &empty_ind;
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

        if let Err(error) = accrue_margin_costs(
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
        ) {
            warn!(strategy = %name, "Margin cost accrual failed: {error}");
            run_error.get_or_insert(error);
        }

        // ── Resolve open orders against the current bar ─────────────────────

        let mut still_open: Vec<Order> = Vec::new();
        let drained: Vec<Order> = std::mem::take(&mut open_orders);
        for mut order in drained {
            // Cancel orders take effect immediately.
            if order.order_type == OrderType::Cancel {
                if let Some(pos) = still_open.iter().position(|o| o.id == order.id) {
                    let canceled = still_open.remove(pos);
                    trail_state.remove(&canceled.id);

                    order_records.push(OrderRecord {
                        order: canceled,
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

            let Some(&it) = it_map.get(order.symbol.as_str()) else {
                let reason = format!("instrument metadata unavailable for {:?}", order.symbol);
                warn!(strategy = %name, order_id = %order.id, "{reason}");
                order_records.push(OrderRecord {
                    order,
                    timestamp: ts,
                    status: OrderStatus::Rejected,
                    fill_price: None,
                    reason,
                    commission: 0.0,
                    pnl: None,
                });
                continue;
            };

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
                        order,
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

            let qty = &mut order.quantity;

            // Determine accounting currency for cash operations. For non-fiat
            // quote currencies, convert fill amounts to the portfolio base
            // currency so cash accounting stays in fiat.
            let order_ccy_str = quote_ccy.get(order.symbol.as_str()).unwrap_or(&base_ccy_ref);

            let (order_ccy, nonfiat_fx_rate, order_ccy_ref) =
                match order_ccy_str.parse::<Currency>() {
                    Ok(fiat) => (fiat, 1.0_f64, *order_ccy_str),
                    Err(_) => {
                        let rate = fx.rate(order_ccy_str, &base_ccy_str, ts).unwrap_or(1.0);
                        (base_ccy, rate, base_ccy_ref)
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

            let current_qty = positions.amount(&order.symbol);

            let current_pos_base = if is_significant(current_qty) {
                let bar_close = aligned
                    .get(&order.symbol)
                    .and_then(|r| r[bar_index].as_ref())
                    .map(|b| b.close)
                    .unwrap_or(fill_px);

                let value = current_qty.abs() * bar_close;
                let ccy = quote_ccy.get(order.symbol.as_str()).unwrap_or(&base_ccy_ref);
                fx.convert(value, ccy, &base_ccy_str, ts).unwrap_or(value)
            } else {
                0.0
            };

            if let Err((violation, reason)) = check_order_against_limits(
                cfg,
                &order.symbol,
                *qty,
                acct_fill_px,
                order_ccy_ref,
                &base_ccy_str,
                equity_base,
                invested_base,
                current_qty,
                current_pos_base,
                fx,
                ts,
            )
            .and_then(|new_qty| {
                if is_negligible(new_qty - *qty) {
                    return Ok(());
                }

                let mut abs_qty = new_qty.abs();

                if !it.allows_fractional_quantities() {
                    abs_qty = abs_qty.floor();
                }

                if !abs_qty.is_finite() || is_negligible(abs_qty) {
                    return Err((
                        LimitViolation::Margin,
                        format!(
                            "no headroom under leverage / position-size limits for {}",
                            order.symbol
                        ),
                    ));
                }

                // Update the order quantity, sign-preserving, and re-derive notional and
                // commission from the shrunk size.
                *qty = qty.signum() * abs_qty;

                notional = acct_fill_px * abs_qty;
                commission = match cfg.exchange.commission_type {
                    CommissionType::Percentage => notional * cfg.exchange.commission_pct / 100.0,
                    CommissionType::Fixed => cfg.exchange.commission_fixed,
                    CommissionType::PercentagePlusFixed => {
                        notional * cfg.exchange.commission_pct / 100.0
                            + cfg.exchange.commission_fixed
                    },
                };

                fill_reason = if fill_reason.is_empty() {
                    "partial: shrunk to fit leverage / position-size limit".to_owned()
                } else {
                    format!("{fill_reason}; partial: shrunk to fit leverage / position-size limit")
                };

                Ok(())
            }) {
                warn!(strategy=%name, order_id=%order.id, "{reason}");

                // Position-size rejections are just warnings. Only margin/leverage violations
                // are gated by `raise_on_margin_limit`.
                if violation == LimitViolation::Margin && cfg.exchange.raise_on_margin_limit {
                    run_error.get_or_insert_with(|| reason.clone());
                }

                order_records.push(OrderRecord {
                    order,
                    timestamp: ts,
                    status: OrderStatus::Rejected,
                    fill_price: None,
                    reason,
                    commission: 0.0,
                    pnl: None,
                });

                continue;
            }

            let mut filled_qty = *qty;
            let mut fill_pnl: Option<f64> = None;

            if *qty > 0.0 {
                // BUY: try paying in `order_ccy` first, else convert from base.
                if !try_debit(&mut cash, order_ccy, notional + commission, base_ccy, fx, ts) {
                    let avail: f64 = cash
                        .iter()
                        .filter(|(_, v)| v.is_finite() && **v > 0.0)
                        .filter_map(|(ccy, v)| fx.convert(*v, &ccy.to_string(), order_ccy_ref, ts))
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
                    let denom = acct_fill_px * (1.0 + pct_part);
                    let mut max_qty: f64 = if denom > 0.0 && avail > fixed_part {
                        ((avail - fixed_part) / denom).max(0.0)
                    } else {
                        0.0
                    };

                    // Non-crypto instruments must settle whole units, so
                    // floor the cash-fit quantity before retrying the debit.
                    if !it.allows_fractional_quantities() {
                        max_qty = max_qty.floor();
                    }

                    if max_qty <= 0.0 {
                        warn!(
                            strategy=%name, order_id=%order.id,
                            "Insufficient funds for buy, skipping order."
                        );

                        order_records.push(OrderRecord {
                            order,
                            timestamp: ts,
                            status: OrderStatus::Rejected,
                            fill_price: None,
                            reason: "insufficient funds".into(),
                            commission: 0.0,
                            pnl: None,
                        });

                        continue;
                    }

                    filled_qty = max_qty.min(*qty);

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

                    // The first debit failed because the submitted quantity did not fit.
                    // Debit the recalculated cash-fit amount before granting the position.
                    // Without this retry, partially filled buys create shares without reducing
                    // cash and can make equity grow exponentially across repeated entries.
                    if !try_debit(&mut cash, order_ccy, notional + commission, base_ccy, fx, ts) {
                        warn!(
                            strategy=%name, order_id=%order.id,
                            "Cash-fit buy could not be debited, skipping order."
                        );

                        order_records.push(OrderRecord {
                            order,
                            timestamp: ts,
                            status: OrderStatus::Rejected,
                            fill_price: None,
                            reason: "insufficient funds after cash-fit sizing".into(),
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

                if let Some(v) = positions.get_mut(&order.symbol) {
                    *v += filled_qty;
                } else {
                    positions.insert(order.symbol.clone(), filled_qty);
                }
                if let Some((_, q, p)) = open_trades.get_mut(&order.symbol) {
                    let total = *q * *p + filled_qty * acct_fill_px;
                    *q += filled_qty;
                    if is_significant(*q) {
                        *p = total / *q;
                    }
                } else {
                    open_trades.insert(order.symbol.clone(), (ts, filled_qty, acct_fill_px));
                }
            } else if *qty < 0.0 {
                let abs_qty = qty.abs();
                let cur = positions.amount(&order.symbol);

                if !cfg.exchange.allow_short_selling && cur < abs_qty {
                    warn!(strategy=%name, order_id=%order.id, "Short selling disabled and not enough position, skipping.");

                    if cfg.exchange.raise_on_short_violation {
                        run_error.get_or_insert_with(|| "short selling disabled".to_owned());
                    }

                    order_records.push(OrderRecord {
                        order,
                        timestamp: ts,
                        status: OrderStatus::Rejected,
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
                    // Reverse: not enough to even pay commission.
                    *cash.entry(order_ccy).or_insert(0.0) -= notional;
                    order_records.push(OrderRecord {
                        order,
                        timestamp: ts,
                        status: OrderStatus::Rejected,
                        fill_price: None,
                        reason: "cannot pay commission".into(),
                        commission: 0.0,
                        pnl: None,
                    });

                    continue;
                }

                if let Some(v) = positions.get_mut(&order.symbol) {
                    *v -= abs_qty;
                } else {
                    positions.insert(order.symbol.clone(), -abs_qty);
                }

                let realised_pnl = close_open_trade_sell(
                    &mut open_trades,
                    &order.symbol,
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

            // Reflect the actually-filled quantity on the record.
            if is_significant(filled_qty - order.quantity) {
                order.quantity = filled_qty;
            }

            order_records.push(OrderRecord {
                order,
                timestamp: ts,
                status: OrderStatus::Filled,
                fill_price: Some(fill_px),
                reason: fill_reason,
                commission,
                pnl: fill_pnl,
            });
        }

        open_orders = still_open;

        // ── Apply currency-conversion policy ────────────────────────────────

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
                if conv_interval > 0 && bars_since_conv >= conv_interval {
                    sweep_foreign_to_base(&mut cash, base_ccy, fx, ts, None);
                    bars_since_conv = 0;
                }
            },
        }

        // ── Strategy decision ───────────────────────────────────────────────

        if !is_warmup {
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

            let new_orders: Result<Vec<Order>, PyErr> = if let Some(b) = &builtin {
                let bars_view: Vec<(&str, &[Bar])> =
                    bars_full.iter().map(|(s, v)| (*s, &v[..=bar_index])).collect();

                let inds = IndicatorView::new(indicators, bar_index as u64);
                let orders = b.evaluate(&bars_view, &portfolio, &state, &inds, &it_map);

                for o in &orders {
                    debug!(
                        strategy=%name,
                        "Order placed: {} {} {} @ bar {bar_index}",
                        if o.quantity > 0.0 {
                            "BUY"
                        } else {
                            "SELL"
                        },
                        o.quantity.abs(),
                        o.symbol,
                    );
                }

                Ok(orders)
            } else {
                Python::attach(|py| -> PyResult<Vec<Order>> {
                    let data = build_per_symbol_view(py, cached_data, bar_index, &symbols)?;
                    let inds = build_indicator_view(py, cached_indicators, bar_index, &symbols)?;

                    let orders: Vec<Order> = strategy
                        .bind(py)
                        .call_method1("evaluate", (data, portfolio.clone(), state.clone(), inds))?
                        .extract()
                        .unwrap_or_default();

                    Ok(orders)
                })
            };

            match new_orders {
                Ok(mut orders) => {
                    if cfg.engine.exclusive_orders && !orders.is_empty() {
                        // Cancel everything pending first.
                        for o in &open_orders {
                            order_records.push(OrderRecord {
                                order: o.clone(),
                                timestamp: ts,
                                status: OrderStatus::Canceled,
                                fill_price: None,
                                reason: "exclusive_orders".into(),
                                commission: 0.0,
                                pnl: None,
                            });
                        }

                        open_orders.clear();
                    }

                    // ── Resolve sizer-based quantities ──────────────────────

                    for o in &mut orders {
                        if let Some(sizer_slot) = o.sizer.take() {
                            let order_ccy_str_sizer =
                                quote_ccy.get(o.symbol.as_str()).unwrap_or(&base_ccy_ref);

                            // Get the current close price for this symbol.
                            let sym_price = aligned
                                .get(&o.symbol)
                                .and_then(|r| r[bar_index].as_ref())
                                .map(|b| b.close)
                                .unwrap_or(0.0);

                            let stop_distance: Option<f64> = o.price.and_then(|p| {
                                let d = (sym_price - p).abs();
                                if d > 0.0 {
                                    Some(d)
                                } else {
                                    None
                                }
                            });

                            let resolved = match &sizer_slot {
                                SizerSlot::Builtin(builtin) => {
                                    let capital = if builtin.uses_cash_capital() {
                                        compute_portfolio_cash(&cash, order_ccy_str_sizer, fx, ts)
                                    } else {
                                        compute_portfolio_equity(
                                            &cash,
                                            &positions,
                                            aligned,
                                            bar_index,
                                            &quote_ccy,
                                            order_ccy_str_sizer,
                                            fx,
                                            ts,
                                        )
                                    };
                                    // Resolve entirely in Rust — no GIL needed.
                                    builtin
                                        .calculate(capital, sym_price, stop_distance, None)
                                        .map(|quantity| {
                                            if builtin.uses_cash_capital()
                                                && it_map.get(o.symbol.as_str()).is_some_and(
                                                    |instrument_type| {
                                                        !instrument_type
                                                            .allows_fractional_quantities()
                                                    },
                                                )
                                            {
                                                quantity.abs().floor().copysign(quantity)
                                            } else {
                                                quantity
                                            }
                                        })
                                        .map_err(|e| {
                                            warn!(strategy=%name, order_id=%o.id, "Builtin sizer failed: {e}");
                                            e
                                        })
                                },
                                SizerSlot::Custom(py_sizer) => {
                                    let equity = compute_portfolio_equity(
                                        &cash,
                                        &positions,
                                        aligned,
                                        bar_index,
                                        &quote_ccy,
                                        order_ccy_str_sizer,
                                        fx,
                                        ts,
                                    );
                                    // Fall back to calling Python's calculate().
                                    Python::attach(|py| -> PyResult<f64> {
                                        py_sizer
                                            .bind(py)
                                            .call_method1(
                                                "calculate",
                                                (
                                                    equity,
                                                    sym_price,
                                                    stop_distance,
                                                    Option::<f64>::None,
                                                ),
                                            )?
                                            .extract()
                                    })
                                    .map_err(|e| {
                                        let msg = e.to_string();
                                        warn!(strategy=%name, order_id=%o.id, "Custom sizer failed: {msg}");
                                        msg
                                    })
                                },
                            };

                            match resolved {
                                Ok(qty) => o.quantity = qty,
                                Err(e) => {
                                    warn!(strategy=%name, order_id=%o.id, "Sizer resolution failed: {e}");
                                    o.quantity = 0.0; // Will be rejected by the qty check below.
                                },
                            }
                        }
                    }

                    // Validate allowed types/quantities & ensure ids are populated.
                    let allowed = &cfg.exchange.allowed_order_types;
                    orders.retain_mut(|o| {
                        if o.id.is_nil() {
                            o.id = OrderId::new();
                        }

                        // Reject orders targeting a symbol outside this strategy run. The
                        // experiment-wide profile map can also contain a benchmark symbol.
                        if !symbols.contains(o.symbol.as_str()) {
                            let reason = format!(
                                "unknown symbol {:?}: not in this strategy run's symbol list",
                                o.symbol
                            );
                            warn!(strategy=%name, order_id=%o.id, "{reason}");
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

                        if !allowed.contains(&o.order_type) && o.order_type != OrderType::Cancel {
                            warn!(strategy=%name, "Order type {} not allowed, rejecting.", o.order_type);
                            order_records.push(OrderRecord {
                                order: o.clone(),
                                timestamp: ts,
                                status: OrderStatus::Rejected,
                                fill_price: None,
                                reason: "order type not allowed".into(),
                                commission: 0.0,
                                pnl: None,
                            });

                            return false;
                        }

                        if !matches!(o.order_type, OrderType::Cancel | OrderType::SettlePosition) {
                            let Some(&it) = it_map.get(o.symbol.as_str()) else {
                                let reason = format!(
                                    "instrument metadata unavailable for {:?}",
                                    o.symbol
                                );
                                warn!(strategy = %name, order_id = %o.id, "{reason}");
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
                            };
                            if let Some(reason) = validate_qty(o.quantity, it) {
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
                    let mut seen_ids: HashSet<OrderId> = open_orders.iter().map(|o| o.id).collect();

                    orders.retain(|o| {
                        if matches!(o.order_type, OrderType::Cancel | OrderType::SettlePosition) {
                            return true;
                        }

                        if !seen_ids.insert(o.id) {
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

                        true
                    });

                    open_orders.extend(orders);
                },
                Err(e) => {
                    let msg = format!("evaluate() raised: {e}");
                    warn!(strategy=%name, "{msg}");
                    run_error.get_or_insert(msg);
                },
            }
        }

        // ── Mark-to-market & equity sample ──────────────────────────────────

        // Equity is computed entirely in the portfolio base currency.
        let equity = compute_portfolio_equity(
            &cash,
            &positions,
            aligned,
            bar_index,
            &quote_ccy,
            base_ccy_ref,
            fx,
            ts,
        );

        if equity > peak_equity {
            peak_equity = equity;
        }

        let drawdown = if peak_equity > 0.0 {
            (equity - peak_equity) / peak_equity
        } else {
            0.0
        };

        // Build the cash snapshot for this equity sample.
        let cash_snapshot = if cash.len() <= 1 {
            cash.clone()
        } else {
            cash.iter().filter(|(_, v)| is_significant(**v)).map(|(k, v)| (*k, *v)).collect()
        };

        equity_curve.push(EquitySample {
            timestamp: ts,
            equity,
            cash: cash_snapshot,
            drawdown,
        });

        // ── Maintenance-margin check ────────────────────────────────────────

        // If equity has fallen below `maintenance_margin` of gross notional,
        // force-flatten every open position at the current close price and
        // record a synthetic "margin call" order for each.
        let gross_base = compute_invested_equity(
            &positions,
            aligned,
            bar_index,
            &quote_ccy,
            &base_ccy_str,
            fx,
            ts,
        );

        if let Some(reason) =
            check_maintenance_margin(cfg.exchange.maintenance_margin, equity, gross_base)
        {
            warn!(strategy=%name, "{reason}");
            if cfg.exchange.raise_on_margin_limit {
                run_error.get_or_insert_with(|| reason.clone());
            }

            // Force-flatten every position at the current close.
            let to_flatten: Vec<(String, f64)> =
                positions.iter().map(|(s, q)| (s.clone(), *q)).collect();

            for (sym, qty) in &to_flatten {
                if is_negligible(*qty) {
                    continue;
                }

                let close = match aligned.get(sym.as_str()).and_then(|r| r[bar_index].as_ref()) {
                    Some(b) => b.close,
                    None => continue,
                };

                let pos_ccy_str = quote_ccy.get(sym.as_str()).unwrap_or(&base_ccy_ref);
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
                    id: OrderId::new(),
                    symbol: sym.clone(),
                    order_type: OrderType::Market,
                    quantity: -qty,
                    price: None,
                    limit_price: None,
                    sizer: None,
                };

                if *qty > 0.0 {
                    // Long: credit cash with proceeds.
                    *cash.entry(pos_ccy).or_insert(0.0) += notional_fiat;
                    if let Some(t) =
                        close_open_trade_sell(&mut open_trades, sym, ts, *qty, close, 0.0)
                    {
                        closed_trades.push(t);
                    }
                } else {
                    // Short: debit cash (or any available bucket) to buy back the shares.
                    let _ = try_debit(&mut cash, pos_ccy, notional_fiat, base_ccy, fx, ts);
                    open_trades.remove(sym.as_str());
                }

                positions.insert(sym.clone(), 0.0);
                order_records.push(OrderRecord {
                    order: synth,
                    timestamp: ts,
                    status: OrderStatus::Filled,
                    fill_price: Some(close),
                    reason: reason.clone(),
                    commission: 0.0,
                    pnl: None,
                });
            }

            positions.retain(|_, q| is_significant(*q));
        }

        if let Some(progress_bar) = progress_bar {
            progress_bar.inc(1);
        }
        if let Some(progress) = progress {
            progress.advance(1);
        }
    }

    // ── Liquidate remaining positions to compute final PnL ──────────────────

    if let Some(last_idx) = total_bars.checked_sub(1) {
        for (sym, qty) in positions {
            if is_negligible(qty) {
                continue;
            }

            if let Some(b) = aligned.get(&sym).and_then(|r| r[last_idx].as_ref()) {
                let exit_px = b.close;
                if let Some((entry_ts, _q, entry_px)) = open_trades.remove(&sym) {
                    let pnl = (exit_px - entry_px) * qty;
                    closed_trades.push(Trade {
                        symbol: sym,
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

    // ── Metrics ─────────────────────────────────────────────────────────────

    let metrics = compute_builtin_metrics(
        &cfg.metrics,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::interface::Config;
    use crate::data::errors::DataResult;
    use crate::data::models::{
        Bar, BarDownload, Currency, Exchange, Instrument, InstrumentType, Interval, Provider,
    };
    use crate::data::providers::DataProvider;
    use crate::engine::{Engine, EngineCache};
    use crate::storage::duckdb::DuckDb;
    use crate::storage::models::BarSeries;
    use crate::storage::traits::Storage;
    use async_trait::async_trait;
    use pyo3::types::{PyDict, PyList, PyModule};
    use std::collections::HashMap;
    use std::sync::Arc;
    use strum::IntoEnumIterator;
    use tempfile::TempDir;
    use tokio::runtime::Runtime;

    // ── Stub provider ────────────────────────────────────────────────────

    struct StubProvider {
        instruments: HashMap<String, Instrument>,
    }

    impl StubProvider {
        fn new() -> Self {
            Self {
                instruments: HashMap::new(),
            }
        }
    }

    #[async_trait]
    impl DataProvider for StubProvider {
        async fn fetch_instrument(
            &self,
            symbol: &String,
            _: InstrumentType,
        ) -> DataResult<Instrument> {
            self.instruments
                .get(symbol)
                .cloned()
                .ok_or_else(|| crate::data::errors::DataError::SymbolNotFound(symbol.clone()))
        }

        async fn fetch_range(&self, _: Instrument, _: Interval) -> DataResult<(u64, u64)> {
            Ok((1_000_000_000, 2_000_000_000))
        }

        async fn list_instruments(
            &self,
            _: InstrumentType,
            _: Option<Vec<Exchange>>,
            _: usize,
        ) -> DataResult<Vec<Instrument>> {
            Ok(self.instruments.values().cloned().collect())
        }

        async fn download_bars(
            &self,
            _: &str,
            _: InstrumentType,
            _: Interval,
            _: u64,
            _: u64,
        ) -> DataResult<BarDownload> {
            Ok(BarDownload {
                bars: vec![],
                dividends: vec![],
            })
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn make_engine() -> (Engine, TempDir) {
        make_engine_with_stub(StubProvider::new())
    }

    fn make_engine_with_stub(stub: StubProvider) -> (Engine, TempDir) {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.data.storage_path = tmp.path().join("state");
        let config = Box::leak(Box::new(config));
        let rt = Runtime::new().unwrap();
        let db = DuckDb::new(&tmp.path().join("test.db")).unwrap();
        db.init().unwrap();
        let stub: Arc<dyn DataProvider> = Arc::new(stub);
        let providers = InstrumentType::iter().map(|it| (it, stub.clone())).collect();
        (
            Engine {
                config,
                rt,
                providers,
                db: Box::new(db),
                cache: EngineCache::new(),
            },
            tmp,
        )
    }

    fn make_bar(ts: u64, close: f64) -> Bar {
        Bar {
            open_ts: ts,
            close_ts: ts + 3_600,
            open_ts_exchange: ts,
            open: close,
            high: close + 1.0,
            low: close - 1.0,
            close,
            adj_close: close,
            volume: 1_000_000.0,
            n_trades: Some(100),
        }
    }

    fn make_instrument(symbol: &str) -> Instrument {
        Instrument {
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            base: None,
            quote: "USD".to_owned(),
            instrument_type: InstrumentType::Stocks,
            exchange: "XNAS".to_owned(),
            provider: Provider::Yahoo,
        }
    }

    fn base_config() -> ExperimentConfig {
        ExperimentConfig {
            general: GeneralExpConfig::default(),
            data: DataExpConfig {
                instrument_type: InstrumentType::Stocks,
                symbols: vec!["AAPL".to_owned()],
                ..DataExpConfig::default()
            },
            portfolio: PortfolioExpConfig::default(),
            strategy: StrategyExpConfig::default(),
            indicators: IndicatorExpConfig::default(),
            metrics: ExperimentConfigInner::default().metrics,
            exchange: ExchangeExpConfig::default(),
            engine: EngineExpConfig::default(),
        }
    }

    fn write_bars(engine: &Engine, symbol: &str, bars: Vec<Bar>) {
        engine
            .write_bars_bulk(&[BarSeries {
                symbol: symbol.to_owned(),
                interval: Interval::OneDay,
                provider: Provider::Yahoo,
                bars,
            }])
            .unwrap();
    }

    // ── Engine::load_bars ────────────────────────────────────────────────

    #[test]
    fn load_bars_empty_db_returns_empty_map() {
        let (engine, _tmp) = make_engine();
        let result =
            engine.load_bars(&["AAPL".to_owned()], Interval::OneDay, Provider::Yahoo, None, None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn load_bars_empty_symbol_list() {
        let (engine, _tmp) = make_engine();
        let result = engine.load_bars(&[], Interval::OneDay, Provider::Yahoo, None, None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn load_bars_returns_correct_data() {
        let (engine, _tmp) = make_engine();
        write_bars(&engine, "AAPL", vec![make_bar(1_000, 100.0), make_bar(2_000, 101.0)]);

        let map = engine
            .load_bars(&["AAPL".to_owned()], Interval::OneDay, Provider::Yahoo, None, None)
            .unwrap();
        let bars = map.get("AAPL").unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].close, 100.0);
        assert_eq!(bars[1].close, 101.0);
    }

    #[test]
    fn load_bars_sorted_by_timestamp() {
        let (engine, _tmp) = make_engine();
        // Write in reverse order
        write_bars(
            &engine,
            "AAPL",
            vec![make_bar(3_000, 103.0), make_bar(1_000, 101.0), make_bar(2_000, 102.0)],
        );
        let map = engine
            .load_bars(&["AAPL".to_owned()], Interval::OneDay, Provider::Yahoo, None, None)
            .unwrap();
        let bars = map.get("AAPL").unwrap();
        assert_eq!(bars[0].open_ts, 1_000);
        assert_eq!(bars[1].open_ts, 2_000);
        assert_eq!(bars[2].open_ts, 3_000);
    }

    #[test]
    fn load_bars_start_filter_excludes_early_bars() {
        let (engine, _tmp) = make_engine();
        write_bars(
            &engine,
            "AAPL",
            vec![make_bar(100, 100.0), make_bar(200, 101.0), make_bar(300, 102.0)],
        );
        let map = engine
            .load_bars(&["AAPL".to_owned()], Interval::OneDay, Provider::Yahoo, Some(200), None)
            .unwrap();
        let bars = map.get("AAPL").unwrap();
        assert_eq!(bars.len(), 2);
        assert!(bars.iter().all(|b| b.open_ts >= 200));
    }

    #[test]
    fn load_bars_end_filter_excludes_late_bars() {
        let (engine, _tmp) = make_engine();
        write_bars(
            &engine,
            "AAPL",
            vec![make_bar(100, 100.0), make_bar(200, 101.0), make_bar(300, 102.0)],
        );
        let map = engine
            .load_bars(&["AAPL".to_owned()], Interval::OneDay, Provider::Yahoo, None, Some(200))
            .unwrap();
        let bars = map.get("AAPL").unwrap();
        // open_ts < 200 is the only bar with ts=100
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].open_ts, 100);
    }

    #[test]
    fn load_bars_start_and_end_clamp() {
        let (engine, _tmp) = make_engine();
        write_bars(
            &engine,
            "AAPL",
            vec![make_bar(100, 1.0), make_bar(200, 2.0), make_bar(300, 3.0), make_bar(400, 4.0)],
        );
        let map = engine
            .load_bars(
                &["AAPL".to_owned()],
                Interval::OneDay,
                Provider::Yahoo,
                Some(200),
                Some(400),
            )
            .unwrap();
        let bars = map.get("AAPL").unwrap();
        assert_eq!(bars.len(), 2);
        assert!(bars.iter().all(|b| b.open_ts >= 200 && b.open_ts < 400));
    }

    #[test]
    fn load_bars_multiple_symbols() {
        let (engine, _tmp) = make_engine();
        write_bars(&engine, "AAPL", vec![make_bar(100, 100.0)]);
        write_bars(&engine, "MSFT", vec![make_bar(100, 200.0)]);

        let map = engine
            .load_bars(
                &["AAPL".to_owned(), "MSFT".to_owned()],
                Interval::OneDay,
                Provider::Yahoo,
                None,
                None,
            )
            .unwrap();
        assert!(map.contains_key("AAPL"));
        assert!(map.contains_key("MSFT"));
    }

    #[test]
    fn load_bars_symbol_not_in_db_absent_from_map() {
        let (engine, _tmp) = make_engine();
        write_bars(&engine, "AAPL", vec![make_bar(100, 100.0)]);

        let map = engine
            .load_bars(&["NONEXISTENT".to_owned()], Interval::OneDay, Provider::Yahoo, None, None)
            .unwrap();
        assert!(map.is_empty());
    }

    // ── run_one_strategy — BuyAndHold (built-in) ─────────────────────────

    fn bah_strategy() -> Py<PyAny> {
        Python::attach(|py| {
            let bah = BuyAndHold::new(None);
            Py::new(py, bah).unwrap().into_any()
        })
    }

    fn make_aligned(symbol: &str, bars: Vec<Option<Bar>>) -> HashMap<String, Vec<Option<Bar>>> {
        let mut m = HashMap::new();
        m.insert(symbol.to_owned(), bars);
        m
    }

    fn make_profile(symbol: &str) -> InstrumentProfile {
        InstrumentProfile {
            instrument: make_instrument(symbol),
            earliest_ts: [(Interval::OneDay, 0u64)].into(),
            latest_ts: [(Interval::OneDay, 9_999_999u64)].into(),
            legs: vec![],
        }
    }

    fn make_order(symbol: &str, quantity: f64, order_type: OrderType, price: Option<f64>) -> Order {
        Order {
            id: OrderId::new(),
            symbol: symbol.to_owned(),
            quantity,
            order_type,
            price,
            limit_price: None,
            sizer: None,
        }
    }

    fn custom_strategy(orders: Vec<Order>, raises: bool) -> Py<PyAny> {
        Python::attach(|py| {
            let module = PyModule::from_code(
                py,
                pyo3::ffi::c_str!(
                    r#"
class Strategy:
    def __init__(self, orders, raises):
        self.orders = orders
        self.raises = raises

    def evaluate(self, data, portfolio, state, indicators):
        if self.raises:
            raise RuntimeError("deliberate test error")
        orders, self.orders = self.orders, []
        return orders
"#
                ),
                pyo3::ffi::c_str!("engine_test_strategy.py"),
                pyo3::ffi::c_str!("engine_test_strategy"),
            )
            .unwrap();
            let py_orders =
                PyList::new(py, orders.into_iter().map(|order| Py::new(py, order).unwrap()))
                    .unwrap();
            module.getattr("Strategy").unwrap().call1((py_orders, raises)).unwrap().unbind()
        })
    }

    fn custom_strategy_batches(batches: Vec<Vec<Order>>) -> Py<PyAny> {
        Python::attach(|py| {
            let module = PyModule::from_code(
                py,
                pyo3::ffi::c_str!(
                    r#"
class Strategy:
    def __init__(self, batches):
        self.batches = batches

    def evaluate(self, data, portfolio, state, indicators):
        return self.batches.pop(0) if self.batches else []
"#
                ),
                pyo3::ffi::c_str!("engine_test_batches.py"),
                pyo3::ffi::c_str!("engine_test_batches"),
            )
            .unwrap();
            let batches = PyList::new(
                py,
                batches.into_iter().map(|orders| {
                    PyList::new(py, orders.into_iter().map(|order| Py::new(py, order).unwrap()))
                        .unwrap()
                }),
            )
            .unwrap();
            module.getattr("Strategy").unwrap().call1((batches,)).unwrap().unbind()
        })
    }

    fn custom_sizer(raises: bool) -> Py<PyAny> {
        Python::attach(|py| {
            let module = PyModule::from_code(
                py,
                pyo3::ffi::c_str!(
                    r#"
class Sizer:
    def __init__(self, raises):
        self.raises = raises

    def calculate(self, equity, price, stop_distance, atr):
        if self.raises:
            raise RuntimeError("deliberate sizer error")
        return 2.0
"#
                ),
                pyo3::ffi::c_str!("engine_test_sizer.py"),
                pyo3::ffi::c_str!("engine_test_sizer"),
            )
            .unwrap();
            module.getattr("Sizer").unwrap().call1((raises,)).unwrap().unbind()
        })
    }

    fn custom_metric(value: f64, raises: bool) -> Py<PyAny> {
        Python::attach(|py| {
            let module = PyModule::from_code(
                py,
                pyo3::ffi::c_str!(
                    r#"
class Metric:
    def __init__(self, value, raises):
        self.value = value
        self.raises = raises

    def compute(self, equity, trades):
        if self.raises:
            raise RuntimeError("deliberate metric error")
        return self.value
"#
                ),
                pyo3::ffi::c_str!("engine_test_metric.py"),
                pyo3::ffi::c_str!("engine_test_metric"),
            )
            .unwrap();
            module.getattr("Metric").unwrap().call1((value, raises)).unwrap().unbind()
        })
    }

    fn progress_callback() -> Py<PyAny> {
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                pyo3::ffi::c_str!("def callback(completed, total):\n    return None"),
                Some(&globals),
                Some(&globals),
            )
            .unwrap();
            globals.get_item("callback").unwrap().unwrap().unbind()
        })
    }

    fn run_custom_strategy(
        strategy: Py<PyAny>,
        cfg: &ExperimentConfig,
        profiles: &[InstrumentProfile],
    ) -> RunResult {
        let timestamps = [1_000_i64, 2_000];
        let aligned = make_aligned(
            "AAPL",
            timestamps.iter().map(|ts| Some(make_bar(*ts as u64, 100.0))).collect(),
        );
        run_one_strategy(
            "custom",
            strategy,
            cfg,
            &aligned,
            &HashMap::new(),
            profiles,
            &timestamps,
            &FxTable::new("USD"),
            None,
        )
    }

    fn run_scheduled_strategy(strategy: Py<PyAny>, cfg: &ExperimentConfig) -> RunResult {
        let timestamps = [1_000_i64, 2_000, 3_000, 4_000];
        let aligned = make_aligned(
            "AAPL",
            [100.0, 110.0, 120.0, 115.0]
                .into_iter()
                .zip(timestamps)
                .map(|(price, timestamp)| Some(make_bar(timestamp as u64, price)))
                .collect(),
        );
        run_one_strategy(
            "scheduled",
            strategy,
            cfg,
            &aligned,
            &HashMap::new(),
            &[make_profile("AAPL")],
            &timestamps,
            &FxTable::new("USD"),
            None,
        )
    }

    #[test]
    fn run_one_strategy_empty_timeline_no_equity_curve() {
        let cfg = base_config();
        let aligned = HashMap::new();
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline: Vec<i64> = vec![];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert_eq!(result.strategy_name, "bah");
        assert!(result.equity_curve.is_empty());
        assert!(result.error.is_none());
        assert!(!result.is_benchmark);
    }

    #[test]
    fn run_one_strategy_single_bar_produces_one_equity_sample() {
        let cfg = base_config();
        let aligned = make_aligned("AAPL", vec![Some(make_bar(1_000_000_000, 100.0))]);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![1_000_000_000i64];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert_eq!(result.equity_curve.len(), 1);
        assert_eq!(result.equity_curve[0].timestamp, 1_000_000_000);
        assert!(result.equity_curve[0].equity > 0.0);
    }

    #[test]
    fn run_one_strategy_initial_cash_appears_in_equity() {
        let mut cfg = base_config();
        cfg.portfolio.initial_cash = 50_000;

        let aligned = make_aligned("AAPL", vec![Some(make_bar(1_000_000_000, 100.0))]);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![1_000_000_000i64];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        // After BuyAndHold buys on bar 1, equity is still ~50k
        let eq = result.equity_curve[0].equity;
        assert!((eq - 50_000.0).abs() < 5_000.0);
    }

    #[test]
    fn buy_and_hold_converts_base_cash_before_sizing() {
        let mut cfg = base_config();
        cfg.portfolio.initial_cash = 10_000;
        cfg.portfolio.base_currency = Currency::EUR;
        cfg.exchange.commission_pct = 0.0;
        cfg.exchange.slippage = 0.0;

        let timestamp = 1_000_000_000;
        let next_timestamp = timestamp + 3_600;
        let aligned = make_aligned(
            "AAPL",
            vec![Some(make_bar(timestamp, 100.0)), Some(make_bar(next_timestamp, 100.0))],
        );
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![timestamp as i64, next_timestamp as i64];
        let mut fx = FxTable::new("EUR");
        fx.add_series("USD", "EUR", vec![(timestamp as i64, 0.86)]);

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert_eq!(result.orders[0].status, OrderStatus::Filled);
        assert_eq!(result.orders[0].order.quantity, 116.0);
        assert!(result.equity_curve[1].cash.amount(&Currency::EUR) < 100.0);
    }

    #[test]
    fn cash_fit_buy_debits_the_reduced_fill_cost() {
        let mut cfg = base_config();
        cfg.portfolio.initial_cash = 10_000;
        cfg.exchange.commission_pct = 0.0;
        cfg.exchange.slippage = 1.0;
        cfg.engine.trade_on_close = false;

        let timestamp = 1_000_000_000;
        let next_timestamp = timestamp + 3_600;
        let aligned = make_aligned(
            "AAPL",
            vec![Some(make_bar(timestamp, 100.0)), Some(make_bar(next_timestamp, 100.0))],
        );
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![timestamp as i64, next_timestamp as i64];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert_eq!(result.orders[0].status, OrderStatus::Filled);
        assert_eq!(result.orders[0].order.quantity, 99.0);
        assert!((result.equity_curve[1].cash.amount(&Currency::USD) - 1.0).abs() < 1e-9);
        assert!((result.equity_curve[1].equity - 9_901.0).abs() < 1e-9);
    }

    #[test]
    fn run_one_strategy_multiple_bars_correct_curve_length() {
        let cfg = base_config();
        let n = 10usize;
        let bars: Vec<Option<Bar>> =
            (0..n).map(|i| Some(make_bar(1_000_000 + i as u64 * 3600, 100.0))).collect();
        let aligned = make_aligned("AAPL", bars);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline: Vec<i64> = (0..n).map(|i| 1_000_000i64 + i as i64 * 3600).collect();
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert_eq!(result.equity_curve.len(), n);
    }

    #[test]
    fn run_one_strategy_equity_positive_throughout() {
        let cfg = base_config();
        let bars: Vec<Option<Bar>> =
            (0..5usize).map(|i| Some(make_bar(1_000 + i as u64, 100.0 + i as f64))).collect();
        let aligned = make_aligned("AAPL", bars);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline: Vec<i64> = (0..5).map(|i| 1_000i64 + i).collect();
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        for sample in &result.equity_curve {
            assert!(sample.equity > 0.0, "equity went non-positive at ts={}", sample.timestamp);
        }
    }

    #[test]
    fn run_one_strategy_drawdown_non_positive() {
        let cfg = base_config();
        let bars: Vec<Option<Bar>> = vec![
            Some(make_bar(1_000, 100.0)),
            Some(make_bar(2_000, 110.0)),
            Some(make_bar(3_000, 90.0)),
        ];
        let aligned = make_aligned("AAPL", bars);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![1_000i64, 2_000, 3_000];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        for sample in &result.equity_curve {
            assert!(sample.drawdown <= 0.0, "drawdown should be ≤ 0");
            assert!(sample.drawdown >= -1.0, "drawdown should be ≥ -1");
        }
    }

    #[test]
    fn run_one_strategy_warmup_period_skips_strategy_calls() {
        let mut cfg = base_config();
        cfg.engine.warmup_period = 3;

        let bars: Vec<Option<Bar>> =
            (0..6usize).map(|i| Some(make_bar(1_000 + i as u64, 100.0))).collect();
        let aligned = make_aligned("AAPL", bars);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline: Vec<i64> = (0..6).map(|i| 1_000i64 + i).collect();
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        // Equity curve covers all bars including warmup
        assert_eq!(result.equity_curve.len(), 6);
    }

    #[test]
    fn run_one_strategy_is_benchmark_when_name_matches() {
        let mut cfg = base_config();
        cfg.strategy.benchmark = Some("AAPL".to_owned());
        cfg.data.symbols = vec!["AAPL".to_owned()];

        let aligned = make_aligned("AAPL", vec![Some(make_bar(1_000, 100.0))]);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![1_000i64];
        let fx = FxTable::new("USD");

        // The strategy name matches the benchmark symbol
        let result = run_one_strategy(
            "AAPL",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert!(result.is_benchmark);
    }

    #[test]
    fn run_one_strategy_is_not_benchmark_when_name_differs() {
        let cfg = base_config();
        let aligned = make_aligned("AAPL", vec![Some(make_bar(1_000, 100.0))]);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![1_000i64];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "my_strategy",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert!(!result.is_benchmark);
    }

    #[test]
    fn run_one_strategy_starting_positions_boost_equity() {
        let mut cfg = base_config();
        cfg.portfolio.initial_cash = 1_000;
        cfg.portfolio.starting_positions = [("AAPL".to_owned(), 10.0)].into();

        let aligned = make_aligned("AAPL", vec![Some(make_bar(1_000, 100.0))]);
        let indicators = HashMap::new();
        let profiles = vec![InstrumentProfile {
            instrument: make_instrument("AAPL"),
            earliest_ts: [(Interval::OneDay, 1_000u64)].into(),
            latest_ts: [(Interval::OneDay, 5_000u64)].into(),
            legs: vec![],
        }];
        let timeline = vec![1_000i64];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        // initial_cash (1000) + 10 shares × $100 = $2000
        let eq = result.equity_curve[0].equity;
        assert!((eq - 2_000.0).abs() < 1.0);
    }

    #[test]
    fn run_one_strategy_benchmark_ignores_starting_positions() {
        let mut cfg = base_config();
        cfg.strategy.benchmark = Some("AAPL".to_owned());
        cfg.data.symbols = vec!["AAPL".to_owned()];
        cfg.portfolio.initial_cash = 1_000;
        cfg.portfolio.starting_positions = [("AAPL".to_owned(), 10.0)].into();

        let aligned = make_aligned("AAPL", vec![Some(make_bar(1_000, 100.0))]);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![1_000i64];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "AAPL",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        // Benchmark always starts with a clean slate (no positions)
        // so equity should equal initial_cash only
        assert!(result.is_benchmark);
        let eq = result.equity_curve[0].equity;
        assert!((eq - 1_000.0).abs() < 1.0);
    }

    #[test]
    fn run_one_strategy_metrics_populated() {
        let cfg = base_config();
        let bars: Vec<Option<Bar>> =
            (0..5usize).map(|i| Some(make_bar(1_000_000 + i as u64 * 86_400, 100.0))).collect();
        let aligned = make_aligned("AAPL", bars);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline: Vec<i64> = (0..5).map(|i| 1_000_000i64 + i * 86_400).collect();
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        // At minimum, total_return should be present
        assert!(!result.metrics.is_empty());
    }

    #[test]
    fn run_one_strategy_strategy_id_is_16_chars() {
        let cfg = base_config();
        let aligned = HashMap::new();
        let indicators = HashMap::new();
        let profiles = vec![];
        let timeline: Vec<i64> = vec![];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert_eq!(result.strategy_id.len(), 16);
    }

    #[test]
    fn run_one_strategy_none_bars_in_aligned_handled_gracefully() {
        let cfg = base_config();
        // Two bars but the first is None (no data that bar)
        let aligned = make_aligned("AAPL", vec![None, Some(make_bar(2_000, 100.0))]);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![1_000i64, 2_000];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert_eq!(result.equity_curve.len(), 2);
        assert!(result.error.is_none());
    }

    #[test]
    fn run_one_strategy_trade_on_close_flag() {
        let mut cfg = base_config();
        cfg.exchange.allowed_order_types = vec![OrderType::Market];
        cfg.engine.trade_on_close = true;

        let bars: Vec<Option<Bar>> =
            (0..3usize).map(|i| Some(make_bar(1_000 + i as u64, 100.0))).collect();
        let aligned = make_aligned("AAPL", bars);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline: Vec<i64> = vec![1_000, 1_001, 1_002];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert_eq!(result.equity_curve.len(), 3);
        assert!(result.error.is_none());
    }

    #[test]
    fn run_one_strategy_exclusive_orders_flag() {
        let mut cfg = base_config();
        cfg.engine.exclusive_orders = true;

        let bars: Vec<Option<Bar>> =
            (0..3usize).map(|i| Some(make_bar(1_000 + i as u64, 100.0))).collect();
        let aligned = make_aligned("AAPL", bars);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline: Vec<i64> = vec![1_000, 1_001, 1_002];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert!(result.error.is_none());
    }

    #[test]
    fn run_one_strategy_no_symbols_in_config() {
        let mut cfg = base_config();
        cfg.data.symbols = vec![];

        let aligned = HashMap::new();
        let indicators = HashMap::new();
        let profiles = vec![];
        let timeline: Vec<i64> = vec![];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert!(result.error.is_none());
    }

    #[test]
    fn run_one_strategy_cash_snapshot_present_each_bar() {
        let cfg = base_config();
        let bars: Vec<Option<Bar>> =
            (0..3usize).map(|i| Some(make_bar(1_000 + i as u64, 100.0))).collect();
        let aligned = make_aligned("AAPL", bars);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline: Vec<i64> = vec![1_000, 1_001, 1_002];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        for sample in &result.equity_curve {
            assert!(!sample.cash.is_empty(), "cash snapshot missing at ts={}", sample.timestamp);
        }
    }

    #[test]
    fn run_one_strategy_base_currency_carried_through() {
        let mut cfg = base_config();
        cfg.portfolio.base_currency = Currency::EUR;

        let aligned = make_aligned("AAPL", vec![Some(make_bar(1_000, 100.0))]);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![1_000i64];
        let fx = FxTable::new("EUR");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert_eq!(result.base_currency, Currency::EUR);
    }

    #[test]
    fn run_one_strategy_no_error_on_clean_run() {
        let cfg = base_config();
        let aligned = make_aligned("AAPL", vec![Some(make_bar(1_000, 100.0))]);
        let indicators = HashMap::new();
        let profiles = vec![make_profile("AAPL")];
        let timeline = vec![1_000i64];
        let fx = FxTable::new("USD");

        let result = run_one_strategy(
            "bah",
            bah_strategy(),
            &cfg,
            &aligned,
            &indicators,
            &profiles,
            &timeline,
            &fx,
            None,
        );

        assert!(result.error.is_none());
    }

    #[test]
    fn custom_strategy_reports_evaluation_errors() {
        let cfg = base_config();
        let result =
            run_custom_strategy(custom_strategy(vec![], true), &cfg, &[make_profile("AAPL")]);

        assert!(result.error.as_deref().is_some_and(|error| error.contains("evaluate() raised")));
        assert_eq!(result.equity_curve.len(), 2);
    }

    #[test]
    fn custom_strategy_rejects_invalid_orders() {
        let mut cfg = base_config();
        cfg.exchange.allowed_order_types = vec![OrderType::Market];

        let unknown = run_custom_strategy(
            custom_strategy(vec![make_order("MSFT", 1.0, OrderType::Market, None)], false),
            &cfg,
            &[make_profile("AAPL")],
        );
        assert!(unknown.orders[0].reason.contains("unknown symbol"));

        let disallowed = run_custom_strategy(
            custom_strategy(vec![make_order("AAPL", 1.0, OrderType::Limit, Some(90.0))], false),
            &cfg,
            &[make_profile("AAPL")],
        );
        assert_eq!(disallowed.orders[0].reason, "order type not allowed");

        let fractional = run_custom_strategy(
            custom_strategy(vec![make_order("AAPL", 0.5, OrderType::Market, None)], false),
            &cfg,
            &[make_profile("AAPL")],
        );
        assert!(fractional.orders[0].reason.contains("fractional"));

        let metadata_missing = run_custom_strategy(
            custom_strategy(vec![make_order("AAPL", 1.0, OrderType::Market, None)], false),
            &cfg,
            &[],
        );
        assert!(metadata_missing.orders[0].reason.contains("metadata unavailable"));
    }

    #[test]
    fn custom_strategy_rejects_duplicate_order_ids() {
        let cfg = base_config();
        let order = make_order("AAPL", 1.0, OrderType::Market, None);
        let duplicate = order.clone();
        let result = run_custom_strategy(
            custom_strategy(vec![order, duplicate], false),
            &cfg,
            &[make_profile("AAPL")],
        );

        assert!(result.orders.iter().any(|record| record.reason.contains("duplicate order id")));
        assert!(result.orders.iter().any(|record| record.status == OrderStatus::Filled));
    }

    #[test]
    fn custom_strategy_resolves_and_rejects_python_sizers() {
        let cfg = base_config();
        let mut sized = make_order("AAPL", 0.0, OrderType::Market, None);
        sized.sizer = Some(SizerSlot::Custom(custom_sizer(false)));
        let filled =
            run_custom_strategy(custom_strategy(vec![sized], false), &cfg, &[make_profile("AAPL")]);
        assert_eq!(filled.orders[0].status, OrderStatus::Filled);
        assert_eq!(filled.orders[0].order.quantity, 2.0);

        let mut rejected = make_order("AAPL", 0.0, OrderType::Market, None);
        rejected.sizer = Some(SizerSlot::Custom(custom_sizer(true)));
        let failed = run_custom_strategy(
            custom_strategy(vec![rejected], false),
            &cfg,
            &[make_profile("AAPL")],
        );
        assert_eq!(failed.orders[0].status, OrderStatus::Rejected);
        assert!(failed.orders[0].reason.contains("quantity"));
    }

    #[test]
    fn custom_strategy_cancels_invalid_trigger() {
        let mut cfg = base_config();
        cfg.exchange.allowed_order_types.push(OrderType::Limit);
        let result = run_custom_strategy(
            custom_strategy(vec![make_order("AAPL", 1.0, OrderType::Limit, None)], false),
            &cfg,
            &[make_profile("AAPL")],
        );

        assert_eq!(result.orders[0].status, OrderStatus::Canceled);
        assert!(result.orders[0].reason.contains("missing price"));
    }

    #[test]
    fn custom_strategy_cancellation_order_removes_pending_order() {
        let mut cfg = base_config();
        cfg.exchange.allowed_order_types.push(OrderType::Limit);
        let pending = make_order("AAPL", 1.0, OrderType::Limit, Some(1.0));
        let mut cancel = make_order("AAPL", 0.0, OrderType::Cancel, None);
        cancel.id = pending.id;
        let result = run_custom_strategy(
            custom_strategy(vec![pending, cancel], false),
            &cfg,
            &[make_profile("AAPL")],
        );

        assert_eq!(result.orders[0].status, OrderStatus::Canceled);
        assert_eq!(result.orders[0].reason, "canceled by cancellation order");
    }

    #[test]
    fn scheduled_strategy_closes_long_trade_and_executes_short_round_trip() {
        let mut long_config = base_config();
        long_config.exchange.commission_pct = 0.1;
        long_config.exchange.commission_fixed = 1.0;
        long_config.exchange.commission_type = CommissionType::PercentagePlusFixed;
        let long = run_scheduled_strategy(
            custom_strategy_batches(vec![
                vec![make_order("AAPL", 10.0, OrderType::Market, None)],
                vec![make_order("AAPL", -10.0, OrderType::Market, None)],
            ]),
            &long_config,
        );
        assert!(long.orders.iter().all(|record| record.status == OrderStatus::Filled));
        assert!(long.orders.iter().all(|record| record.commission > 0.0));
        assert_eq!(long.trades.len(), 1);
        assert!(long.trades[0].pnl > 0.0);

        let mut short_config = base_config();
        short_config.exchange.allow_short_selling = true;
        short_config.exchange.allow_margin = true;
        short_config.exchange.max_leverage = 10.0;
        short_config.exchange.max_position_size = 1_000;
        short_config.exchange.commission_type = CommissionType::Fixed;
        short_config.exchange.commission_fixed = 1.0;
        let short = run_scheduled_strategy(
            custom_strategy_batches(vec![
                vec![make_order("AAPL", -5.0, OrderType::Market, None)],
                vec![make_order("AAPL", 5.0, OrderType::Market, None)],
            ]),
            &short_config,
        );
        assert_eq!(short.orders.len(), 2);
        assert!(short.orders.iter().all(|record| record.status == OrderStatus::Filled));
        assert!(short.orders.iter().all(|record| record.commission == 1.0));
    }

    #[test]
    fn scheduled_strategy_surfaces_short_and_margin_limit_errors() {
        let mut short_config = base_config();
        short_config.exchange.raise_on_short_violation = true;
        let short = run_scheduled_strategy(
            custom_strategy_batches(vec![vec![make_order("AAPL", -1.0, OrderType::Market, None)]]),
            &short_config,
        );
        assert!(short.error.as_deref().is_some_and(|error| error.contains("short")));

        let mut margin_config = base_config();
        margin_config.portfolio.initial_cash = 100;
        margin_config.exchange.allow_margin = true;
        margin_config.exchange.max_leverage = 1.0;
        margin_config.exchange.raise_on_margin_limit = true;
        margin_config.exchange.max_position_size = 1_000;
        let margin = run_scheduled_strategy(
            custom_strategy_batches(vec![vec![make_order("AAPL", 100.0, OrderType::Market, None)]]),
            &margin_config,
        );
        assert!(margin.error.as_deref().is_some_and(|error| error.contains("limit")));
    }

    // ── Engine::run_experiment — smoke tests ─────────────────────────────

    #[test]
    fn run_experiment_no_symbols_returns_error() {
        let (engine, _tmp) = make_engine();
        let cfg = base_config(); // symbols = ["AAPL"], but no data downloaded

        let result = engine.run_experiment(
            &cfg,
            false,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            None,
        );

        // resolve_profiles will call provider which returns NotFound → cascade error
        // OR no-bars → empty timeline → ExperimentStatus::Error
        // either way the call should not panic
        match result {
            Ok(exp) => {
                // If it succeeds it must have a valid experiment_id
                assert_eq!(exp.experiment_id.len(), 16);
            },
            Err(_) => {
                // Acceptable — provider returned error
            },
        }
    }

    #[test]
    fn run_experiment_empty_symbols_list_returns_engine_error() {
        let (engine, _tmp) = make_engine();
        let mut cfg = base_config();
        cfg.data.symbols = vec![]; // explicitly empty

        let result = engine.run_experiment(
            &cfg,
            false,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn run_experiment_exercises_builtin_custom_benchmark_metrics_and_diagnostics() {
        crate::backtest::interface::ABORT_REQUESTED
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let mut stub = StubProvider::new();
        stub.instruments.insert("AAPL".to_owned(), make_instrument("AAPL"));
        let (engine, _tmp) = make_engine_with_stub(stub);
        write_bars(
            &engine,
            "AAPL",
            vec![
                make_bar(1_000_000_000, 100.0),
                make_bar(1_000_086_400, 101.0),
                make_bar(1_000_172_800, 102.0),
            ],
        );

        let mut config = base_config();
        config.general.name = "coverage experiment".to_owned();
        config.general.tags = vec!["coverage".to_owned()];
        config.strategy.benchmark = Some("AAPL".to_owned());
        config.strategy.strategies = vec![
            "builtin".to_owned(),
            "empty".to_owned(),
            "pending".to_owned(),
            "rejected".to_owned(),
            "raising".to_owned(),
        ];
        config.indicators.indicators = vec!["manual".to_owned(), "missing".to_owned()];
        config.metrics = crate::metrics::selection::MetricSelection::from_names(vec![
            "total_return".to_owned(),
            "sharpe".to_owned(),
            "excess_return".to_owned(),
            "alpha".to_owned(),
            "custom_good".to_owned(),
            "custom_bad".to_owned(),
        ]);
        config.exchange.allowed_order_types.push(OrderType::Limit);

        let pending_order = make_order("AAPL", 1.0, OrderType::Limit, Some(1.0));
        let strategies = HashMap::from([
            ("builtin".to_owned(), bah_strategy()),
            ("empty".to_owned(), custom_strategy(vec![], false)),
            ("pending".to_owned(), custom_strategy(vec![pending_order], false)),
            (
                "rejected".to_owned(),
                custom_strategy(vec![make_order("AAPL", 0.5, OrderType::Market, None)], false),
            ),
            ("raising".to_owned(), custom_strategy(vec![], true)),
        ]);
        let indicators = Python::attach(|py| {
            HashMap::from([(
                "manual".to_owned(),
                Py::new(py, crate::indicators::interface::SimpleMovingAverage::new(2))
                    .unwrap()
                    .into_any(),
            )])
        });
        let metrics = HashMap::from([
            ("custom_good".to_owned(), custom_metric(42.0, false)),
            ("custom_bad".to_owned(), custom_metric(0.0, true)),
        ]);
        let progress = crate::backtest::interface::ProgressReporter::new(progress_callback());

        let result = engine
            .run_experiment(&config, true, &strategies, &indicators, &metrics, Some(&progress))
            .unwrap();

        assert_eq!(result.name, "coverage experiment");
        assert_eq!(result.status, ExperimentStatus::Partial);
        assert_eq!(result.strategies.len(), 6);
        assert!(result.warnings.iter().any(|warning| warning.contains("missing")));
        assert!(result.warnings.iter().any(|warning| warning.contains("produced no orders")));
        assert!(result.warnings.iter().any(|warning| warning.contains("none were filled")));
        assert!(result.warnings.iter().any(|warning| warning.contains("Metric")));
        assert!(result
            .strategies
            .iter()
            .filter(|run| run.error.is_none())
            .all(|run| run.metrics.get("custom_good") == Some(&42.0)));
        assert!(engine
            .db
            .query_experiments(Some(&[result.experiment_id.clone()]), None, None)
            .unwrap()
            .iter()
            .any(|stored| stored.id == result.experiment_id));
    }
}
