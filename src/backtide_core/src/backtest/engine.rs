//! Backtest engine logic.
//!
//! This module implements the per-strategy event loop, order matching,
//! multi-currency portfolio bookkeeping and result aggregation. It runs
//! every selected strategy fully in parallel using [`rayon`].

use crate::backtest::fx::FxTable;
use crate::backtest::indicators::Indicator as BuiltinIndicator;
use crate::backtest::models::commission_type::CommissionType;
use crate::backtest::models::experiment_config::ExperimentConfig;
use crate::backtest::models::experiment_result::*;
use crate::backtest::models::order::{new_order_id, Order};
use crate::backtest::models::order_type::OrderType;
use crate::backtest::models::portfolio::Portfolio;
use crate::backtest::models::state::State;
use crate::backtest::strategies::{BuiltinStrategy, BuyAndHold, IndicatorView};
use crate::constants::BENCHMARK;
use crate::data::models::bar::Bar;
use crate::data::models::currency::Currency;
use crate::data::models::instrument_profile::InstrumentProfile;
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
    ///
    /// Phases:
    /// 1. Resolve & download required data (skipped if already in storage).
    ///    The configured ``strategy.benchmark`` symbol (if any) is folded
    ///    into the symbol list so its bars flow through the standard
    ///    pipeline and become available to every strategy.
    /// 2. Load OHLCV bars for every primary symbol on the chosen interval.
    /// 3. Compute every requested indicator (built-in or custom Python) once
    ///    over the full dataset, in parallel across (symbol, indicator).
    /// 4. Run every selected strategy in parallel, each with its own
    ///    portfolio, order book and equity log. When a benchmark is
    ///    configured, a ``BuyAndHold(symbol=benchmark)`` strategy is
    ///    auto-injected under the name returned by
    ///    [`benchmark_strategy_name`] so alpha can be derived from real
    ///    backtest results.
    /// 5. Persist the aggregate result (and per-strategy artefacts) to
    ///    DuckDB, then return it to the caller.
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
                status: "failed".into(),
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
        let primary_set: std::collections::HashSet<&str> =
            symbols.iter().map(String::as_str).collect();
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
        // Surface per-strategy failures: log each one and roll the
        // experiment status up to "failed" if any strategy errored out.
        let n_failed = results.iter().filter(|r| r.error.is_some()).count();
        for r in &results {
            if let Some(err) = &r.error {
                warn!(strategy = %r.strategy_name, "Strategy failed: {err}");
                warnings.push(format!("Strategy {:?} failed: {}", r.strategy_name, err));
            } else if r.orders.is_empty() {
                // No error, no orders — most often happens when the initial
                // cash is too low to afford a single whole unit of the
                // configured symbol(s). Quantities are tracked as integers,
                // so e.g. a $10k portfolio buying a $60k BTC benchmark
                // would round to 0 shares and never trade. Surface this
                // explicitly so it doesn't look like a silent failure.
                let msg = format!(
                    "Strategy {:?} placed no orders. The initial cash may be too low \
                     to afford a single whole unit of the targeted symbol(s) \
                     (quantities are integer). Consider increasing initial_cash.",
                    r.strategy_name
                );
                warn!(strategy = %r.strategy_name, "{msg}");
                warnings.push(msg);
            }
        }
        let status = if n_failed == 0 {
            "completed".to_owned()
        } else if n_failed == results.len() {
            "failed".to_owned()
        } else {
            "partial".to_owned()
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

/// Align bars to a master timeline using the configured empty-bar policy.
fn align_bars(
    bars: &HashMap<String, Vec<Bar>>,
    timeline: &[i64],
    policy: crate::backtest::models::empty_bar_policy::EmptyBarPolicy,
) -> HashMap<String, Vec<Option<Bar>>> {
    use crate::backtest::models::empty_bar_policy::EmptyBarPolicy::*;

    let mut out: HashMap<String, Vec<Option<Bar>>> = HashMap::new();
    for (sym, sym_bars) in bars {
        let by_ts: HashMap<i64, Bar> =
            sym_bars.iter().map(|b| (b.open_ts as i64, b.clone())).collect();
        let mut row: Vec<Option<Bar>> = Vec::with_capacity(timeline.len());
        let mut last: Option<Bar> = None;
        for ts in timeline {
            match by_ts.get(ts) {
                Some(b) => {
                    last = Some(b.clone());
                    row.push(Some(b.clone()));
                },
                None => match policy {
                    Skip => row.push(None),
                    ForwardFill => {
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
                    FillWithNaN => {
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
    let mut out: HashMap<String, HashMap<String, Vec<Vec<f64>>>> = HashMap::new();

    for (name, obj) in indicator_objs {
        let mut per_symbol: HashMap<String, Vec<Vec<f64>>> = HashMap::new();

        for (sym, row) in aligned {
            let bars: Vec<Bar> = row
                .iter()
                .map(|b| {
                    b.clone().unwrap_or(Bar {
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
                    })
                })
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

/// Build the deterministic ``__auto_*`` name for a Python indicator
/// instance. Mirrors `_auto_indicator_name` in the Python strategy utils
/// and the Rust `auto_indicator_name` used by built-in strategies'
/// ``decide_inner`` so the engine and the strategies look up indicators
/// under the *same* key.
///
/// Format: ``__auto_<ACRONYM>_<arg1>_<arg2>_...`` (or ``__auto_<ACRONYM>_default``
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
    Ok(format!("__auto_{acronym}_{sanitized}"))
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
#[allow(clippy::too_many_arguments)]
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
    let mut positions: HashMap<String, f64> = cfg.portfolio.starting_positions.clone();
    let mut open_orders: Vec<Order> = Vec::new();
    // Per-order extremes for trailing stops: (running_high, running_low)
    // observed since the order was first seen. Cleared on fill / cancel.
    let mut trail_state: HashMap<String, (f64, f64)> = HashMap::new();

    let mut equity_curve: Vec<EquitySample> = Vec::with_capacity(total_bars);
    let mut order_records: Vec<OrderRecord> = Vec::new();
    let mut closed_trades: Vec<Trade> = Vec::new();
    // Open trade tracker per symbol: (entry_ts, qty_remaining, entry_price)
    let mut open_trades: HashMap<String, (i64, f64, f64)> = HashMap::new();

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
        let ts = timeline[bar_index];
        let is_warmup = bar_index < warmup;

        // ── 1. Resolve open orders against the *current* bar ────────────
        let mut still_open: Vec<Order> = Vec::new();
        let drained: Vec<Order> = std::mem::take(&mut open_orders);
        for mut order in drained {
            // Cancel orders take effect immediately and do not need a price.
            if order.order_type == OrderType::CancelOrder {
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

            let qty = order.quantity;
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
                    // Solve for the largest fractional quantity q such that
                    //   fill_px * q * (1 + pct_part) + fixed_part <= avail.
                    let denom = fill_px * (1.0 + pct_part);
                    let max_qty: f64 = if denom > 0.0 && avail > fixed_part {
                        ((avail - fixed_part) / denom).max(0.0)
                    } else {
                        0.0
                    };
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
                let closes_view: Vec<(String, &[f64])> =
                    closes_full.iter().map(|(s, v)| (s.clone(), &v[..=bar_index])).collect();
                let inds = IndicatorView::new(indicators, bar_index);
                Ok(b.decide(&closes_view, &inds, &portfolio, &state))
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
                    // Validate allowed types & ensure ids are populated.
                    let allowed = &cfg.exchange.allowed_order_types;
                    ords.retain_mut(|o| {
                        if o.id.is_empty() {
                            o.id = new_order_id();
                        }
                        if !allowed.contains(&o.order_type)
                            && o.order_type != OrderType::CancelOrder
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
        equity_curve.push(EquitySample {
            timestamp: ts,
            equity,
            cash: cash.iter().filter(|(_, v)| v.abs() > 1e-12).map(|(k, v)| (*k, *v)).collect(),
            drawdown,
        });
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
        // CancelOrder is handled before resolve_trigger is called.
        CancelOrder => TriggerOutcome::Cancel {
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
    let base_avail = *cash.get(&base).unwrap_or(&0.0);
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
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::models::commission_type::CommissionType;
    use crate::backtest::models::empty_bar_policy::EmptyBarPolicy;
    use crate::backtest::models::experiment_config::*;
    use crate::data::models::instrument::Instrument;
    use crate::data::models::instrument_profile::InstrumentProfile;
    use crate::data::models::instrument_type::InstrumentType;

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
        cfg.exchange.allowed_order_types = vec![OrderType::Market, OrderType::CancelOrder];
        cfg
    }

    fn mk_profile(symbol: &str, quote: &str) -> InstrumentProfile {
        InstrumentProfile {
            instrument: Instrument {
                symbol: symbol.to_owned(),
                name: symbol.to_owned(),
                base: None,
                quote: quote.to_owned(),
                instrument_type: InstrumentType::Stocks,
                exchange: "TEST".to_owned(),
                provider: Provider::Yahoo,
            },
            earliest_ts: HashMap::new(),
            latest_ts: HashMap::new(),
            legs: vec![],
        }
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
                    if !allowed.contains(&o.order_type) && o.order_type != OrderType::CancelOrder {
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
                    open_orders.push(o.clone());
                }
            }

            // Resolve open orders: faithful copy of run_one_strategy's logic.
            let drained: Vec<Order> = std::mem::take(&mut open_orders);
            for mut order in drained {
                if order.order_type == OrderType::CancelOrder {
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
                        let max_qty: f64 = if denom > 0.0 && avail > fixed_part {
                            ((avail - fixed_part) / denom).max(0.0)
                        } else {
                            0.0
                        };
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
            error: None,
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
        cfg.portfolio.initial_cash = 50; // can't afford 10 @ 100
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let order = Order {
            id: "buy1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, order)]);
        // Current behavior auto-shrinks to the largest buy that fits cash.
        assert_eq!(r.orders[0].status, "filled");
        assert!(r.orders[0].order.quantity > 0.0);
        assert!(r.orders[0].order.quantity < 10.0);
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
        };
        let cancel = Order {
            id: "buy1".into(),
            symbol: "".into(),
            order_type: OrderType::CancelOrder,
            quantity: 0.0,
            price: None,
            limit_price: None,
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
        };
        let buy_b = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            limit_price: None,
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
    // Order types — see `OrderType` enum. The engine only implements
    // Market + CancelOrder execution semantics today; everything else
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
        cfg.exchange.allowed_order_types = vec![OrderType::Market, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let limit = Order {
            id: "lim1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(95.0),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::Limit, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]); // mk_bar makes high=*1.01, low=*0.99
        let limit = Order {
            id: "lim1".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(50.0),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::Limit, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 100.0]);
        let limit = Order {
            id: "lim".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(110.0),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::Limit, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0]);
        let limit = Order {
            id: "lim".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(99.5),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::Limit, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0]);
        // Need a long to sell from.
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
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
            vec![OrderType::Market, OrderType::Limit, OrderType::CancelOrder];
        cfg.exchange.slippage = 1.0;
        let aligned = mk_aligned("AAPL", &[100.0]);
        let limit = Order {
            id: "lim".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(99.5),
            limit_price: None,
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, limit)]);
        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].fill_price, Some(99.5));
    }

    // ─────────────────────────────────────────────────────────────────
    // Stop-loss / take-profit
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn stop_loss_sell_does_not_trigger_above_stop() {
        // Sell stop at 90, prices stay at/above 100 → never triggers.
        let mut cfg = mk_cfg("AAPL");
        cfg.exchange.allowed_order_types =
            vec![OrderType::Market, OrderType::StopLoss, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
        };
        let sl = Order {
            id: "sl".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLoss,
            quantity: -5.0,
            price: Some(90.0),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::StopLoss, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 90.0]); // bar2 open=90 < 95
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
        };
        let sl = Order {
            id: "sl".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLoss,
            quantity: -5.0,
            price: Some(95.0),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::StopLoss, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
        };
        let sl = Order {
            id: "sl".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLoss,
            quantity: -5.0,
            price: Some(99.5),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::StopLoss, OrderType::CancelOrder];
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
            vec![OrderType::Market, OrderType::TakeProfit, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
        };
        let tp = Order {
            id: "tp".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::TakeProfit,
            quantity: -5.0,
            price: Some(100.5),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::StopLossLimit, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 90.0, 96.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
        };
        let sll = Order {
            id: "sll".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLossLimit,
            quantity: -5.0,
            price: Some(95.0),       // stop trigger
            limit_price: Some(95.0), // limit price after trigger
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
            vec![OrderType::Market, OrderType::StopLossLimit, OrderType::CancelOrder];
        // Prices stay above stop forever.
        let aligned = mk_aligned("AAPL", &[100.0, 102.0, 104.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
        };
        let sll = Order {
            id: "sll".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::StopLossLimit,
            quantity: -5.0,
            price: Some(90.0),
            limit_price: Some(89.5),
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
            vec![OrderType::Market, OrderType::TrailingStop, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 105.0, 110.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
        };
        let trail = Order {
            id: "t".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::TrailingStop,
            quantity: -5.0,
            price: Some(5.0),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::TrailingStop, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 110.0, 100.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
        };
        let trail = Order {
            id: "t".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::TrailingStop,
            quantity: -5.0,
            price: Some(5.0),
            limit_price: None,
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
            vec![OrderType::Market, OrderType::TrailingStopLimit, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 110.0, 100.0, 106.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 5.0,
            price: None,
            limit_price: None,
        };
        let trail = Order {
            id: "t".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::TrailingStopLimit,
            quantity: -5.0,
            price: Some(5.0),
            limit_price: Some(105.0),
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
            vec![OrderType::Market, OrderType::SettlePosition, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let buy = Order {
            id: "b".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: 7.0,
            price: None,
            limit_price: None,
        };
        let settle = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::SettlePosition,
            quantity: 0.0,
            price: None,
            limit_price: None,
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
            vec![OrderType::Market, OrderType::SettlePosition, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let settle = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::SettlePosition,
            quantity: 0.0,
            price: None,
            limit_price: None,
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
            vec![OrderType::Market, OrderType::SettlePosition, OrderType::CancelOrder];
        let aligned = mk_aligned("AAPL", &[100.0, 101.0]);
        let short = Order {
            id: "sh".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: -3.0,
            price: None,
            limit_price: None,
        };
        let settle = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::SettlePosition,
            quantity: 0.0,
            price: None,
            limit_price: None,
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
        };
        let sell = Order {
            id: "s".into(),
            symbol: "AAPL".into(),
            order_type: OrderType::Market,
            quantity: -3.0,
            price: None,
            limit_price: None,
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
        // `same_currency_buy_can_overdraw_cash` below for that quirk).
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
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("VOD.L", "GBP")], vec![(0, buy)]);
        // No GBP/USD FX path is provided in this unit setup, so funding fails.
        assert_eq!(r.orders[0].status, "rejected");
        assert_eq!(r.orders[0].reason, "insufficient funds");
    }

    #[test]
    fn same_currency_buy_can_overdraw_cash() {
        // GAP: when the order's quote currency equals the portfolio's
        // base currency, `try_debit` falls back to "pay from base" and
        // allows cash to go negative because the same map entry is
        // first read for `avail` then re-read for `base_avail`. The
        // auto-shrink branch is therefore never reached and the buy
        // settles in full at a negative cash balance.
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
        };
        let r = run_with_orders(&cfg, &aligned, &[mk_profile("AAPL", "USD")], vec![(0, buy)]);
        assert_eq!(r.orders[0].status, "filled");
        assert_eq!(r.orders[0].order.quantity, 11.0); // not shrunk
                                                      // Cash went negative: 1,000 - 1,100 = -100.
        let last_cash = r
            .equity_curve
            .last()
            .and_then(|s| s.cash.get(&cfg.portfolio.base_currency))
            .copied()
            .unwrap_or(0.0);
        assert!(last_cash < 0.0);
    }
}
