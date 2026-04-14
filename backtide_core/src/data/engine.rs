//! Implementation of data related methods for [`Engine`].

use crate::config::models::triangulation_strategy::TriangulationStrategy;
use crate::constants::{Symbol, CIRCUIT_BREAKER_THRESHOLD, MAX_CONCURRENT_REQUESTS, TASK_TIMEOUT};
use crate::data::errors::{DataError, DataResult};
use crate::data::models::currency::Currency;
use crate::data::models::download_result::DownloadResult;
use crate::data::models::exchange::Exchange;
use crate::data::models::forex_pair::ForexPair;
use crate::data::models::instrument::Instrument;
use crate::data::models::instrument_profile::InstrumentProfile;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::interval::Interval;
use crate::data::models::provider::Provider;
use crate::engine::Engine;
use crate::errors::EngineResult;
use crate::storage::models::bar_series::BarSeries;
use crate::storage::models::dividend_series::DividendSeries;
use crate::utils::progress::{progress_bar, progress_spinner};
use futures::future::{join_all, try_join_all};
use futures::stream::{self, StreamExt};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

impl Engine {
    // ────────────────────────────────────────────────────────────────────────
    // Public interface
    // ────────────────────────────────────────────────────────────────────────

    /// Download instruments from a list of [`InstrumentProfile`]s and store the
    /// results in the database.
    ///
    /// * Checks what is already in storage — skips completed ranges and only downloads
    ///   the missing head/tail (or both) for partial ones.
    /// * Downloads concurrently across symbols and intervals.
    /// * Writes all downloaded data in a single bulk transaction.
    /// * Only writes contiguous (gap-free) bars starting from the beginning of the range.
    /// * Idempotent, i.e., re-calling with the same profiles is a no-op.
    ///
    /// When `start` or `end` is provided, the per-instrument range is clamped so that
    /// no data before `start` or after `end` is requested from the provider.
    #[instrument(skip(self, profiles), fields(n_profiles = profiles.len(), start, end))]
    pub fn download_instruments(
        &self,
        profiles: &[InstrumentProfile],
        start: Option<u64>,
        end: Option<u64>,
        verbose: bool,
    ) -> EngineResult<DownloadResult> {
        self.rt.block_on(async {
            // Build a list of (symbol, instrument_type, interval, start, end) tasks
            let mut tasks: Vec<(String, InstrumentType, Interval, u64, u64)> = Vec::new();

            let stored_ranges = self.get_bar_ranges()?;

            for profile in profiles {
                let symbol = &profile.instrument.symbol;
                let instrument_type = profile.instrument.instrument_type;
                let provider = self.provider(instrument_type);

                for (interval, meta_start) in &profile.earliest_ts {
                    let meta_end = profile.latest_ts.get(interval).unwrap();

                    // Clamp to the user-requested range when provided.
                    let start = start.map_or(*meta_start, |s| s.max(*meta_start));
                    let end = end.map_or(*meta_end, |e| e.min(*meta_end));

                    if start >= end {
                        debug!(%symbol, ?interval, "User range does not overlap provider range, skipping.");
                        continue;
                    }

                    // Look up the stored range from the pre-fetched map.
                    let key = (symbol.clone(), interval.to_string(), provider.to_string());
                    if let Some(&(db_min, db_max)) = stored_ranges.get(&key) {
                        let interval_secs = interval.minutes() * 60;

                        if db_min <= start + interval_secs && db_max + interval_secs >= end {
                            debug!(%symbol, ?interval, "Already in database, skipping download.");
                            continue;
                        }

                        if start + interval_secs < db_min {
                            debug!(%symbol, ?interval, head_end = db_min, "Downloading missing head.");
                            tasks.push((symbol.clone(), instrument_type, *interval, start, db_min));
                        }

                        if end > db_max + interval_secs {
                            debug!(%symbol, ?interval, tail_start = db_max, "Downloading missing tail.");
                            tasks.push((symbol.clone(), instrument_type, *interval, db_max, end));
                        }
                    } else {
                        tasks.push((symbol.clone(), instrument_type, *interval, start, end));
                    };
                }
            }

            let total_tasks = tasks.len();
            info!("Download plan: {total_tasks} symbol x interval tasks");

            // ── Phase 1: download all tasks concurrently ─────────────────

            let pb = (verbose && total_tasks > 0).then(|| {
                progress_bar(
                    total_tasks as u64,
                    format!("Downloading bars for {} profiles...", profiles.len()),
                )
            });

            // Circuit breaker: after CIRCUIT_BREAKER_THRESHOLD consecutive
            // failures, skip remaining tasks — the provider is likely
            // unreachable or blocking us.
            let consecutive_failures = Arc::new(AtomicUsize::new(0));

            let downloaded: Vec<_> = stream::iter(tasks.into_iter().enumerate().map(
                |(idx, (symbol, it, interval, start, end))| {
                    let pb = pb.clone();
                    let consecutive_failures = Arc::clone(&consecutive_failures);
                    async move {
                        // Check circuit breaker before attempting the request.
                        let failures = consecutive_failures.load(Ordering::Relaxed);
                        if failures >= CIRCUIT_BREAKER_THRESHOLD {
                            if let Some(ref pb) = pb {
                                pb.inc(1);
                            }
                            return (
                                idx,
                                symbol,
                                it,
                                interval,
                                Err(DataError::CircuitBreaker(failures)),
                            );
                        }

                        let provider = self.providers.get(&it).unwrap();
                        info!(%symbol, ?interval, start, end, "Downloading...");

                        let result = tokio::time::timeout(
                            TASK_TIMEOUT,
                            provider.download_bars(&symbol, it, interval, start, end),
                        )
                        .await;

                        // Flatten the timeout result into a DataError.
                        let result = match result {
                            Ok(inner) => inner,
                            Err(_) => {
                                warn!(%symbol, ?interval, "Download timed out after {TASK_TIMEOUT:?}.");
                                Err(DataError::Timeout { symbol: symbol.clone(), interval })
                            },
                        };

                        // Update circuit breaker state.
                        match &result {
                            Ok(_) => {
                                consecutive_failures.store(0, Ordering::Relaxed);
                            },
                            Err(_) => {
                                let prev =
                                    consecutive_failures.fetch_add(1, Ordering::Relaxed);
                                if prev + 1 == CIRCUIT_BREAKER_THRESHOLD {
                                    warn!(
                                        "Circuit breaker tripped after {} consecutive failures, \
                                         skipping remaining tasks.",
                                        prev + 1
                                    );
                                }
                            },
                        }

                        if let Some(ref pb) = pb {
                            pb.inc(1);
                        }

                        (idx, symbol, it, interval, result)
                    }
                },
            ))
            .buffer_unordered(MAX_CONCURRENT_REQUESTS)
            .collect()
            .await;

            if let Some(ref pb) = pb {
                pb.finish_and_clear();
            }

            // ── Phase 2: collect results and build one bulk write ────────

            let mut bar_series: Vec<BarSeries> = Vec::new();
            let mut div_series: Vec<DividendSeries> = Vec::new();
            let mut outcomes: Vec<(usize, String, Interval, Result<usize, String>)> = Vec::new();

            for (idx, symbol, it, interval, result) in downloaded {
                let provider_enum = self.provider(it);
                match result {
                    Ok(download) => {
                        let n_bars = download.bars.len();
                        let n_divs = download.dividends.len();
                        info!(%symbol, ?interval, bars = n_bars, dividends = n_divs, "Downloaded.");

                        bar_series.push(BarSeries {
                            symbol: symbol.clone(),
                            instrument_type: it,
                            interval,
                            provider: provider_enum,
                            bars: download.bars,
                        });

                        if !download.dividends.is_empty() {
                            div_series.push(DividendSeries {
                                symbol: symbol.clone(),
                                provider: provider_enum,
                                dividends: download.dividends,
                            });
                        }
                        outcomes.push((idx, symbol, interval, Ok(n_bars)));
                    },
                    Err(e) => {
                        warn!(%symbol, ?interval, "Download failed: {e}");
                        outcomes.push((idx, symbol, interval, Err(e.to_string())));
                    },
                }
            }

            if !bar_series.is_empty() {
                info!(n_series = bar_series.len(), "Writing bar data to the database...");
                self.write_bars_bulk(&bar_series)?;
                info!("Bar bulk write complete.");
            }

            if !div_series.is_empty() {
                info!(n_series = div_series.len(), "Writing dividend data to the database...");
                self.write_dividends_bulk(&div_series)?;
                info!("Dividend bulk write complete.");
            }

            // ── Phase 3: build result summary ────────────────────────────

            let mut n_succeeded = 0usize;
            let mut warnings = Vec::new();
            for (_idx, symbol, interval, outcome) in outcomes {
                match outcome {
                    Ok(_) => n_succeeded += 1,
                    Err(msg) => warnings.push(format!("{symbol} ({interval}): {msg}")),
                }
            }

            Ok(DownloadResult {
                n_succeeded,
                n_failed: warnings.len(),
                warnings,
            })
        })
    }

    /// Fetch instruments concurrently, using the cache where possible.
    #[instrument(skip(self, symbols), fields(?instrument_type, n_symbols = symbols.len()))]
    pub fn get_instruments(
        &self,
        symbols: Vec<Symbol>,
        instrument_type: InstrumentType,
    ) -> DataResult<Vec<Instrument>> {
        self.rt.block_on(async {
            let tasks: Vec<_> =
                symbols.iter().map(|s| self.load_instrument(s, instrument_type)).collect();
            join_all(tasks).await.into_iter().collect::<Result<Vec<_>, _>>()
        })
    }

    /// List the most important instruments for a given instrument type, capped at `limit`.
    ///
    /// When `exchanges` is `None`, delegates directly to the provider.
    /// When `exchanges` is `Some`, distributes `limit` evenly across the
    /// specified exchanges (returning the top instruments by volume×price
    /// per exchange).
    #[instrument(skip(self, exchanges), fields(?instrument_type, n_exchanges = exchanges.as_ref().map_or(0, |e| e.len())))]
    pub fn list_instruments(
        &self,
        instrument_type: InstrumentType,
        exchanges: Option<Vec<Exchange>>,
        limit: usize,
        verbose: bool,
    ) -> DataResult<Vec<Instrument>> {
        let pb =
            verbose.then(|| progress_spinner(format!("Listing {instrument_type} instruments...")));

        let instruments =
            self.rt.block_on(self.providers.get(&instrument_type).unwrap().list_instruments(
                instrument_type,
                exchanges,
                limit,
            ))?;

        if let Some(ref pb) = pb {
            pb.finish_and_clear();
        }

        // Warm the instrument cache so subsequent resolve_profiles calls don't
        // need to re-fetch each symbol individually.
        self.rt.block_on(async {
            for instr in &instruments {
                self.cache
                    .instrument_cache
                    .insert(instr.symbol.clone(), Arc::new(instr.clone()))
                    .await;
            }
        });

        Ok(instruments)
    }

    /// Resolves all instruments required to price the given symbols in the
    /// portfolio base currency, including any triangulation intermediaries.
    ///
    /// Returns a flat, deduplicated list of [`InstrumentProfile`]s — direct
    /// instruments first, followed by currency-conversion legs.
    #[instrument(skip(self, symbols), fields(?instrument_type, ?intervals, n_symbols = symbols.len()))]
    pub fn resolve_profiles(
        &self,
        symbols: Vec<Symbol>,
        instrument_type: InstrumentType,
        intervals: Vec<Interval>,
        verbose: bool,
    ) -> DataResult<Vec<InstrumentProfile>> {
        let base_currency = &self.config.general.base_currency.to_string();

        let tri_strategy = self.config.general.triangulation_strategy;
        let tri_fiat = &self.config.general.triangulation_fiat.to_string();
        let tri_crypto = &self.config.general.triangulation_crypto;
        let tri_crypto_pegged = &self.config.general.triangulation_crypto_pegged.to_string();

        self.rt.block_on(async {
            // Resolve the primary instruments.
            let tasks: Vec<_> =
                symbols.iter().map(|s| self.load_instrument(s, instrument_type)).collect();
            let instruments = join_all(tasks).await.into_iter().collect::<Result<Vec<_>, _>>()?;

            let mut leg_map: IndexMap<String, Instrument> = IndexMap::new();
            let mut instrument_leg_symbols: Vec<Vec<Symbol>> = Vec::new();

            // Collect the unique quote currencies that need conversion legs.
            let unique_quotes: Vec<&str> = {
                let mut seen = HashSet::new();
                instruments
                    .iter()
                    .filter(|i| {
                        !(i.base.as_ref().is_some_and(|b| b == base_currency)
                            || &i.quote == base_currency)
                    })
                    .filter_map(|i| seen.insert(i.quote.as_str()).then_some(i.quote.as_str()))
                    .collect()
            };

            // Resolve legs for every unique quote currency concurrently.
            let resolved_legs: HashMap<&str, Vec<Instrument>> =
                join_all(unique_quotes.into_iter().map(|quote| {
                    let intervals = &intervals;
                    async move {
                        let is_fiat = quote.parse::<Currency>().is_ok();
                        self.resolve_legs(
                            quote,
                            base_currency,
                            if is_fiat {
                                (tri_fiat, tri_fiat)
                            } else {
                                (tri_crypto, tri_crypto_pegged)
                            },
                            if is_fiat {
                                InstrumentType::Forex
                            } else {
                                InstrumentType::Crypto
                            },
                            intervals,
                            tri_strategy,
                        )
                        .await
                        .map(|legs| (quote, legs))
                    }
                }))
                .await
                .into_iter()
                .filter_map(|result| match result {
                    Ok(pair) => Some(pair),
                    Err(e) => {
                        warn!("Skipping unconvertible quote currency. {e}");
                        None
                    },
                })
                .collect();

            // Build per-instrument leg symbols from the pre-resolved results.
            // Instruments whose quote could not be resolved get empty legs.
            for instr in &instruments {
                if instr.base.as_ref().is_some_and(|b| b == base_currency)
                    || &instr.quote == base_currency
                {
                    instrument_leg_symbols.push(vec![]);
                } else if let Some(legs) = resolved_legs.get(instr.quote.as_str()) {
                    instrument_leg_symbols.push(legs.iter().map(|l| l.symbol.clone()).collect());
                } else {
                    instrument_leg_symbols.push(vec![]);
                }
            }

            // Consume resolved legs into the flat leg map.
            for (_, legs) in resolved_legs {
                for leg in legs {
                    leg_map.entry(leg.symbol.clone()).or_insert(leg);
                }
            }

            let total = instruments.len();
            let pb = verbose.then(|| {
                progress_bar(total as u64, format!("Resolving profiles for {total} symbols..."))
            });

            let instrument_profiles: Vec<InstrumentProfile> =
                stream::iter(instruments.into_iter().zip(instrument_leg_symbols.into_iter()).map(
                    |(instr, legs)| {
                        let pb = pb.clone();
                        let intervals = intervals.clone();
                        async move {
                            let (earliest_ts, latest_ts) =
                                self.load_range(&instr, &intervals).await?;

                            if let Some(ref pb) = pb {
                                pb.inc(1);
                            }

                            Ok::<_, DataError>(InstrumentProfile {
                                instrument: instr,
                                earliest_ts,
                                latest_ts,
                                legs,
                            })
                        }
                    },
                ))
                .buffer_unordered(MAX_CONCURRENT_REQUESTS)
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;

            if let Some(ref pb) = pb {
                pb.finish_and_clear();
            }

            let leg_profiles = try_join_all(leg_map.into_values().map(|instr| {
                let intervals = intervals.clone();
                async move {
                    let (earliest_ts, latest_ts) = self.load_range(&instr, &intervals).await?;
                    Ok::<_, DataError>(InstrumentProfile {
                        instrument: instr,
                        earliest_ts,
                        latest_ts,
                        legs: vec![],
                    })
                }
            }))
            .await?;

            // Merge into a single flat vec, deduplicating by symbol.
            let mut seen = HashSet::new();
            let profiles: Vec<_> = instrument_profiles
                .into_iter()
                .chain(leg_profiles)
                .filter(|p| seen.insert(p.instrument.symbol.clone()))
                .collect();

            Ok(profiles)
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private interface
    // ────────────────────────────────────────────────────────────────────────

    /// Resolve the [`Provider`] for a given instrument type from config.
    fn provider(&self, instrument_type: InstrumentType) -> Provider {
        *self.config.data.providers.get(&instrument_type).unwrap()
    }

    /// Resolve an instrument using the engine's cache.
    async fn load_instrument(
        &self,
        symbol: &Symbol,
        instrument_type: InstrumentType,
    ) -> DataResult<Instrument> {
        if let Some(instr) = self.cache.instrument_cache.get(symbol).await {
            debug!(%symbol, "Instrument cache hit.");
            return Ok(instr.as_ref().clone());
        }

        let provider = self.providers.get(&instrument_type).unwrap();
        let instr = provider.get_instrument(symbol, instrument_type).await?;
        self.cache.instrument_cache.insert(symbol.clone(), Arc::new(instr.clone())).await;
        debug!(%symbol, "Instrument cached");
        Ok(instr)
    }

    /// Resolve an instrument range for one or multiple intervals using the engine's cache.
    async fn load_range(
        &self,
        instrument: &Instrument,
        intervals: &[Interval],
    ) -> DataResult<(HashMap<Interval, u64>, HashMap<Interval, u64>)> {
        let provider = self.providers.get(&instrument.instrument_type).unwrap();

        let ranges = try_join_all(intervals.iter().map(|&iv| async move {
            let key = (instrument.symbol.clone(), iv);

            if let Some(range) = self.cache.range_cache.get(&key).await {
                debug!(symbol = %instrument.symbol, ?iv, "Range cache hit.");
                return Ok::<_, DataError>((iv, range.0, range.1));
            }

            let (start, end) = provider.get_download_range(instrument.clone(), iv).await?;
            self.cache.range_cache.insert(key, (start, end)).await;
            Ok::<_, DataError>((iv, start, end))
        }))
        .await?;

        let mut earliest = HashMap::new();
        let mut latest = HashMap::new();
        for (iv, start, end) in ranges {
            earliest.insert(iv, start);
            latest.insert(iv, end);
            debug!(symbol = %instrument.symbol, ?iv, "Range cached.");
        }

        Ok((earliest, latest))
    }

    /// Try to load an instrument from symbol format base-quote or quote-base.
    ///
    /// When both orderings exist, prefer the one whose concatenated symbol
    /// matches a known [`ForexPair`] variant.
    /// If neither (or both) match, fall back to the one with the longest history.
    async fn load_instrument_bidirectional(
        &self,
        base: &str,
        quote: &str,
        it: InstrumentType,
        intervals: &[Interval],
    ) -> DataResult<Instrument> {
        let base_quote = format!("{base}-{quote}");
        let quote_base = format!("{quote}-{base}");

        let (direct, inverse) = tokio::join!(
            self.load_instrument(&base_quote, it),
            self.load_instrument(&quote_base, it),
        );

        match (direct, inverse) {
            (Ok(d), Ok(i)) => {
                // Prefer the ordering that matches a canonical pair variant.
                let bq_is_forex = format!("{base}{quote}").parse::<ForexPair>().is_ok();
                let qb_is_forex = format!("{quote}{base}").parse::<ForexPair>().is_ok();

                if bq_is_forex && !qb_is_forex {
                    return Ok(d);
                }
                if qb_is_forex && !bq_is_forex {
                    return Ok(i);
                }

                // Neither or both match — fall back to longest history.
                let d_start =
                    self.load_range(&d, intervals).await?.0.into_values().min().unwrap_or(u64::MAX);
                let i_start =
                    self.load_range(&i, intervals).await?.0.into_values().min().unwrap_or(u64::MAX);

                Ok(if d_start <= i_start {
                    d
                } else {
                    i
                })
            },
            (Ok(d), Err(_)) => Ok(d),
            (Err(_), Ok(i)) => Ok(i),
            (Err(e), Err(_)) => Err(e),
        }
    }

    /// Resolve a two-leg triangulation path: `quote → mid` and `mid_pegged → base`.
    ///
    /// Legs that are identical to their target currency are omitted.
    async fn triangulate(
        &self,
        quote: &str,
        mid: (&str, &str),
        base: &str,
        it: InstrumentType,
        intervals: &[Interval],
    ) -> DataResult<Vec<Instrument>> {
        let mut legs = Vec::new();

        if quote != mid.0 {
            legs.push(self.load_instrument_bidirectional(quote, mid.0, it, intervals).await?);
        }

        if mid.1 != base {
            // When both the pegged mid-currency and the target base are fiat
            // currencies, resolve the leg via the Forex provider
            let leg_it = if mid.1.parse::<Currency>().is_ok() && base.parse::<Currency>().is_ok() {
                InstrumentType::Forex
            } else {
                it
            };

            legs.push(self.load_instrument_bidirectional(mid.1, base, leg_it, intervals).await?);
        }

        if legs.is_empty() {
            return Err(DataError::NoConversionPath {
                from: quote.to_string(),
                to: base.to_string(),
            });
        }

        Ok(legs)
    }

    /// Resolve the conversion legs needed to bring `quote` to `base`.
    async fn resolve_legs(
        &self,
        quote: &str,
        base: &str,
        mid: (&str, &str),
        it: InstrumentType,
        intervals: &[Interval],
        strategy: TriangulationStrategy,
    ) -> DataResult<Vec<Instrument>> {
        let direct = self.load_instrument_bidirectional(quote, base, it, intervals).await;

        match strategy {
            TriangulationStrategy::Direct => match direct {
                Ok(leg) => Ok(vec![leg]),
                Err(_) => self.triangulate(quote, mid, base, it, intervals).await,
            },
            TriangulationStrategy::Earliest => {
                let tri = self.triangulate(quote, mid, base, it, intervals).await;
                match (direct, tri) {
                    (Ok(d), Ok(t)) => {
                        let d_start = self
                            .load_range(&d, intervals)
                            .await?
                            .0
                            .into_values()
                            .min()
                            .unwrap_or(u64::MAX);

                        let t_start = try_join_all(t.iter().map(|l| self.load_range(l, intervals)))
                            .await?
                            .into_iter()
                            .flat_map(|(e, _)| e.into_values())
                            .max()
                            .unwrap_or(u64::MAX);

                        Ok(if d_start <= t_start {
                            vec![d]
                        } else {
                            t
                        })
                    },
                    (Ok(d), Err(_)) => Ok(vec![d]),
                    (Err(_), Ok(t)) => Ok(t),
                    (Err(e), Err(_)) => Err(e),
                }
            },
        }
    }
}
