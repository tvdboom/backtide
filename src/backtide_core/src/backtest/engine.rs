//! Backtest engine logic.
//!
//! This module implements the per-strategy event loop, order matching,
//! multi-currency portfolio bookkeeping and result aggregation.

use crate::backtest::fx::*;
use crate::backtest::interface::check_abort;
use crate::backtest::margin::*;
use crate::backtest::models::*;
use crate::backtest::orders::*;
use crate::backtest::utils::*;
use crate::constants::*;
use crate::data::models::*;
use crate::engine::Engine;
use crate::errors::{EngineError, EngineResult};
use crate::indicators::interface::_indicator_deterministic_name;
use crate::indicators::utils::compute_indicators;
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
                    config,
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
    let base_ccy_ref: &str = &base_ccy_str;

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
    let mut trail_state: HashMap<OrderId, (f64, f64)> = HashMap::new();

    let total_bars: usize = aligned.values().map(|v| v.len()).next().unwrap_or(0);
    let mut equity_curve: Vec<EquitySample> = Vec::with_capacity(total_bars);
    let mut order_records: Vec<OrderRecord> = Vec::new();
    let mut closed_trades: Vec<Trade> = Vec::new();

    // Open trade tracker per symbol: (entry_ts, qty_remaining, entry_price)
    let mut open_trades: HashMap<String, (i64, f64, f64)> = HashMap::new();
    let mut margin_limit_warnings: HashSet<String> = HashSet::new();

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

            let it = *it_map.get(order.symbol.as_str()).unwrap();

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
                // Avoid spam warnings.
                let warning_key = limit_warning_dedupe_key(&order.symbol, violation, &reason);
                if margin_limit_warnings.insert(warning_key) {
                    warn!(strategy=%name, order_id=%order.id, "{reason}");
                } else {
                    debug!(strategy=%name, order_id=%order.id, "suppressed repeated limit rejection: {reason}");
                }

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

            let mut filled_qty = 0.;
            let mut fill_pnl: Option<f64> = None;

            if *qty > 0.0 {
                // BUY: try paying in `order_ccy` first, else convert from base.
                if !try_debit(&mut cash, order_ccy, notional + commission, base_ccy, fx, ts) {
                    // Auto-shrink the order rather than reject it.
                    // Compute available funds in `order_ccy` by summing every
                    // cash bucket converted to `order_ccy` at the current
                    // bar's FX rate (forward-filled). Buckets whose currency
                    // can't be converted at `ts` are ignored.
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
                let bars_view: Vec<(&str, &[Bar])> = symbols
                    .iter()
                    .zip(bars_full.iter())
                    .map(|(&s, (_, v))| (s, &v[..=bar_index]))
                    .collect();

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
                    let data = build_per_symbol_view(py, cached_data, bar_index)?;
                    let inds = build_indicator_view(py, cached_indicators, bar_index)?;

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

                            // Compute mark-to-market equity in the same currency
                            // as the symbol's price.
                            let eq = compute_portfolio_equity(
                                &cash,
                                &positions,
                                aligned,
                                bar_index,
                                &quote_ccy,
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

                            let stop_distance: Option<f64> = o.price.and_then(|p| {
                                let d = (sym_price - p).abs();
                                if d > 0.0 {
                                    Some(d)
                                } else {
                                    None
                                }
                            });

                            // Call sizer.calculate(equity, price, stop_distance, atr).
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

                        // Reject orders targeting a symbol not present in the experiment.
                        if !it_map.contains_key(o.symbol.as_str()) {
                            let reason = format!(
                                "unknown symbol {:?}: not in the experiment's symbol list",
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
                            let it = *it_map.get(o.symbol.as_str()).unwrap();
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
    }

    // ── Liquidate remaining positions to compute final PnL ──────────────────

    if let Some(last_idx) = total_bars.checked_sub(1) {
        for (sym, qty) in positions.clone() {
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
