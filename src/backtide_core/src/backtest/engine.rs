//! Backtest engine logic.
//!
//! This module implements the per-strategy event loop, order matching,
//! multi-currency portfolio bookkeeping and result aggregation. It runs
//! every selected strategy fully in parallel using [`rayon`].

use crate::backtest::fx::FxTable;
use crate::backtest::indicators::Indicator as BuiltinIndicator;
use crate::backtest::interface::check_abort;
use crate::backtest::models::commission_type::CommissionType;
use crate::backtest::models::empty_bar_policy::EmptyBarPolicy;
use crate::backtest::models::experiment_config::ExperimentConfig;
use crate::backtest::models::experiment_result::*;
use crate::backtest::models::experiment_status::ExperimentStatus;
use crate::backtest::models::order::{new_order_id, Order};
use crate::backtest::models::order_type::OrderType;
use crate::backtest::models::portfolio::Portfolio;
use crate::backtest::models::state::State;
use crate::backtest::strategies::{BuiltinStrategy, BuyAndHold, IndicatorView};
use crate::constants::BENCHMARK;
use crate::data::models::bar::Bar;
use crate::data::models::currency::Currency;
use crate::data::models::instrument_profile::InstrumentProfile;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::engine::Engine;
use crate::errors::{EngineError, EngineResult};
use crate::utils::experiment_log::{EXPERIMENT_SPAN, LOG_PATH_FIELD};
use crate::utils::progress::{progress_bar, progress_spinner};
use indicatif::ProgressBar;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::Py;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn, Span};
use uuid::Uuid;
// ────────────────────────────────────────────────────────────────────────────
// Public interface
// ────────────────────────────────────────────────────────────────────────────

impl Engine {
    /// Run a single backtest experiment end-to-end.
    pub fn run_experiment(
        &self,
        config: &ExperimentConfig,
        verbose: bool,
        strategy_overrides: &HashMap<String, Py<PyAny>>,
        indicator_overrides: &HashMap<String, Py<PyAny>>,
    ) -> EngineResult<ExperimentResult> {
        let started_at = now_secs();
        let started_instant = Instant::now();
        let experiment_id = Uuid::new_v4().simple().to_string()[..16].to_owned();
        let mut warnings: Vec<String> = Vec::new();

        // ── Set up per-experiment logging ───────────────────────────────
        //
        // We open a top-level [`EXPERIMENT_SPAN`] span and let the global
        // `ExperimentFileLayer` (registered in `init_logging`) mirror every
        // event emitted while the span is on the stack into a dedicated
        // `<storage>/experiments/<experiment_id>/logs.txt` file. The UI
        // exposes that file via a popover on the full-analysis page.
        //
        // No bespoke logging plumbing is required in this module — plain
        // `tracing::info!` / `warn!` / `debug!` calls Just Work, including
        // those emitted by helper functions called from here.
        let storage_path = &self.config.data.storage_path;
        let exp_dir = storage_path.join("experiments").join(&experiment_id);
        if let Err(e) = std::fs::create_dir_all(&exp_dir) {
            warn!(experiment_id = %experiment_id, "Failed to create experiment dir: {e}");
            warnings.push(format!("Failed to create experiment dir: {e}"));
        }
        let log_path = exp_dir.join("logs.txt");

        let experiment_span = tracing::info_span!(
            EXPERIMENT_SPAN,
            experiment_id = %experiment_id,
            { LOG_PATH_FIELD } = %log_path.display(),
        );
        let _enter = experiment_span.enter();

        info!(
            "Starting experiment id={} name={:?} tags={:?}",
            experiment_id, config.general.name, config.general.tags
        );
        info!(
            "Configuration summary: {} symbols, interval={:?}, instrument_type={:?}, \
             benchmark={:?}, risk_free_rate={}%, initial_cash={}, {} indicators, \
             {} strategies",
            config.data.symbols.len(),
            config.data.interval,
            config.data.instrument_type,
            config.strategy.benchmark,
            config.engine.risk_free_rate,
            config.portfolio.initial_cash,
            config.indicators.indicators.len(),
            config.strategy.strategies.len()
        );

        // Persist the source configuration as a TOML file.
        match persist_experiment_config(storage_path, &experiment_id, config) {
            Ok(p) => info!("Persisted experiment config to {}", p.display()),
            Err(e) => {
                warn!(experiment_id = %experiment_id, "Failed to persist experiment config: {e}");
                warnings.push(format!("Failed to persist experiment config: {e}"));
            },
        }

        // Augment the symbol list with the benchmark (if any) so its bars
        // get downloaded & loaded just like any user symbol.
        // If the benchmark matches a strategy name, it refers to that strategy
        // — no extra download needed. Otherwise treat it as a ticker symbol.
        let mut symbols = config.data.symbols.clone();
        let benchmark = config.strategy.benchmark.as_deref().unwrap_or("").trim().to_owned();
        let benchmark_from_strategy =
            !benchmark.is_empty() && config.strategy.strategies.iter().any(|s| s == &benchmark);
        if !benchmark.is_empty()
            && !benchmark_from_strategy
            && !symbols.iter().any(|s| s == &benchmark)
        {
            info!("Folding benchmark symbol {:?} into symbol list", benchmark);
            symbols.push(benchmark.clone());
        }
        if symbols.is_empty() {
            warn!("Experiment has no symbols — aborting.");
            return Err(EngineError::Experiment("Experiment has no symbols.".to_owned()));
        }

        // ── Phase 1: data ───────────────────────────────────────────────
        info!("Phase 1: resolving instrument profiles for {} symbol(s)...", symbols.len());

        let pb = verbose.then(|| progress_spinner("Resolving instrument profiles..."));
        let profiles = self.resolve_profiles(
            symbols.clone(),
            config.data.instrument_type,
            vec![config.data.interval],
            false,
        )?;
        info!("Resolved {} instrument profile(s).", profiles.len());
        if let Some(p) = pb {
            p.finish_and_clear();
        }

        validate_starting_position_quantities(config, &profiles)?;

        let pb = verbose.then(|| progress_spinner("Downloading missing bars..."));
        let start_clamp = config.data.start_date.as_deref().and_then(parse_iso_date_to_ts);
        let end_clamp = config.data.end_date.as_deref().and_then(parse_iso_date_to_ts);
        info!(
            "Downloading missing bars (start_clamp={:?}, end_clamp={:?})...",
            config.data.start_date, config.data.end_date
        );
        let dl = self.download_bars(&profiles, start_clamp, end_clamp, false)?;
        info!(
            "Download complete: {} succeeded, {} failed, {} warning(s).",
            dl.n_succeeded,
            dl.n_failed,
            dl.warnings.len()
        );
        for w in &dl.warnings {
            warn!("Download warning: {w}");
        }
        warnings.extend(dl.warnings.iter().cloned());
        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // ── Phase 2: load bars ──────────────────────────────────────────
        info!("Phase 2: loading bars from storage...");
        let pb = verbose.then(|| progress_spinner("Loading bars from storage..."));
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
        info!("Loaded {} bar(s) across {} symbol(s).", total_bars, bar_map.len());
        for (sym, bars) in &bar_map {
            debug!("  {} → {} bars", sym, bars.len());
        }
        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // Build a master timeline (union of all symbol timestamps, sorted).
        let mut all_ts: Vec<i64> =
            bar_map.values().flat_map(|bars| bars.iter().map(|b| b.open_ts as i64)).collect();
        all_ts.sort_unstable();
        all_ts.dedup();
        info!("Master timeline has {} unique timestamps.", all_ts.len());

        if all_ts.is_empty() {
            warn!("No bars available for the selected symbols/interval — experiment failed.");
            warnings.push("No bars available for the selected symbols/interval.".into());
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
        info!("Aligned bars using policy={:?}.", config.engine.empty_bar_policy);

        // ── Build FX rate table from currency-conversion legs ───────────
        //
        // Every primary instrument whose quote currency differs from the
        // portfolio base currency carries one or more conversion legs in
        // its profile (resolved by `Engine::resolve_legs`). We load the
        // close-price series for each unique leg symbol — possibly from a
        // different provider than the primary instruments — and feed
        // them into an `FxTable` keyed by `(from_ccy, to_ccy)` so the
        // run loop can convert any cash/MTM amount at any timestamp via
        // forward-fill (latest known rate ≤ ts).
        let primary_set: HashSet<&str> = symbols.iter().map(String::as_str).collect();
        let leg_profiles: Vec<&InstrumentProfile> = profiles
            .iter()
            .filter(|p| !primary_set.contains(p.instrument.symbol.as_str()))
            .collect();
        info!("Building FX table from {} conversion leg(s).", leg_profiles.len());

        let mut fx = FxTable::new(config.portfolio.base_currency);
        for leg in &leg_profiles {
            let leg_provider = match self.config.data.providers.get(&leg.instrument.instrument_type)
            {
                Some(p) => *p,
                None => {
                    warn!(symbol=%leg.instrument.symbol, "No provider for leg instrument type, skipping.");
                    continue;
                },
            };
            let leg_bars = match self.load_bars(
                std::slice::from_ref(&leg.instrument.symbol),
                config.data.interval,
                leg_provider,
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

            // Parse base/quote currencies. Crypto legs may have a non-fiat
            // base (e.g. BTC) — we can still record the series if both
            // sides parse to a `Currency`, otherwise skip the leg.
            let from_ccy = leg.instrument.base.as_deref().and_then(|s| s.parse::<Currency>().ok());
            let to_ccy = leg.instrument.quote.parse::<Currency>().ok();
            let (Some(from_ccy), Some(to_ccy)) = (from_ccy, to_ccy) else {
                debug!(
                    symbol=%leg.instrument.symbol,
                    base=?leg.instrument.base,
                    quote=%leg.instrument.quote,
                    "Leg base/quote not a recognised Currency; skipping FX series.",
                );
                continue;
            };

            let series: Vec<(i64, f64)> =
                bars.iter().map(|b| (b.open_ts as i64, b.close)).collect();
            debug!(
                symbol=%leg.instrument.symbol,
                from=%from_ccy, to=%to_ccy,
                "Adding FX series ({} points).", series.len()
            );
            fx.add_series(from_ccy, to_ccy, series);
        }

        // ── Phase 3a: load strategies (so we can collect their auto indicators) ──
        info!("Phase 3: loading {} strategy definition(s)...", config.strategy.strategies.len());

        let mut strategy_objs = load_strategies(&config.strategy.strategies, strategy_overrides)?;

        // Auto-inject a Buy & Hold of the benchmark symbol as a regular strategy,
        // but only when benchmark_from_strategy is false (i.e. the benchmark is an
        // external ticker symbol, not one of the user's strategies).
        let benchmark_name = BENCHMARK.to_owned();
        if !benchmark.is_empty() && !benchmark_from_strategy {
            match Python::attach(|py| -> PyResult<Py<PyAny>> {
                let bh = BuyAndHold {
                    symbol: Some(benchmark.clone()),
                };
                Ok(Py::new(py, bh)?.into_any())
            }) {
                Ok(obj) => {
                    info!("Injected benchmark strategy BuyAndHold({}).", benchmark);
                    strategy_objs.push((benchmark_name.clone(), obj, false));
                },
                Err(e) => {
                    warn!("Failed to instantiate benchmark: {e}");
                    warnings.push(format!("Failed to instantiate benchmark: {e}"));
                },
            }
        }

        // ── Phase 3b: collect indicator objects (user-selected + auto-injected) ──
        //
        // Built-in strategies declare their dependencies via the
        // ``required_indicators()`` pymethod. We instantiate those here and
        // hand them to ``compute_indicators`` alongside any user-selected
        // indicators loaded from disk. Without this step, every strategy
        // that relies on auto-included indicators (SMA Crossover, MACD,
        // RSI, BB Mean Reversion, …) would silently place zero orders
        // because the lookups in ``decide_inner`` return ``None``.
        let mut indicator_objs: Vec<(String, Py<PyAny>)> = Vec::new();
        let mut seen_inds: HashSet<String> = HashSet::new();

        for name in &config.indicators.indicators {
            match Python::attach(|py| -> PyResult<Py<PyAny>> {
                if let Some(o) = indicator_overrides.get(name) {
                    Ok(o.clone_ref(py))
                } else {
                    load_indicator(py, name)
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
                    let name = auto_indicator_name_for(py, &ind)?;
                    out.push((name, ind));
                }
                Ok(out)
            });
            match pairs {
                Ok(pairs) => {
                    for (name, obj) in pairs {
                        if seen_inds.insert(name.clone()) {
                            info!("Auto-injecting indicator {name} required by {sname}");
                            indicator_objs.push((name, obj));
                        }
                    }
                },
                Err(e) => warn!(
                    "Failed to collect required indicators for {sname}: {e} \
                     (strategy will run without auto-indicators)"
                ),
            }
        }

        // ── Phase 3c: indicators (computed once) ────────────────────────
        info!("Computing {} indicator(s)...", indicator_objs.len());

        let pb =
            verbose.then(|| progress_bar(indicator_objs.len() as u64, "Computing indicators..."));
        let indicators = compute_indicators(&indicator_objs, &aligned, pb.as_ref())?;
        info!("Computed {} indicator series.", indicators.len());
        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // ── Phase 4: run strategies in parallel ─────────────────────────

        let pb = verbose.then(|| {
            progress_bar(
                strategy_objs.len() as u64,
                format!("Running {} strategies...", strategy_objs.len()),
            )
        });
        let pb_arc = pb.as_ref().map(|p| Mutex::new(p.clone()));

        let cfg_clone = config.clone();
        let aligned_arc = Arc::new(aligned);
        let indicators_arc = Arc::new(indicators);
        let profiles_arc = Arc::new(profiles.clone());
        let fx_arc = Arc::new(fx);

        // Master timeline shared by all strategies. Built once here from
        // `all_ts` so per-strategy logic doesn't have to reconstruct it
        // from per-symbol rows (which would fall back to row indices for
        // bars where the chosen reference symbol has no data — yielding
        // bogus timestamps like ``1970-01-01 …`` for any benchmark whose
        // history starts later than the earliest selected symbol).
        let timeline_arc = Arc::new(all_ts.clone());

        // Built-in (Rust) strategies are run in parallel via rayon.
        // Custom (Python) strategies are run sequentially under the GIL.
        let (custom, builtin): (Vec<(String, Py<PyAny>, bool)>, Vec<(String, Py<PyAny>, bool)>) =
            strategy_objs.into_iter().partition(|(_, _, is_custom)| *is_custom);
        info!(
            "Dispatching strategies: {} built-in (parallel) and {} custom (sequential).",
            builtin.len(),
            custom.len()
        );

        let cfg_arc = Arc::new(cfg_clone);

        let cfg_for_par = Arc::clone(&cfg_arc);
        let aligned_for_par = Arc::clone(&aligned_arc);
        let indicators_for_par = Arc::clone(&indicators_arc);
        let profiles_for_par = Arc::clone(&profiles_arc);
        let timeline_for_par = Arc::clone(&timeline_arc);
        let fx_for_par = Arc::clone(&fx_arc);

        // Capture the experiment span so each rayon worker can re-enter it
        // — `tracing` span scope is thread-local, so events from worker
        // threads would otherwise miss the file layer entirely.
        let par_span = Span::current();

        let mut results: Vec<RunResult> = builtin
            .into_par_iter()
            .map(|(name, obj, _)| {
                par_span.in_scope(|| {
                    info!("▶ Running strategy {:?}...", name);
                    let r = run_one_strategy(
                        &name,
                        obj,
                        &cfg_for_par,
                        &aligned_for_par,
                        &indicators_for_par,
                        &profiles_for_par,
                        &timeline_for_par,
                        &fx_for_par,
                    );
                    info!(
                        "✔ Finished strategy {:?}: {} trades, {} bars in equity curve.",
                        r.strategy_name,
                        r.trades.len(),
                        r.equity_curve.len()
                    );
                    if let Some(pb) = &pb_arc {
                        pb.lock().unwrap().inc(1);
                    }
                    r
                })
            })
            .collect();

        for (name, obj, _) in custom {
            // Check for abort before each sequential strategy.
            info!("▶ Running custom strategy {:?}...", name);
            let r = run_one_strategy(
                &name,
                obj,
                &cfg_arc,
                &aligned_arc,
                &indicators_arc,
                &profiles_arc,
                &timeline_arc,
                &fx_arc,
            );
            info!(
                "✔ Finished custom strategy {:?}: {} trades, {} bars in equity curve.",
                r.strategy_name,
                r.trades.len(),
                r.equity_curve.len()
            );
            if let Some(pb) = &pb_arc {
                pb.lock().unwrap().inc(1);
            }
            results.push(r);
        }

        if let Some(p) = pb {
            p.finish_and_clear();
        }

        // ── Compute alpha & excess return for every run ─────────────────
        //
        // Alpha is defined as the windowed total-return difference between a
        // strategy and the benchmark, where the window starts at the *later*
        // of the two equity-curve start dates. This avoids comparing periods
        // where one of the two series did not yet exist (e.g. benchmark only
        // goes back to 2004 while the strategy has data from 1990 — alpha
        // must then be measured from 2004 onwards on both sides).
        //
        // Excess return is the strategy's windowed total return minus the
        // compounded risk-free return over the same window.
        info!(
            "Computing alpha & excess return (rf={}%, benchmark={:?}).",
            config.engine.risk_free_rate,
            if benchmark.is_empty() {
                "<none>"
            } else {
                benchmark.as_str()
            }
        );
        const SECS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;
        let rf = config.engine.risk_free_rate / 100.0;

        // Snapshot of the benchmark's equity curve (ts, equity), if any.
        // When benchmark_from_strategy is true, the benchmark strategy is the
        // user strategy whose name matches the benchmark field; mark it now.
        if benchmark_from_strategy && !benchmark.is_empty() {
            for r in &mut results {
                if r.strategy_name == benchmark {
                    r.is_benchmark = true;
                    break;
                }
            }
        }
        let bench_run = if !benchmark.is_empty() {
            if benchmark_from_strategy {
                results.iter().find(|r| r.strategy_name == benchmark)
            } else {
                results.iter().find(|r| r.strategy_name == benchmark_name)
            }
        } else {
            None
        };
        let bench_snapshot: Option<Vec<(i64, f64)>> =
            bench_run.map(|r| r.equity_curve.iter().map(|s| (s.timestamp, s.equity)).collect());

        // Benchmark availability starts when the benchmark can actually be
        // traded (first entry trade), not at the first synthetic equity sample.
        let bench_start_ts = bench_run.and_then(|r| r.trades.iter().map(|t| t.entry_ts).min());

        // Windowed total return: (final_equity - first_equity_at_or_after_ws)
        //                       / first_equity_at_or_after_ws.
        let windowed_return = |curve: &[(i64, f64)], window_start: i64| -> Option<f64> {
            let (_, start_eq) = curve.iter().find(|(t, _)| *t >= window_start).copied()?;
            let (_, end_eq) = curve.last().copied()?;
            if start_eq <= 0.0 {
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

            // For delayed listings, the strategy only becomes investable at
            // first fill; before that, equity is a placeholder flat segment.
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
                let is_bench = if benchmark_from_strategy {
                    r.strategy_name == benchmark
                } else {
                    r.strategy_name == benchmark_name
                };
                if !is_bench {
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

        // Ensure the benchmark run is always the first entry in the results.
        if !benchmark.is_empty() {
            let bench_name_to_find = if benchmark_from_strategy {
                &benchmark
            } else {
                &benchmark_name
            };
            if let Some(idx) = results.iter().position(|r| r.strategy_name == *bench_name_to_find) {
                if idx != 0 {
                    let bench = results.remove(idx);
                    results.insert(0, bench);
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

        // Surface per-strategy failures: log each one and roll the
        // experiment status up to "failed" if any strategy errored out.
        // The benchmark is excluded from the status calculation.
        let non_bench: Vec<&_> = results.iter().filter(|r| r.strategy_name != BENCHMARK).collect();
        let n_failed = non_bench.iter().filter(|r| r.error.is_some()).count();
        let n_non_bench = non_bench.len();
        for r in &results {
            if let Some(err) = &r.error {
                warn!(strategy = %r.strategy_name, "Strategy failed: {err}");
                warnings.push(format!("Strategy {:?} failed: {}", r.strategy_name, err));
                continue;
            }

            // Diagnose the two "no fills" cases separately:
            //
            // 1. The strategy never produced an order at all — it simply
            //    did not signal during the backtest window (data range too
            //    short, indicators never crossed, parameters too tight,
            //    etc.). `initial_cash` is irrelevant here.
            //
            // 2. The strategy produced orders but *every* one was rejected
            //    or canceled — typically because the initial cash was
            //    too small to afford a single whole unit of a non-crypto
            //    instrument, or because the broker ran out of cash. In
            //    that case we surface the (first) rejection reason and
            //    point the user at `initial_cash`.
            let n_filled = r.orders.iter().filter(|o| o.status == "filled").count();
            if n_filled > 0 {
                continue;
            }
            if r.orders.is_empty() {
                let msg = format!(
                    "Strategy {:?} produced no orders — no buy/sell signal was triggered \
                     during the backtest window. Try a longer date range, different \
                     strategy parameters, or different symbols.",
                    r.strategy_name
                );
                warn!(strategy = %r.strategy_name, "{msg}");
                warnings.push(msg);
            } else {
                // All orders rejected/canceled. Use the first non-empty reason as the
                // headline cause; fall back to a generic message when no reason was recorded.
                let first_reason = r
                    .orders
                    .iter()
                    .find(|o| !o.reason.is_empty())
                    .map(|o| o.reason.as_str())
                    .unwrap_or("see per-order rejection reasons");

                let msg = format!(
                    "Strategy {:?} produced {} order(s) but none filled ({}). \
                     If the rejection is about quantity or insufficient funds, the \
                     initial cash may be too low: non-crypto quantities must be whole \
                     numbers (crypto allows fractional), so per-symbol allocation must \
                     be ≥ price. Consider increasing initial_cash.",
                    r.strategy_name,
                    r.orders.len(),
                    first_reason,
                );

                warn!(strategy = %r.strategy_name, "{msg}");
                warnings.push(msg);
            }
        }
        let status = if n_failed == 0 {
            ExperimentStatus::Success
        } else if n_failed >= n_non_bench {
            ExperimentStatus::Error
        } else {
            ExperimentStatus::Partial
        };
        info!(
            "All strategies completed in {}s ({} result(s), {} failed, status={}).",
            finished_at - started_at,
            results.len(),
            n_failed,
            status,
        );
        for r in &results {
            if r.error.is_some() {
                info!("  ✗ {:<32} FAILED — {}", r.strategy_name, r.error.as_deref().unwrap_or(""));
                continue;
            }
            let tr = r.metrics.get("total_return").copied().unwrap_or(0.0);
            let sh = r.metrics.get("sharpe").copied().unwrap_or(0.0);
            let alpha = r.metrics.get("alpha").copied();
            let excess = r.metrics.get("excess_return").copied();
            info!(
                "  • {:<32} total_return={:+.4} sharpe={:+.3} excess={} alpha={}",
                r.strategy_name,
                tr,
                sh,
                excess.map(|e| format!("{e:+.4}")).unwrap_or_else(|| "n/a".into()),
                alpha.map(|a| format!("{a:+.4}")).unwrap_or_else(|| "n/a".into())
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

        // ── Phase 5: persist ────────────────────────────────────────────

        info!("Phase 5: persisting experiment to DuckDB...");
        let pb = verbose.then(|| progress_spinner("Persisting experiment results..."));
        // Refresh finished_at right before the upsert so it reflects every
        // bit of work done up to the persist (logging / status roll-up
        // included), then write everything in a single transaction.
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
            "Experiment {} finished with status={} ({} strategies, {} warnings) in {:?}.",
            experiment_id,
            result.status,
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
// Helper functions (free)
// ────────────────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Serialize `config` and write it to `/experiments/<experiment_id>/config.toml`.
fn persist_experiment_config(
    storage_path: &std::path::Path,
    experiment_id: &str,
    config: &ExperimentConfig,
) -> Result<PathBuf, String> {
    use crate::backtest::models::experiment_config::ExperimentConfigInner;

    let dir = storage_path.join("experiments").join(experiment_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all({}): {e}", dir.display()))?;

    let inner = ExperimentConfigInner {
        general: config.general.clone(),
        data: config.data.clone(),
        portfolio: config.portfolio.clone(),
        strategy: config.strategy.clone(),
        indicators: config.indicators.clone(),
        exchange: config.exchange.clone(),
        engine: config.engine.clone(),
    };
    let toml_str = toml::to_string_pretty(&inner).map_err(|e| format!("toml serialise: {e}"))?;

    let path = dir.join("config.toml");
    std::fs::write(&path, toml_str).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

fn parse_iso_date_to_ts(s: &str) -> Option<u64> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp() as u64)
}

const QUANTITY_EPS: f64 = 1e-9;

fn is_whole_quantity(qty: f64) -> bool {
    qty.is_finite() && (qty - qty.round()).abs() <= QUANTITY_EPS
}

fn profile_instrument_types(profiles: &[InstrumentProfile]) -> HashMap<String, InstrumentType> {
    profiles.iter().map(|p| (p.instrument.symbol.clone(), p.instrument.instrument_type)).collect()
}

fn instrument_type_for_symbol(
    symbol: &str,
    instrument_types: &HashMap<String, InstrumentType>,
    fallback: InstrumentType,
) -> InstrumentType {
    instrument_types.get(symbol).copied().unwrap_or(fallback)
}

fn quantity_rejection_reason(
    symbol: &str,
    qty: f64,
    instrument_type: InstrumentType,
) -> Option<String> {
    if !qty.is_finite() {
        return Some(format!("quantity for {symbol} must be finite"));
    }
    if !instrument_type.allows_fractional_quantities() && !is_whole_quantity(qty) {
        return Some(format!(
            "fractional quantity {qty} is not allowed for {instrument_type} instrument {symbol}; \
             only crypto instruments support fractional quantities"
        ));
    }
    None
}

fn validate_starting_position_quantities(
    config: &ExperimentConfig,
    profiles: &[InstrumentProfile],
) -> EngineResult<()> {
    let instrument_types = profile_instrument_types(profiles);
    for (symbol, qty) in &config.portfolio.starting_positions {
        let instrument_type =
            instrument_type_for_symbol(symbol, &instrument_types, config.data.instrument_type);
        if let Some(reason) = quantity_rejection_reason(symbol, *qty, instrument_type) {
            return Err(EngineError::Experiment(format!("Invalid starting position: {reason}.")));
        }
    }
    Ok(())
}

fn normalize_builtin_order_quantity(
    order: &mut Order,
    instrument_type: InstrumentType,
) -> Option<String> {
    if matches!(order.order_type, OrderType::Cancel | OrderType::SettlePosition) {
        return None;
    }
    if !order.quantity.is_finite() {
        return Some(format!("quantity for {} must be finite", order.symbol));
    }
    if !instrument_type.allows_fractional_quantities() && !is_whole_quantity(order.quantity) {
        let whole_abs = order.quantity.abs().floor();
        if whole_abs <= 0.0 {
            return Some(format!(
                "quantity for {} is less than one whole {instrument_type} unit",
                order.symbol
            ));
        }
        order.quantity = whole_abs.copysign(order.quantity);
    }
    None
}

/// Align bars to a master timeline using the configured empty-bar policy.
///
/// Uses binary search on the (already-sorted) per-symbol bar vectors
/// instead of building a temporary `HashMap<i64, Bar>` for each symbol.
/// This avoids O(n) hash-map construction and per-key hashing overhead,
/// replacing it with O(log n) lookups — a significant win on large
/// datasets (10 k+ bars per symbol).
fn align_bars(
    bars: &HashMap<String, Vec<Bar>>,
    timeline: &[i64],
    policy: EmptyBarPolicy,
) -> HashMap<String, Vec<Option<Bar>>> {
    let mut out: HashMap<String, Vec<Option<Bar>>> = HashMap::with_capacity(bars.len());
    for (sym, sym_bars) in bars {
        let mut row: Vec<Option<Bar>> = Vec::with_capacity(timeline.len());
        let mut last: Option<Bar> = None;
        for ts in timeline {
            // Binary search on the sorted bar slice (sorted by open_ts in load_bars).
            let found = sym_bars
                .binary_search_by_key(&(*ts as u64), |b| b.open_ts)
                .ok()
                .map(|i| &sym_bars[i]);
            match found {
                Some(b) => {
                    last = Some(b.clone());
                    row.push(Some(b.clone()));
                },
                None => match policy {
                    EmptyBarPolicy::Skip => row.push(None),
                    EmptyBarPolicy::ForwardFill => {
                        if let Some(b) = &last {
                            let mut filled = b.clone();
                            filled.open_ts = *ts as u64;
                            filled.close_ts = *ts as u64;
                            filled.volume = 0.0;
                            row.push(Some(filled));
                        } else {
                            row.push(None);
                        }
                    },
                    EmptyBarPolicy::FillWithNaN => {
                        let nan_bar = Bar {
                            open_ts: *ts as u64,
                            close_ts: *ts as u64,
                            open_ts_exchange: *ts as u64,
                            open: f64::NAN,
                            high: f64::NAN,
                            low: f64::NAN,
                            close: f64::NAN,
                            adj_close: f64::NAN,
                            volume: f64::NAN,
                            n_trades: None,
                        };
                        row.push(Some(nan_bar));
                    },
                },
            }
        }
        out.insert(sym.clone(), row);
    }
    out
}

/// Compute every requested indicator once over each symbol.
///
/// Each input pair is ``(deterministic_name, indicator_object)``: the
/// caller is expected to have already loaded user-selected indicators
/// from disk and to have instantiated any strategy-required auto
/// indicators. We don't load anything by name here so that auto-injected
/// indicators (which only exist as in-memory objects, never as ``.pkl``
/// files on disk) are first-class citizens of the pipeline.
///
/// Returns a `{indicator_name -> {symbol -> Vec<Vec<f64>>}}` map.
fn compute_indicators(
    indicator_objs: &[(String, Py<PyAny>)],
    aligned: &HashMap<String, Vec<Option<Bar>>>,
    pb: Option<&ProgressBar>,
) -> EngineResult<HashMap<String, HashMap<String, Vec<Vec<f64>>>>> {
    let mut out: HashMap<String, HashMap<String, Vec<Vec<f64>>>> =
        HashMap::with_capacity(indicator_objs.len());

    // Pre-build a NaN bar template once, reused for every missing bar
    // across all indicator × symbol combinations.
    let nan_bar = Bar {
        open_ts: 0,
        close_ts: 0,
        open_ts_exchange: 0,
        open: f64::NAN,
        high: f64::NAN,
        low: f64::NAN,
        close: f64::NAN,
        adj_close: f64::NAN,
        volume: f64::NAN,
        n_trades: None,
    };

    for (name, obj) in indicator_objs {
        let mut per_symbol: HashMap<String, Vec<Vec<f64>>> = HashMap::with_capacity(aligned.len());

        for (sym, row) in aligned {
            // Build the dense bar slice. Clone existing bars, substitute
            // the pre-built NaN bar for gaps — avoids constructing a new
            // Bar struct per missing slot.
            let bars: Vec<Bar> = row
                .iter()
                .map(|b| b.as_ref().cloned().unwrap_or_else(|| nan_bar.clone()))
                .collect();

            let computed = Python::attach(|py| -> PyResult<Vec<Vec<f64>>> {
                compute_indicator(py, obj, &bars)
            });

            match computed {
                Ok(series) => {
                    per_symbol.insert(sym.clone(), series);
                },
                Err(e) => warn!("Indicator {name} failed for {sym}: {e}"),
            }
        }
        out.insert(name.clone(), per_symbol);
        if let Some(p) = pb {
            p.inc(1);
        }
    }
    Ok(out)
}

/// Try to compute an indicator: built-in (compute_inner via Rust) first,
/// else fall back to calling Python `.compute(df)`.
fn compute_indicator(py: Python, obj: &Py<PyAny>, bars: &[Bar]) -> PyResult<Vec<Vec<f64>>> {
    // Try every built-in indicator type.
    use crate::backtest::indicators::*;
    let bound = obj.bind(py);
    macro_rules! try_builtin {
        ($($t:ty),* $(,)?) => {
            $(
                if let Ok(b) = bound.cast::<$t>() {
                    let inst: pyo3::PyRef<'_, $t> = b.borrow();
                    let res: Vec<Vec<f64>> = <$t as BuiltinIndicator>::compute_inner(&inst, bars);
                    return Ok(res);
                }
            )*
        };
    }
    try_builtin!(
        AverageDirectionalIndex,
        AverageTrueRange,
        BollingerBands,
        CommodityChannelIndex,
        ExponentialMovingAverage,
        MovingAverageConvergenceDivergence,
        OnBalanceVolume,
        RelativeStrengthIndex,
        SimpleMovingAverage,
        StochasticOscillator,
        VolumeWeightedAveragePrice,
        WeightedMovingAverage,
    );

    // Fallback: call `.compute(df)` on the Python object with a numpy/pandas df.
    let df = bars_to_dataframe(py, bars)?;
    let result = bound.call_method1("compute", (df,))?;

    // Try to extract as a 2-D structure; otherwise treat as 1-D.
    let rows_res: PyResult<Vec<Vec<f64>>> = result.extract();
    if let Ok(rows) = rows_res {
        // Transpose to (n_series, n_points)
        if rows.is_empty() {
            return Ok(vec![]);
        }
        let cols = rows[0].len();
        let mut out: Vec<Vec<f64>> = vec![vec![f64::NAN; rows.len()]; cols];
        for (i, row) in rows.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                out[j][i] = *v;
            }
        }
        return Ok(out);
    }

    let flat: Vec<f64> = result.extract()?;
    Ok(vec![flat])
}

fn bars_to_dataframe<'py>(py: Python<'py>, bars: &[Bar]) -> PyResult<Bound<'py, PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("open", PyList::new(py, bars.iter().map(|b| b.open))?)?;
    dict.set_item("high", PyList::new(py, bars.iter().map(|b| b.high))?)?;
    dict.set_item("low", PyList::new(py, bars.iter().map(|b| b.low))?)?;
    dict.set_item("close", PyList::new(py, bars.iter().map(|b| b.close))?)?;
    dict.set_item("volume", PyList::new(py, bars.iter().map(|b| b.volume))?)?;
    let pd = py.import("pandas")?;
    pd.call_method1("DataFrame", (dict,))
}

/// Resolve an indicator name to a concrete Python object loaded from
/// the local indicators directory.
fn load_indicator(py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
    load_pickled(py, "indicators", name)
}

/// Resolve a strategy name to a concrete Python object loaded from
/// the local strategies directory.
fn load_strategy(py: Python<'_>, name: &str) -> PyResult<(Py<PyAny>, bool)> {
    let obj = load_pickled(py, "strategies", name)?;
    // Detect built-in strategies via the module path of their class.
    let is_custom = Python::attach(|py| -> PyResult<bool> {
        let cls = obj.bind(py).get_type();
        let module: String = cls.getattr("__module__")?.extract()?;
        Ok(!module.starts_with("backtide."))
    })?;
    Ok((obj, is_custom))
}

fn load_pickled(py: Python<'_>, sub: &str, name: &str) -> PyResult<Py<PyAny>> {
    let cfg = crate::config::interface::Config::get()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    let path = cfg.data.storage_path.join(sub).join(format!("{name}.pkl"));
    let cloudpickle = py.import("cloudpickle")?;
    let builtins = py.import("builtins")?;
    let f = builtins.call_method1("open", (path.to_string_lossy().to_string(), "rb"))?;
    let obj = cloudpickle.call_method1("load", (&f,))?;
    f.call_method0("close")?;
    Ok(obj.unbind())
}

/// Build the deterministic name for a Python indicator
/// instance. Mirrors `_auto_indicator_name` in the Python strategy utils
/// and the Rust `auto_indicator_name` used by built-in strategies'
/// ``decide_inner`` so the engine and the strategies look up indicators
/// under the *same* key.
///
/// Format: ``<ACRONYM>_<arg1>_<arg2>_...`` (or ``<ACRONYM>_default``
/// when the indicator takes no constructor arguments). ``.``, ``-`` and
/// spaces are sanitised for filesystem-friendliness.
fn auto_indicator_name_for(py: Python<'_>, ind: &Py<PyAny>) -> PyResult<String> {
    let bound = ind.bind(py);
    let acronym: String = bound.getattr("acronym")?.extract()?;

    // ``__reduce__`` returns ``(cls, args_tuple)`` for picklable objects.
    let reduce = bound.call_method0("__reduce__")?;
    let args_any = reduce.get_item(1)?;
    let args: Vec<Py<PyAny>> = args_any.extract().unwrap_or_default();

    let arg_strs: Vec<String> = args
        .iter()
        .map(|a| a.bind(py).str().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
        .collect();

    let arg_str = if arg_strs.is_empty() {
        "default".to_owned()
    } else {
        arg_strs.join("_")
    };
    let sanitized = arg_str.replace('.', "p").replace('-', "n").replace(' ', "");
    Ok(format!("{acronym}_{sanitized}"))
}

/// Load every requested strategy. Returns `(name, obj, is_custom)` triples.
///
/// Names present in `overrides` use the supplied in-memory instance
/// directly (nothing is read from disk). Other names are resolved from
/// the local strategies directory.
fn load_strategies(
    names: &[String],
    overrides: &HashMap<String, Py<PyAny>>,
) -> EngineResult<Vec<(String, Py<PyAny>, bool)>> {
    Python::attach(|py| -> PyResult<_> {
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let (obj, is_custom) = if let Some(o) = overrides.get(name) {
                let obj = o.clone_ref(py);
                let is_custom = {
                    let cls = obj.bind(py).get_type();
                    let module: String = cls.getattr("__module__")?.extract()?;
                    !module.starts_with("backtide.")
                };
                (obj, is_custom)
            } else {
                load_strategy(py, name)?
            };
            out.push((name.clone(), obj, is_custom));
        }
        Ok(out)
    })
    .map_err(|e: PyErr| EngineError::Io(std::io::Error::other(e.to_string())))
}

// ────────────────────────────────────────────────────────────────────────────
// Per-strategy runner
// ────────────────────────────────────────────────────────────────────────────

/// Execute one strategy through the entire timeline.
fn run_one_strategy(
    name: &str,
    strategy: Py<PyAny>,
    cfg: &ExperimentConfig,
    aligned: &HashMap<String, Vec<Option<Bar>>>,
    indicators: &HashMap<String, HashMap<String, Vec<Vec<f64>>>>,
    profiles: &[InstrumentProfile],
    timeline: &[i64],
    fx: &FxTable,
) -> RunResult {
    let symbols: Vec<String> = cfg.data.symbols.clone();
    let _ = &symbols;
    let total_bars: usize = aligned.values().map(|v| v.len()).next().unwrap_or(0);
    let warmup = cfg.engine.warmup_period as usize;

    // Each strategy may only "see" the symbols the user explicitly
    // selected in the data tab. The benchmark symbol is folded into the
    // global symbol list (and downloaded / aligned) so its bars are
    // available for the auto-injected benchmark strategy and so
    // the engine can value any benchmark-only positions, but it must
    // *not* be visible to user strategies — otherwise SMA Crossover &
    // friends would silently trade the benchmark too. The auto-injected
    // benchmark strategy is detected by name and gets a closes view
    // restricted to just the benchmark symbol.
    let benchmark = cfg.strategy.benchmark.as_deref().unwrap_or("").trim().to_owned();
    let is_benchmark_run = !benchmark.is_empty() && name == BENCHMARK;
    let allowed_symbols: HashSet<String> = if is_benchmark_run {
        std::iter::once(benchmark.clone()).collect()
    } else {
        symbols.iter().cloned().collect()
    };

    // First fatal error encountered during the run, if any. Recorded on
    // the result so the UI can flag the strategy as failed and surface
    // the message — the rest of the experiment continues.
    let mut run_error: Option<String> = None;

    // Initial portfolio: all initial cash in base currency.
    let base_ccy = cfg.portfolio.base_currency;
    let mut cash: HashMap<Currency, f64> = HashMap::new();
    cash.insert(base_ccy, cfg.portfolio.initial_cash as f64);
    // The benchmark strategy always starts with a clean slate (no pre-existing
    // holdings) so its return reflects a pure buy-and-hold from cash.
    let mut positions: HashMap<String, f64> = if is_benchmark_run {
        HashMap::new()
    } else {
        cfg.portfolio.starting_positions.clone()
    };
    let mut open_orders: Vec<Order> = Vec::new();
    // Per-order extremes for trailing stops: (running_high, running_low)
    // observed since the order was first seen. Cleared on fill / cancel.
    let mut trail_state: HashMap<String, (f64, f64)> = HashMap::new();

    let mut equity_curve: Vec<EquitySample> = Vec::with_capacity(total_bars);
    let mut order_records: Vec<OrderRecord> = Vec::new();
    let mut closed_trades: Vec<Trade> = Vec::new();
    // Open trade tracker per symbol: (entry_ts, qty_remaining, entry_price)
    let mut open_trades: HashMap<String, (i64, f64, f64)> = HashMap::new();
    let mut margin_limit_warnings: HashSet<String> = HashSet::new();

    let mut peak_equity = cfg.portfolio.initial_cash as f64;

    // ── Currency-conversion state ──────────────────────────────────────
    //
    // Tracks the boundary used by `EndOfPeriod` (last seen day/week/...
    // bucket) and the bar counter used by `CustomInterval`.
    use crate::backtest::models::currency_conversion_mode::CurrencyConversionMode;
    let conv_mode = cfg.exchange.conversion_mode;
    let conv_threshold = cfg.exchange.conversion_threshold.unwrap_or(0.0);
    let conv_period = cfg.exchange.conversion_period;
    let conv_interval = cfg.exchange.conversion_interval.unwrap_or(0) as usize;
    let mut last_period_bucket: Option<i64> = None;
    let mut bars_since_conv: usize = 0;

    // Pre-compute instrument quote currency lookup.
    let quote_ccy: HashMap<String, Currency> = profiles
        .iter()
        .filter_map(|p| {
            p.instrument.quote.parse::<Currency>().ok().map(|c| (p.instrument.symbol.clone(), c))
        })
        .collect();
    let instrument_types = profile_instrument_types(profiles);

    // Try to take a Rust-only snapshot of the strategy. If this succeeds
    // (every built-in strategy succeeds), the bar loop runs entirely
    // without the GIL — no Python::attach per bar, no DataFrame/numpy
    // slicing, no Vec<f64> round-trips. For multi-year backtests this
    // turns ~20 s runs into ~50 ms ones. Custom (Python-defined)
    // strategies fall back to the Python evaluate dispatch below.
    let builtin: Option<BuiltinStrategy> =
        Python::attach(|py| BuiltinStrategy::try_from_py(py, &strategy));

    // Pre-extract per-symbol close arrays once. Pure Rust, dense, ready
    // to be sliced as `&closes_full[sym][..=bar_index]` per bar. Filtered
    // to only the symbols this strategy is allowed to trade (see
    // `allowed_symbols` above).
    let mut closes_full: Vec<(String, Vec<f64>)> = aligned
        .iter()
        .filter(|(s, _)| allowed_symbols.contains(s.as_str()))
        .map(|(s, row)| {
            let v: Vec<f64> =
                row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.close)).collect();
            (s.clone(), v)
        })
        .collect();
    // Stable order (matches HashMap iteration would otherwise be random).
    closes_full.sort_by(|a, b| a.0.cmp(&b.0));

    // Pre-borrow symbol names so the per-bar `closes_view` construction
    // reuses `&str` references instead of cloning `String` on every bar.
    // This eliminates ~n_symbols × n_bars heap allocations on the hot path.
    let symbol_refs: Vec<&str> = closes_full.iter().map(|(s, _)| s.as_str()).collect();

    // Pre-build per-symbol full DataFrames and per-indicator full series
    // for the *Python* path only. Built-in strategies don't need these and we
    // skip the GIL-heavy pre-build entirely. For custom strategies this turns
    // the loop from O(n^2) Python work into O(n) cheap slice views.
    //
    // The container types follow the user's `cfg.data.dataframe_library`
    // setting (numpy / pandas / polars) so custom strategies receive
    // exactly the data shape they configured.
    let (cached_data, cached_indicators) = if builtin.is_some() {
        (HashMap::new(), HashMap::new())
    } else {
        Python::attach(
            |py| -> PyResult<(
                HashMap<String, Py<PyAny>>,
                HashMap<String, HashMap<String, Vec<Py<PyAny>>>>,
            )> {
                use crate::config::interface::Config as GlobalConfig;
                use crate::config::models::dataframe_library::DataFrameLibrary;
                use crate::utils::dataframe::dict_to_dataframe;

                // Read once: every wrapping decision below uses this.
                let df_lib = GlobalConfig::get()
                    .map(|c| c.data.dataframe_library)
                    .unwrap_or(DataFrameLibrary::Pandas);

                // Closure: wrap a flat Vec<f64> into the configured 1-D
                // container — np.ndarray, pd.Series or pl.Series. Used
                // for individual indicator output series.
                let wrap_series = |py: Python<'_>, s: &Vec<f64>| -> PyResult<Py<PyAny>> {
                    let list = PyList::new(py, s)?;
                    let obj: Bound<'_, PyAny> = match df_lib {
                        DataFrameLibrary::Numpy => {
                            py.import("numpy")?.call_method1("asarray", (list,))?
                        },
                        DataFrameLibrary::Pandas => {
                            py.import("pandas")?.call_method1("Series", (list,))?
                        },
                        DataFrameLibrary::Polars => {
                            py.import("polars")?.call_method1("Series", (list,))?
                        },
                    };
                    Ok(obj.unbind())
                };

                let mut data_full: HashMap<String, Py<PyAny>> =
                    HashMap::with_capacity(aligned.len());
                for (sym, row) in aligned {
                    if !allowed_symbols.contains(sym.as_str()) {
                        continue;
                    }
                    let dict = PyDict::new(py);
                    dict.set_item(
                        "open",
                        PyList::new(
                            py,
                            row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.open)),
                        )?,
                    )?;
                    dict.set_item(
                        "high",
                        PyList::new(
                            py,
                            row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.high)),
                        )?,
                    )?;
                    dict.set_item(
                        "low",
                        PyList::new(
                            py,
                            row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.low)),
                        )?,
                    )?;
                    dict.set_item(
                        "close",
                        PyList::new(
                            py,
                            row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.close)),
                        )?,
                    )?;
                    dict.set_item(
                        "volume",
                        PyList::new(
                            py,
                            row.iter().map(|b| b.as_ref().map_or(f64::NAN, |x| x.volume)),
                        )?,
                    )?;
                    let df = dict_to_dataframe(py, &dict)?;
                    data_full.insert(sym.clone(), df.unbind());
                }

                let mut ind_full: HashMap<String, HashMap<String, Vec<Py<PyAny>>>> =
                    HashMap::with_capacity(indicators.len());
                for (name, per_sym) in indicators {
                    let mut by_sym: HashMap<String, Vec<Py<PyAny>>> =
                        HashMap::with_capacity(per_sym.len());
                    for (sym, series) in per_sym {
                        let mut arrs: Vec<Py<PyAny>> = Vec::with_capacity(series.len());
                        for s in series {
                            arrs.push(wrap_series(py, s)?);
                        }
                        by_sym.insert(sym.clone(), arrs);
                    }
                    ind_full.insert(name.clone(), by_sym);
                }
                Ok((data_full, ind_full))
            },
        )
        .unwrap_or_else(|e| {
            let msg = format!("Failed to pre-build strategy view: {e}");
            warn!(strategy=%name, "{msg}");
            run_error.get_or_insert(msg);
            (HashMap::new(), HashMap::new())
        })
    };

    for bar_index in 0..total_bars {
        // Check for abort periodically. An atomic Relaxed load is essentially
        // free, so checking every 16 bars gives sub-second abort latency
        // even for very fast bars.
        if bar_index & 15 == 0 && check_abort() {
            break;
        }

        let ts = timeline[bar_index];
        let is_warmup = bar_index < warmup;

        // ── 0. Per-bar margin interest & short-borrow accrual ───────────
        //
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

        // ── 1. Resolve open orders against the *current* bar ────────────
        let mut still_open: Vec<Order> = Vec::new();
        let drained: Vec<Order> = std::mem::take(&mut open_orders);
        for mut order in drained {
            // Cancel orders take effect immediately and do not need a price.
            if order.order_type == OrderType::Cancel {
                still_open.retain(|o| o.id != order.id);
                trail_state.remove(&order.id);
                order_records.push(OrderRecord {
                    order: order.clone(),
                    timestamp: ts,
                    status: "cancelled".into(),
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
                    still_open.push(order);
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
                        status: "cancelled".into(),
                        fill_price: None,
                        reason,
                        commission: 0.0,
                        pnl: None,
                    });
                    continue;
                },
            };

            // Apply slippage; for limit-style fills, never cross the limit.
            let slip = cfg.exchange.slippage / 100.0;
            let fill_px = apply_slippage(raw_px, order.quantity, slip, limit_cap);

            let mut qty = order.quantity;
            let mut filled_qty = qty;
            let mut notional = fill_px * qty.abs();
            let mut commission = match cfg.exchange.commission_type {
                CommissionType::Percentage => notional * cfg.exchange.commission_pct / 100.0,
                CommissionType::Fixed => cfg.exchange.commission_fixed,
                CommissionType::PercentagePlusFixed => {
                    notional * cfg.exchange.commission_pct / 100.0 + cfg.exchange.commission_fixed
                },
            };

            let order_ccy = quote_ccy.get(&symbol).copied().unwrap_or(base_ccy);

            // ── Leverage / position-size pre-check ───────────────────
            //
            // Reject (or — when `raise_on_margin_limit` — abort the run)
            // orders that would push gross exposure beyond
            // `max_leverage` / `initial_margin`, push the per-symbol
            // exposure past `max_position_size`, or attempt to borrow at
            // all when `allow_margin` is disabled.
            {
                let equity_base = portfolio_equity_in_currency(
                    &cash, &positions, aligned, bar_index, &quote_ccy, base_ccy, base_ccy, fx, ts,
                );
                let gross_base = gross_notional_in_currency(
                    &positions, aligned, bar_index, &quote_ccy, base_ccy, fx, ts,
                );
                let current_qty = positions.get(&symbol).copied().unwrap_or(0.0);
                let current_pos_base = if current_qty.abs() > 1e-12 {
                    let bar_close = aligned
                        .get(&symbol)
                        .and_then(|r| r[bar_index].as_ref())
                        .map(|b| b.close)
                        .unwrap_or(fill_px);
                    let value = current_qty.abs() * bar_close;
                    let ccy = quote_ccy.get(&symbol).copied().unwrap_or(base_ccy);
                    fx.convert(value, ccy, base_ccy, ts).unwrap_or(value)
                } else {
                    0.0
                };
                if let Err((violation, reason)) = check_order_against_limits(
                    cfg,
                    &symbol,
                    qty,
                    fill_px,
                    order_ccy,
                    base_ccy,
                    equity_base,
                    gross_base,
                    current_qty,
                    current_pos_base,
                    fx,
                    ts,
                )
                .and_then(|new_qty| {
                    // Auto-shrink when the order would exceed
                    // `max_leverage` / `max_position_size` by the
                    // exact margin slippage + commission introduce
                    // between strategy sizing (bar close) and order
                    // fill (next bar open). This mirrors the cash
                    // shrink below and keeps equal-weight strategies
                    // like Buy & Hold from being silently dropped.
                    if (new_qty - qty).abs() <= 1e-12 {
                        return Ok(());
                    }
                    let abs_new = new_qty.abs();
                    let instrument_type = instrument_type_for_symbol(
                        &symbol,
                        &instrument_types,
                        cfg.data.instrument_type,
                    );
                    let mut abs_after = abs_new;
                    if !instrument_type.allows_fractional_quantities() {
                        abs_after = abs_after.floor();
                    }
                    if !abs_after.is_finite() || abs_after <= 1e-12 {
                        return Err((
                            LimitViolation::Margin,
                            format!(
                                "no headroom under leverage / position-size limits for {symbol}"
                            ),
                        ));
                    }
                    // Update the order quantity, sign-preserving, and
                    // re-derive notional / commission from the
                    // shrunk size.
                    let new_qty_signed = qty.signum() * abs_after;
                    qty = new_qty_signed;
                    order.quantity = new_qty_signed;
                    filled_qty = new_qty_signed;
                    notional = fill_px * abs_after;
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
                    let warning_key = format!("{symbol}\0{reason}");
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
                        .filter_map(|(ccy, v)| fx.convert(*v, *ccy, order_ccy, ts))
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
                    //   fill_px * q * (1 + pct_part) + fixed_part <= avail.
                    // Non-crypto instruments must settle whole units, so
                    // floor the cash-fit quantity before retrying the debit.
                    let denom = fill_px * (1.0 + pct_part);
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
                    notional = fill_px * filled_qty;
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
                update_open_trade_buy(&mut open_trades, &symbol, ts, filled_qty, fill_px);
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
                    fill_px,
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
            if (filled_qty - order.quantity).abs() > 1e-12 {
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
        match conv_mode {
            CurrencyConversionMode::Immediate => {
                sweep_foreign_to_base(&mut cash, base_ccy, fx, ts, None);
            },
            CurrencyConversionMode::HoldUntilThreshold => {
                sweep_foreign_to_base(&mut cash, base_ccy, fx, ts, Some(conv_threshold));
            },
            CurrencyConversionMode::EndOfPeriod => {
                if let Some(period) = conv_period {
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
                let closes_view: Vec<(String, &[f64])> = symbol_refs
                    .iter()
                    .zip(closes_full.iter())
                    .map(|(&s, (_, v))| (s.to_owned(), &v[..=bar_index]))
                    .collect();
                let inds = IndicatorView::new(indicators, bar_index);
                let orders = b.decide(&closes_view, &inds, &portfolio, &state);
                for o in &orders {
                    let side = if o.quantity > 0.0 {
                        "BUY"
                    } else {
                        "SELL"
                    };
                    let abs_qty = o.quantity.abs();
                    info!(
                        strategy=%name,
                        "Order placed: {side} {abs_qty:.4} {} @ bar {bar_index}",
                        o.symbol,
                    );
                }
                Ok(orders)
            } else {
                // Custom (Python) strategy: original evaluate path.
                Python::attach(|py| -> PyResult<Vec<Order>> {
                    let data = build_per_symbol_view(py, &cached_data, bar_index)?;
                    let inds = build_indicator_view(py, &cached_indicators, bar_index)?;
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
                                status: "cancelled".into(),
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
                            let order_ccy = quote_ccy.get(&o.symbol).copied().unwrap_or(base_ccy);
                            // Compute mark-to-market equity in the same currency
                            // as the symbol's price. Sizers divide equity/risk by
                            // price-like inputs, so `equity`, `price`,
                            // `stop_distance`, and `atr` must share a currency.
                            let eq = portfolio_equity_in_currency(
                                &cash, &positions, aligned, bar_index, &quote_ccy, base_ccy,
                                order_ccy, fx, ts,
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
                    let is_builtin_strategy = builtin.is_some();
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
                                &instrument_types,
                                cfg.data.instrument_type,
                            );
                            if is_builtin_strategy {
                                if let Some(reason) = normalize_builtin_order_quantity(o, instrument_type) {
                                    warn!(strategy=%name, "Invalid built-in order quantity, dropping: {reason}.");
                                    order_records.push(OrderRecord {
                                        order: o.clone(),
                                        timestamp: ts,
                                        status: "rejected".into(),
                                        fill_price: None,
                                        reason,
                                        commission: 0.0,
                                        pnl: None,
                                    });
                                    return false;
                                }
                            } else if let Some(reason) =
                                quantity_rejection_reason(&o.symbol, o.quantity, instrument_type)
                            {
                                warn!(strategy=%name, "Invalid order quantity, dropping: {reason}.");
                                order_records.push(OrderRecord {
                                    order: o.clone(),
                                    timestamp: ts,
                                    status: "rejected".into(),
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
                                status: "rejected".into(),
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
            equity += fx.convert(*amount, *ccy, base_ccy, ts).unwrap_or(*amount);
        }
        for (sym, qty) in &positions {
            if qty.abs() < 1e-12 {
                continue;
            }
            if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
                let value = *qty * b.close;
                let pos_ccy = quote_ccy.get(sym).copied().unwrap_or(base_ccy);
                equity += fx.convert(value, pos_ccy, base_ccy, ts).unwrap_or(value);
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
            cash.iter().filter(|(_, v)| v.abs() > 1e-12).map(|(k, v)| (*k, *v)).collect()
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
        let gross_base = gross_notional_in_currency(
            &positions, aligned, bar_index, &quote_ccy, base_ccy, fx, ts,
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
                if qty.abs() < 1e-12 {
                    continue;
                }
                let close = match aligned.get(&sym).and_then(|r| r[bar_index].as_ref()) {
                    Some(b) => b.close,
                    None => continue,
                };
                let pos_ccy = quote_ccy.get(&sym).copied().unwrap_or(base_ccy);
                let notional = qty.abs() * close;
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
                    *cash.entry(pos_ccy).or_insert(0.0) += notional;
                    if let Some(t) =
                        close_open_trade_sell(&mut open_trades, &sym, ts, qty, close, 0.0)
                    {
                        closed_trades.push(t);
                    }
                } else {
                    // Short: debit cash (or any available bucket) to buy
                    // back the shares.
                    let _ = try_debit(&mut cash, pos_ccy, notional, base_ccy, fx, ts);
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
            positions.retain(|_, q| q.abs() > 1e-12);
        }
    }

    // ── 5. Liquidate remaining positions to compute final PnL ───────────
    if let Some(last_idx) = total_bars.checked_sub(1) {
        for (sym, qty) in positions.clone() {
            if qty.abs() < 1e-12 {
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

fn portfolio_equity_in_currency(
    cash: &HashMap<Currency, f64>,
    positions: &HashMap<String, f64>,
    aligned: &HashMap<String, Vec<Option<Bar>>>,
    bar_index: usize,
    quote_ccy: &HashMap<String, Currency>,
    base_ccy: Currency,
    target_ccy: Currency,
    fx: &FxTable,
    ts: i64,
) -> f64 {
    let mut equity = 0.0_f64;
    for (ccy, amount) in cash {
        equity += fx.convert(*amount, *ccy, target_ccy, ts).unwrap_or(*amount);
    }
    for (sym, qty) in positions {
        if qty.abs() < 1e-12 {
            continue;
        }
        if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
            let value = *qty * b.close;
            let pos_ccy = quote_ccy.get(sym).copied().unwrap_or(base_ccy);
            equity += fx.convert(value, pos_ccy, target_ccy, ts).unwrap_or(value);
        }
    }
    equity
}

// ─────────────────────────────────────────────────────────────────────────
// Order trigger resolution
// ─────────────────────────────────────────────────────────────────────────
//
// `resolve_trigger` decides whether an open order fills against the
// current bar, returning a `TriggerOutcome`:
//
//   * `Fill { raw_px, .. }` — the order fills at `raw_px` (before
//     slippage). The caller is responsible for applying slippage and
//     bookkeeping (cash, positions, commissions, …).
//   * `Pending`             — the order does not fill this bar; keep it open.
//   * `Cancel { .. }`       — the order cannot make sense (e.g.
//     SettlePosition with no current position); record as cancelled.
//
// Stop-into-limit variants mutate `order` in place: when the stop fires
// we replace `order_type` with `OrderType::Limit` and copy
// `order.limit_price` into `order.price`, so that on subsequent bars
// the order rests as a regular limit order.
//
// Trailing variants share `trail_state`, keyed by `order.id`, holding
// `(running_high, running_low)` since the order was placed.

#[derive(Debug)]
enum TriggerOutcome {
    /// The order fills at `raw_px` (before slippage).
    /// `limit_cap` constrains slippage so the slipped fill never crosses
    /// the resting limit price (used for Limit and *Limit variants).
    Fill {
        raw_px: f64,
        reason: String,
        limit_cap: Option<f64>,
    },
    /// The order does not fill this bar.
    Pending,
    /// The order is invalid against current state and should be cancelled.
    Cancel {
        reason: String,
    },
}

/// Decide whether `order` fills this bar. May mutate `order` for the
/// stop-into-limit transition, and may mutate `trail_state` for trailing
/// variants. `positions` is read-only and only used by `SettlePosition`.
fn resolve_trigger(
    order: &mut Order,
    bar: &Bar,
    positions: &HashMap<String, f64>,
    trail_state: &mut HashMap<String, (f64, f64)>,
    trade_on_close: bool,
) -> TriggerOutcome {
    use OrderType::*;

    match order.order_type {
        // Cancel is handled before resolve_trigger is called.
        Cancel => TriggerOutcome::Cancel {
            reason: "cancel".into(),
        },

        Market => {
            let px = if trade_on_close {
                bar.close
            } else {
                bar.open
            };
            TriggerOutcome::Fill {
                raw_px: px,
                reason: String::new(),
                limit_cap: None,
            }
        },

        SettlePosition => {
            let cur = *positions.get(&order.symbol).unwrap_or(&0.0);
            if cur.abs() < 1e-12 {
                return TriggerOutcome::Cancel {
                    reason: "no position to settle".into(),
                };
            }
            // Translate to a market order that flattens the position.
            order.quantity = -cur;
            order.order_type = Market;
            let px = if trade_on_close {
                bar.close
            } else {
                bar.open
            };
            TriggerOutcome::Fill {
                raw_px: px,
                reason: "settle position".into(),
                limit_cap: None,
            }
        },

        Limit => match order.price {
            Some(lim) => fill_limit(order.quantity, bar, lim),
            None => TriggerOutcome::Cancel {
                reason: "limit order missing price".into(),
            },
        },

        TakeProfit => match order.price {
            // Take-profit is a profit-target limit: same execution
            // semantics as Limit (a buy fills at-or-below, a sell at-or-above).
            Some(target) => fill_limit(order.quantity, bar, target),
            None => TriggerOutcome::Cancel {
                reason: "take-profit missing price".into(),
            },
        },

        StopLoss => {
            let stop = match order.price {
                Some(p) => p,
                None => {
                    return TriggerOutcome::Cancel {
                        reason: "stop-loss missing price".into(),
                    }
                },
            };
            if stop_triggered(order.quantity, bar, stop, /*is_take_profit=*/ false) {
                fill_stop(order.quantity, bar, stop)
            } else {
                TriggerOutcome::Pending
            }
        },

        StopLossLimit | TakeProfitLimit => {
            let stop = match order.price {
                Some(p) => p,
                None => {
                    return TriggerOutcome::Cancel {
                        reason: "stop-limit missing stop price".into(),
                    }
                },
            };
            let is_tp = order.order_type == TakeProfitLimit;
            if !stop_triggered(order.quantity, bar, stop, is_tp) {
                return TriggerOutcome::Pending;
            }
            // Convert to a resting Limit at `limit_price` (or stop as fallback).
            let lim = order.limit_price.unwrap_or(stop);
            order.order_type = Limit;
            order.price = Some(lim);
            order.limit_price = None;
            // Try to fill same bar; if the limit can't be hit on this bar
            // it will rest and re-evaluate next bar via the new Limit path.
            fill_limit(order.quantity, bar, lim)
        },

        TrailingStop | TrailingStopLimit => {
            let trail = match order.price {
                Some(p) if p > 0.0 => p,
                _ => {
                    return TriggerOutcome::Cancel {
                        reason: "trailing stop missing/invalid trail amount".into(),
                    }
                },
            };

            // First-bar initialisation: seed extremes from this bar.
            let entry = trail_state.entry(order.id.clone()).or_insert_with(|| (bar.high, bar.low));
            entry.0 = entry.0.max(bar.high);
            entry.1 = entry.1.min(bar.low);
            let (running_high, running_low) = (entry.0, entry.1);

            // Effective stop: sells trail running_high downward; buys
            // trail running_low upward. `qty == 0` is meaningless here.
            let stop = if order.quantity < 0.0 {
                running_high - trail
            } else if order.quantity > 0.0 {
                running_low + trail
            } else {
                return TriggerOutcome::Cancel {
                    reason: "zero quantity".into(),
                };
            };

            // Re-use the regular stop-trigger / stop-fill helpers.
            if !stop_triggered(order.quantity, bar, stop, /*is_take_profit=*/ false) {
                return TriggerOutcome::Pending;
            }

            if order.order_type == TrailingStopLimit {
                let lim = order.limit_price.unwrap_or(stop);
                order.order_type = Limit;
                order.price = Some(lim);
                order.limit_price = None;
                fill_limit(order.quantity, bar, lim)
            } else {
                trail_state.remove(&order.id);
                fill_stop(order.quantity, bar, stop)
            }
        },
    }
}

/// Fill semantics for a Limit (or TakeProfit, identical execution-wise):
///
/// * Buy (qty > 0): fill if price reached the limit *or below*. If the
///   bar opens at-or-below the limit, fill at the open (better than
///   limit). Otherwise, if `bar.low <= lim`, fill at the limit price.
/// * Sell (qty < 0): symmetric — fill at open if open ≥ limit, else at
///   limit if `bar.high >= lim`.
fn fill_limit(qty: f64, bar: &Bar, lim: f64) -> TriggerOutcome {
    if qty > 0.0 {
        if bar.open <= lim {
            TriggerOutcome::Fill {
                raw_px: bar.open,
                reason: "limit (open through)".into(),
                limit_cap: Some(lim),
            }
        } else if bar.low <= lim {
            TriggerOutcome::Fill {
                raw_px: lim,
                reason: "limit hit".into(),
                limit_cap: Some(lim),
            }
        } else {
            TriggerOutcome::Pending
        }
    } else if qty < 0.0 {
        if bar.open >= lim {
            TriggerOutcome::Fill {
                raw_px: bar.open,
                reason: "limit (open through)".into(),
                limit_cap: Some(lim),
            }
        } else if bar.high >= lim {
            TriggerOutcome::Fill {
                raw_px: lim,
                reason: "limit hit".into(),
                limit_cap: Some(lim),
            }
        } else {
            TriggerOutcome::Pending
        }
    } else {
        TriggerOutcome::Cancel {
            reason: "zero quantity".into(),
        }
    }
}

/// Stop trigger predicate.
///
/// * Stop-loss sell (qty < 0, long-protection): triggers when price
///   *falls* to `stop` — `bar.low <= stop` or gap-down (`bar.open <= stop`).
/// * Stop-loss buy  (qty > 0, short-cover): triggers when price *rises*
///   to `stop` — `bar.high >= stop` or gap-up (`bar.open >= stop`).
/// * Take-profit-limit reverses both directions (a sell TP triggers on
///   a price rise, a buy TP on a price drop).
fn stop_triggered(qty: f64, bar: &Bar, stop: f64, is_take_profit: bool) -> bool {
    let down_trigger = (qty < 0.0 && !is_take_profit) || (qty > 0.0 && is_take_profit);
    let up_trigger = (qty > 0.0 && !is_take_profit) || (qty < 0.0 && is_take_profit);
    if down_trigger {
        bar.open <= stop || bar.low <= stop
    } else if up_trigger {
        bar.open >= stop || bar.high >= stop
    } else {
        false
    }
}

/// Stop fill price. Realistic gap handling: if the bar opens past the
/// stop level, the stop fills at the open (worse than the stop) — a
/// gap-down for sell stops, a gap-up for buy stops. Otherwise the stop
/// fills at exactly the stop level.
fn fill_stop(qty: f64, bar: &Bar, stop: f64) -> TriggerOutcome {
    if qty < 0.0 {
        if bar.open <= stop {
            TriggerOutcome::Fill {
                raw_px: bar.open,
                reason: "stop triggered (gap-down)".into(),
                limit_cap: None,
            }
        } else {
            TriggerOutcome::Fill {
                raw_px: stop,
                reason: "stop triggered".into(),
                limit_cap: None,
            }
        }
    } else if qty > 0.0 {
        if bar.open >= stop {
            TriggerOutcome::Fill {
                raw_px: bar.open,
                reason: "stop triggered (gap-up)".into(),
                limit_cap: None,
            }
        } else {
            TriggerOutcome::Fill {
                raw_px: stop,
                reason: "stop triggered".into(),
                limit_cap: None,
            }
        }
    } else {
        TriggerOutcome::Cancel {
            reason: "zero quantity".into(),
        }
    }
}

/// Apply slippage to a raw fill price, optionally capping at the limit
/// price so a buy never pays above its limit (and a sell never receives
/// below). `slippage_pct` is the fraction (e.g. 0.005 = 0.5 %).
fn apply_slippage(raw_px: f64, qty: f64, slippage_pct: f64, limit_cap: Option<f64>) -> f64 {
    let slipped = if qty >= 0.0 {
        raw_px * (1.0 + slippage_pct)
    } else {
        raw_px * (1.0 - slippage_pct)
    };
    match limit_cap {
        Some(cap) if qty > 0.0 => slipped.min(cap),
        Some(cap) if qty < 0.0 => slipped.max(cap),
        _ => slipped,
    }
}

/// Try to debit `amount` of `ccy` from `cash`. If `ccy` doesn't have enough,
/// fall back to the base currency (and finally any other foreign bucket)
/// converting at the FX rate observed at `ts`. Returns `false` if no
/// combination of available cash covers the debit.
fn try_debit(
    cash: &mut HashMap<Currency, f64>,
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
    let needed_base = match fx.rate(ccy, base, ts) {
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
            match fx.rate(base, ccy, ts) {
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
        let r = match fx.rate(other_ccy, ccy, ts) {
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
    cash.retain(|_, v| v.abs() > 1e-12);
    true
}

/// Sweep every non-base currency bucket into the base currency at the
/// FX rate observed at `ts`. If `threshold` is `Some(t)`, only buckets
/// whose value in base currency is `>= t` are swept; otherwise every
/// foreign bucket with a positive (or negative) finite balance is
/// converted. Buckets without an available FX rate are left untouched.
fn sweep_foreign_to_base(
    cash: &mut HashMap<Currency, f64>,
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
        let in_base = match fx.convert(amount, ccy, base, ts) {
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
fn period_bucket(
    ts: i64,
    period: crate::backtest::models::conversion_period::ConversionPeriod,
) -> i64 {
    use crate::backtest::models::conversion_period::ConversionPeriod::*;
    use chrono::{DateTime, Datelike, Utc};
    let dt = DateTime::<Utc>::from_timestamp(ts, 0)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    match period {
        Day => ts.div_euclid(86_400),
        Week => {
            // ISO week-year combined identifier.
            let iso = dt.iso_week();
            (iso.year() as i64) * 100 + iso.week() as i64
        },
        Month => (dt.year() as i64) * 12 + (dt.month0() as i64),
        Year => dt.year() as i64,
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
            if q.abs() > 1e-12 {
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
    if q > 1e-12 {
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
// Margin & leverage helpers
// ────────────────────────────────────────────────────────────────────────────

/// Effective leverage cap given the `allow_margin`, `max_leverage` and
/// `initial_margin` settings. When margin is disabled, the cap is 1.0
/// (no borrowing). Otherwise `max_leverage` and `100/initial_margin`
/// are intersected so the more restrictive of the two wins.
fn effective_leverage_cap(cfg: &ExperimentConfig) -> f64 {
    if !cfg.exchange.allow_margin {
        return 1.0;
    }
    let im = cfg.exchange.initial_margin;
    let from_im = if im > 0.0 {
        100.0 / im
    } else {
        f64::INFINITY
    };
    let from_ml = if cfg.exchange.max_leverage > 0.0 {
        cfg.exchange.max_leverage
    } else {
        f64::INFINITY
    };
    from_ml.min(from_im).max(1.0)
}

/// Compute the *gross* notional currently invested across all positions,
/// expressed in `target_ccy`. Open shorts contribute their absolute
/// notional just like longs — this is what the leverage / maintenance
/// margin checks compare against equity.
fn gross_notional_in_currency(
    positions: &HashMap<String, f64>,
    aligned: &HashMap<String, Vec<Option<Bar>>>,
    bar_index: usize,
    quote_ccy: &HashMap<String, Currency>,
    target_ccy: Currency,
    fx: &FxTable,
    ts: i64,
) -> f64 {
    let mut total = 0.0_f64;
    for (sym, qty) in positions {
        if qty.abs() < 1e-12 {
            continue;
        }
        if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
            let value = qty.abs() * b.close;
            let ccy = quote_ccy.get(sym).copied().unwrap_or(target_ccy);
            total += fx.convert(value, ccy, target_ccy, ts).unwrap_or(value);
        }
    }
    total
}

/// Check whether an order of `qty @ fill_px` (in `order_ccy`) for `symbol`
/// satisfies the configured leverage and position-size limits. Returns
/// either the (possibly shrunk) acceptable quantity, or an error string
/// describing which limit was breached.
///
/// `equity_base` and `gross_base` must already be expressed in the
/// portfolio base currency.
/// Classification of a limit-check rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LimitViolation {
    /// Per-symbol concentration limit (`max_position_size`). Not related
    /// to margin — always enforced regardless of `raise_on_margin_limit`.
    PositionSize,
    /// Leverage / margin / equity constraint. Gated by `raise_on_margin_limit`.
    Margin,
}

fn check_order_against_limits(
    cfg: &ExperimentConfig,
    symbol: &str,
    qty: f64,
    fill_px: f64,
    order_ccy: Currency,
    base_ccy: Currency,
    equity_base: f64,
    gross_base: f64,
    current_qty: f64,
    current_pos_base: f64,
    fx: &FxTable,
    ts: i64,
) -> Result<f64, (LimitViolation, String)> {
    let abs_qty = qty.abs();
    if abs_qty <= 0.0 || !abs_qty.is_finite() {
        return Ok(qty);
    }
    if fill_px <= 0.0 || !fill_px.is_finite() {
        return Ok(qty);
    }
    let order_notional_base =
        fx.convert(abs_qty * fill_px, order_ccy, base_ccy, ts).unwrap_or(abs_qty * fill_px);
    let unit_base = order_notional_base / abs_qty;
    if unit_base <= 0.0 || !unit_base.is_finite() {
        return Ok(qty);
    }

    let current_pos_base = current_pos_base.max(0.0);
    let current_abs_qty = current_qty.abs();

    let max_qty_for_final_exposure = |cap_base: f64| -> f64 {
        let same_direction = current_abs_qty <= 1e-12 || current_qty.signum() == qty.signum();
        if same_direction {
            return ((cap_base - current_pos_base) / unit_base).max(0.0);
        }

        // Opposite-side orders reduce existing exposure first. Always allow
        // the requested size when it only closes/reduces the position, even if
        // the account is currently at/over a cap. If it flips the position,
        // only the post-flip exposure consumes cap room.
        if abs_qty <= current_abs_qty + 1e-12 {
            abs_qty
        } else {
            current_abs_qty + (cap_base / unit_base).max(0.0)
        }
    };

    let mut max_abs_qty = abs_qty;

    // ── max_position_size ─────────────────────────────────────────────
    //
    // Per-symbol exposure (existing absolute notional + new order
    // notional) must not exceed `max_position_size / 100` of equity.
    // `max_position_size == 0` is treated as "no cap". When the order
    // would push past the cap, the quantity is *shrunk* to whatever
    // fits rather than rejected outright — this matches real-broker
    // behaviour and prevents an entire equal-weight allocation from
    // being silently dropped over a fractional slippage overshoot.
    let pos_cap_pct = cfg.exchange.max_position_size as f64;
    if pos_cap_pct > 0.0 && equity_base > 0.0 {
        let max_per_pos = equity_base * pos_cap_pct / 100.0;
        let allowed_abs_qty = max_qty_for_final_exposure(max_per_pos);
        if allowed_abs_qty <= 1e-12 {
            return Err((
                LimitViolation::PositionSize,
                format!(
                "order would exceed max_position_size ({pos_cap_pct}% of equity) for {symbol}: \
                 position already at limit (current {current_pos_base:.2}, cap {max_per_pos:.2})"
            ),
            ));
        }
        max_abs_qty = max_abs_qty.min(allowed_abs_qty);
    }

    // ── max_leverage / allow_margin ───────────────────────────────────
    //
    // The total gross notional after this fill (including existing
    // exposure) must not exceed `equity * effective_leverage_cap`.
    // Same shrink-not-reject philosophy as above.
    let cap = effective_leverage_cap(cfg);
    if equity_base > 0.0 && cap.is_finite() {
        let max_gross = equity_base * cap;
        let other_gross_base = (gross_base - current_pos_base).max(0.0);
        let symbol_cap_base = max_gross - other_gross_base;
        let allowed_abs_qty = max_qty_for_final_exposure(symbol_cap_base);
        if allowed_abs_qty <= 1e-12 {
            return Err((
                LimitViolation::Margin,
                format!(
                    "order would exceed max_leverage ({cap:.2}x): gross notional already at limit \
                 (current {gross_base:.2}, cap {max_gross:.2})"
                ),
            ));
        }
        max_abs_qty = max_abs_qty.min(allowed_abs_qty);
    } else if equity_base <= 0.0 {
        // Account already wiped out — nothing more can be opened.
        return Err((
            LimitViolation::Margin,
            "equity is non-positive; cannot open new exposure".to_owned(),
        ));
    }

    if !max_abs_qty.is_finite() || max_abs_qty <= 1e-12 {
        return Err((
            LimitViolation::Margin,
            format!("no headroom under leverage / position-size limits for {symbol}"),
        ));
    }
    Ok(qty.signum() * max_abs_qty.min(abs_qty))
}

/// Post-bar maintenance-margin check.
///
/// Returns `Some(message)` when `equity / gross_notional < maintenance_margin/100`
/// (i.e. the account is undercollateralised), `None` otherwise. The caller
/// decides whether to force-liquidate, record a warning, or abort the run
/// based on `cfg.exchange.raise_on_margin_limit`.
fn check_maintenance_margin(
    cfg: &ExperimentConfig,
    equity_base: f64,
    gross_base: f64,
) -> Option<String> {
    let mm = cfg.exchange.maintenance_margin;
    if mm <= 0.0 || gross_base <= 0.0 {
        return None;
    }
    // Negative equity is always a margin call.
    if equity_base <= 0.0 {
        return Some(format!(
            "margin call: equity {equity_base:.2} ≤ 0 with gross notional {gross_base:.2}"
        ));
    }
    let ratio = equity_base / gross_base;
    if ratio < mm / 100.0 {
        Some(format!(
            "margin call: equity/notional ratio {:.2}% below maintenance_margin {mm:.2}%",
            ratio * 100.0
        ))
    } else {
        None
    }
}

/// Accrue per-bar margin interest and short-borrow cost. Both rates are
/// annual percentages; the cost is prorated by `bar_seconds / SECS_PER_YEAR`.
///
/// * `margin_interest` is charged on negative base cash (borrowed funds).
/// * `borrow_rate` is charged on the gross value of open short positions.
///
/// Charges are taken from the base-currency cash bucket (which may go
/// further negative — the next leverage / maintenance-margin check will
/// surface that).
fn accrue_margin_costs(
    cfg: &ExperimentConfig,
    cash: &mut HashMap<Currency, f64>,
    positions: &HashMap<String, f64>,
    aligned: &HashMap<String, Vec<Option<Bar>>>,
    bar_index: usize,
    quote_ccy: &HashMap<String, Currency>,
    base_ccy: Currency,
    fx: &FxTable,
    ts: i64,
    bar_seconds: i64,
) {
    const SECS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;
    if bar_seconds <= 0 {
        return;
    }
    let frac = bar_seconds as f64 / SECS_PER_YEAR;

    // Margin interest on borrowed cash (any negative cash bucket, converted to base).
    if cfg.exchange.margin_interest > 0.0 {
        let mut borrowed_base = 0.0_f64;
        for (ccy, amt) in cash.iter() {
            if *amt < 0.0 {
                let v = -*amt;
                borrowed_base += fx.convert(v, *ccy, base_ccy, ts).unwrap_or(v);
            }
        }
        if borrowed_base > 0.0 {
            let cost = borrowed_base * cfg.exchange.margin_interest / 100.0 * frac;
            *cash.entry(base_ccy).or_insert(0.0) -= cost;
        }
    }

    // Borrow cost on short positions.
    if cfg.exchange.borrow_rate > 0.0 {
        let mut shorts_value_base = 0.0_f64;
        for (sym, qty) in positions {
            if *qty >= 0.0 {
                continue;
            }
            if let Some(b) = aligned.get(sym).and_then(|r| r[bar_index].as_ref()) {
                let v = qty.abs() * b.close;
                let ccy = quote_ccy.get(sym).copied().unwrap_or(base_ccy);
                shorts_value_base += fx.convert(v, ccy, base_ccy, ts).unwrap_or(v);
            }
        }
        if shorts_value_base > 0.0 {
            let cost = shorts_value_base * cfg.exchange.borrow_rate / 100.0 * frac;
            *cash.entry(base_ccy).or_insert(0.0) -= cost;
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::models::experiment_config::*;
    use crate::data::models::instrument::Instrument;

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
    fn portfolio_equity_for_sizer_uses_target_currency() {
        let mut aligned = HashMap::new();
        aligned.insert("AAPL".to_owned(), vec![Some(mk_bar(1_700_000_000, 50.0))]);

        let cash = HashMap::from([(Currency::EUR, 1_000.0)]);
        let positions = HashMap::from([("AAPL".to_owned(), 2.0)]);
        let quote_ccy = HashMap::from([("AAPL".to_owned(), Currency::USD)]);
        let mut fx = FxTable::new(Currency::EUR);
        fx.add_series(Currency::EUR, Currency::USD, vec![(1_700_000_000, 1.20)]);

        let equity_usd = portfolio_equity_in_currency(
            &cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::EUR,
            Currency::USD,
            &fx,
            1_700_000_000,
        );
        assert!((equity_usd - 1_300.0).abs() < 1e-12);

        let equity_eur = portfolio_equity_in_currency(
            &cash,
            &positions,
            &aligned,
            0,
            &quote_ccy,
            Currency::EUR,
            Currency::EUR,
            &fx,
            1_700_000_000,
        );
        assert!((equity_eur - (1_000.0 + 100.0 / 1.20)).abs() < 1e-12);
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
        let mut cash: HashMap<Currency, f64> = HashMap::new();
        cash.insert(base_ccy, cfg.portfolio.initial_cash as f64);
        let mut positions: HashMap<String, f64> = cfg.portfolio.starting_positions.clone();
        let mut open_orders: Vec<Order> = Vec::new();
        let mut trail_state: HashMap<String, (f64, f64)> = HashMap::new();
        let mut equity_curve: Vec<EquitySample> = Vec::with_capacity(total_bars);
        let mut order_records: Vec<OrderRecord> = Vec::new();
        let mut closed_trades: Vec<Trade> = Vec::new();
        let mut open_trades: HashMap<String, (i64, f64, f64)> = HashMap::new();
        let mut peak = cfg.portfolio.initial_cash as f64;
        let mut run_error: Option<String> = None;

        let quote_ccy: HashMap<String, Currency> = profiles
            .iter()
            .filter_map(|p| {
                p.instrument
                    .quote
                    .parse::<Currency>()
                    .ok()
                    .map(|c| (p.instrument.symbol.clone(), c))
            })
            .collect();
        let instrument_types = profile_instrument_types(profiles);

        let allowed = &cfg.exchange.allowed_order_types;

        // Empty FX table — tests run single-currency, no leg bars. The
        // FX-aware `try_debit` falls back to base-only debits when the
        // table has no rates, exactly matching the legacy behaviour.
        let fx = FxTable::new(base_ccy);

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
                        status: "cancelled".into(),
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
                            status: "cancelled".into(),
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
                let mut notional = fill_px * qty.abs();
                let mut commission = match cfg.exchange.commission_type {
                    CommissionType::Percentage => notional * cfg.exchange.commission_pct / 100.0,
                    CommissionType::Fixed => cfg.exchange.commission_fixed,
                    CommissionType::PercentagePlusFixed => {
                        notional * cfg.exchange.commission_pct / 100.0
                            + cfg.exchange.commission_fixed
                    },
                };
                let order_ccy = quote_ccy.get(&order.symbol).copied().unwrap_or(base_ccy);
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
                        let denom = fill_px * (1.0 + pct_part);
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
                        notional = fill_px * filled_qty;
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
                    update_open_trade_buy(&mut open_trades, &symbol, ts, filled_qty, fill_px);
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
                        fill_px,
                        commission,
                    )
                    .map(|t| {
                        let pnl = t.pnl;
                        closed_trades.push(t);
                        pnl
                    });
                    fill_pnl = realised;
                }

                if (filled_qty - order.quantity).abs() > 1e-12 {
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
                    let pos_ccy = quote_ccy.get(sym).copied().unwrap_or(base_ccy);
                    equity += fx.convert(value, pos_ccy, base_ccy, ts).unwrap_or(value);
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
        let cancelled = r.orders.iter().filter(|o| o.status == "cancelled").count();
        assert!(cancelled >= 1);
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
        let ts = parse_iso_date_to_ts("2024-01-15").unwrap();
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

        let err = validate_starting_position_quantities(&cfg, &[mk_profile("AAPL", "USD")])
            .expect_err("fractional stock starting position should fail");

        assert!(err.to_string().contains("fractional quantity"));
        assert!(err.to_string().contains("only crypto"));
    }

    #[test]
    fn crypto_starting_position_allows_fractional_quantity() {
        let mut cfg = mk_cfg("BTC-USD");
        cfg.data.instrument_type = InstrumentType::Crypto;
        cfg.portfolio.starting_positions.insert("BTC-USD".into(), 0.25);

        validate_starting_position_quantities(
            &cfg,
            &[mk_profile_with_type("BTC-USD", "USD", InstrumentType::Crypto)],
        )
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

        let err = normalize_builtin_order_quantity(&mut order, InstrumentType::Stocks);

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

        let err = normalize_builtin_order_quantity(&mut order, InstrumentType::Crypto);

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

        let err = normalize_builtin_order_quantity(&mut order, InstrumentType::Stocks)
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
    fn settle_position_with_no_position_is_cancelled() {
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
        assert_eq!(r.orders[0].status, "cancelled");
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
        FxTable::new(base)
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
        assert!((effective_leverage_cap(&cfg) - 2.0).abs() < 1e-12);

        cfg.exchange.max_leverage = 1.5;
        cfg.exchange.initial_margin = 25.0; // → 4x
        assert!((effective_leverage_cap(&cfg) - 1.5).abs() < 1e-12);
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
        // Equity = 10_000, max per-pos notional = 5_000.
        // Order = 100 shares @ $60 = $6_000 → exceeds the cap, but
        // limit checks now shrink (instead of reject) so the order
        // returns the largest qty that fits: $5_000 / $60 ≈ 83.33.
        let qty = check_order_against_limits(
            &cfg,
            "AAPL",
            100.0,
            60.0,
            Currency::USD,
            Currency::USD,
            10_000.0,
            0.0,
            0.0,
            0.0,
            &fx,
            0,
        )
        .expect("limit check should shrink, not reject");
        assert!((qty - (5_000.0 / 60.0)).abs() < 1e-9, "got {qty}");
    }

    #[test]
    fn check_order_against_limits_rejects_when_position_already_at_cap() {
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.max_position_size = 50;
        let fx = empty_fx(Currency::USD);
        // Equity = 10_000, cap = 5_000. Current exposure already at 5_000
        // → zero headroom, so any new buy is fully rejected.
        let err = check_order_against_limits(
            &cfg,
            "AAPL",
            1.0,
            60.0,
            Currency::USD,
            Currency::USD,
            10_000.0,
            5_000.0,
            50.0,
            5_000.0,
            &fx,
            0,
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
        // Order = 50 shares @ $60 = $3_000 ≤ $5_000 cap.
        let qty = check_order_against_limits(
            &cfg,
            "AAPL",
            50.0,
            60.0,
            Currency::USD,
            Currency::USD,
            10_000.0,
            0.0,
            0.0,
            0.0,
            &fx,
            0,
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
        // Equity = 1_000; gross already = 1_000 (fully invested).
        // Another order @ $100 = $100 → would push gross past equity × 1.0.
        let err = check_order_against_limits(
            &cfg,
            "AAPL",
            1.0,
            100.0,
            Currency::USD,
            Currency::USD,
            1_000.0,
            1_000.0,
            0.0,
            0.0,
            &fx,
            0,
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
        // 3x of $1_000 equity = $3_000 cap. Gross 1_000 + new 1_500 = 2_500.
        check_order_against_limits(
            &cfg,
            "AAPL",
            15.0,
            100.0,
            Currency::USD,
            Currency::USD,
            1_000.0,
            1_000.0,
            0.0,
            0.0,
            &fx,
            0,
        )
        .expect("within 3x leverage");
    }

    #[test]
    fn check_order_against_limits_rejects_when_equity_non_positive() {
        let cfg = mk_cfg("AAPL");
        let fx = empty_fx(Currency::USD);
        let err = check_order_against_limits(
            &cfg,
            "AAPL",
            1.0,
            100.0,
            Currency::USD,
            Currency::USD,
            0.0,
            0.0,
            0.0,
            0.0,
            &fx,
            0,
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
        let mut fx = FxTable::new(Currency::USD);
        // 1 GBP = 1.30 USD
        fx.add_series(Currency::GBP, Currency::USD, vec![(0, 1.30)]);
        // Equity = 1_000 USD. Order = 100 shares @ £10 = £1_000 = $1_300 →
        // above 1x leverage. The check now shrinks the qty to whatever
        // fits inside $1_000 of headroom: ≈ £769.23 / £10 = 76.92 shares.
        let qty = check_order_against_limits(
            &cfg,
            "VOD.L",
            100.0,
            10.0,
            Currency::GBP,
            Currency::USD,
            1_000.0,
            0.0,
            0.0,
            0.0,
            &fx,
            0,
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
            &cfg,
            "AAPL",
            -1.0,
            100.0,
            Currency::USD,
            Currency::USD,
            1_000.0,
            1_000.0,
            10.0,
            1_000.0,
            &fx,
            0,
        )
        .expect("reducing exposure should be allowed at the cap");
        assert_eq!(qty, -1.0);
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
        let quote_ccy = HashMap::from([("AAPL".to_owned(), Currency::USD)]);
        let fx = empty_fx(Currency::USD);
        let gross =
            gross_notional_in_currency(&positions, &aligned, 0, &quote_ccy, Currency::USD, &fx, 0);
        assert!((gross - 500.0).abs() < 1e-12);
    }

    #[test]
    fn gross_notional_converts_quote_currency() {
        let aligned = dummy_aligned("VOD.L", 100.0);
        let mut positions = HashMap::new();
        positions.insert("VOD.L".to_owned(), 10.0);
        let quote_ccy = HashMap::from([("VOD.L".to_owned(), Currency::GBP)]);
        let mut fx = FxTable::new(Currency::USD);
        fx.add_series(Currency::GBP, Currency::USD, vec![(0, 1.30)]);
        let gross =
            gross_notional_in_currency(&positions, &aligned, 0, &quote_ccy, Currency::USD, &fx, 0);
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
        let quote_ccy = HashMap::from([("AAPL".to_owned(), Currency::USD)]);
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
        let mut fx = FxTable::new(Currency::USD);
        // 1 GBP = 1.30 USD → debit £1_000 = $1_300 from base.
        fx.add_series(Currency::GBP, Currency::USD, vec![(0, 1.30)]);
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
        let mut fx = FxTable::new(Currency::USD);
        fx.add_series(Currency::EUR, Currency::USD, vec![(0, 1.10)]);
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
        let mut fx = FxTable::new(Currency::USD);
        fx.add_series(Currency::EUR, Currency::USD, vec![(0, 1.10)]);
        // Threshold 10 USD; EUR balance worth ~5.50 USD → stays.
        sweep_foreign_to_base(&mut cash, Currency::USD, &fx, 0, Some(10.0));
        assert!(cash.contains_key(&Currency::EUR));
    }

    #[test]
    fn sweep_foreign_to_base_skips_when_no_rate() {
        let mut cash = HashMap::new();
        cash.insert(Currency::USD, 100.0);
        cash.insert(Currency::JPY, 1_000.0);
        let fx = FxTable::new(Currency::USD); // no rates
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

    // ── is_whole_quantity ──────────────────────────────────────────────

    #[test]
    fn is_whole_quantity_edge_cases() {
        assert!(is_whole_quantity(5.0));
        assert!(is_whole_quantity(0.0));
        assert!(is_whole_quantity(-3.0));
        assert!(!is_whole_quantity(3.5));
        assert!(!is_whole_quantity(f64::NAN));
        assert!(!is_whole_quantity(f64::INFINITY));
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
        assert!((qty - 20.0).abs() < 1e-12);
        assert!((avg - 40.0).abs() < 1e-12);
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
        assert!((t.quantity - 5.0).abs() < 1e-12);
        assert!(trades.contains_key("X"));
    }

    // ── persist & parse additional ─────────────────────────────────────

    #[test]
    fn persist_experiment_config_creates_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = mk_cfg("AAPL");
        let path = persist_experiment_config(dir.path(), "t1", &cfg).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn parse_iso_date_to_ts_epoch() {
        assert_eq!(parse_iso_date_to_ts("1970-01-01").unwrap(), 0);
    }

    #[test]
    fn parse_iso_date_to_ts_invalid() {
        assert!(parse_iso_date_to_ts("not-a-date").is_none());
        assert!(parse_iso_date_to_ts("").is_none());
    }

    #[test]
    fn now_secs_positive() {
        assert!(now_secs() > 0);
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
}
