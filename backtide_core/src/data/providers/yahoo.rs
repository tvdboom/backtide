//! Yahoo Finance data provider.
//!
//! Authenticates via a session cookie + CSRF crumb, then exposes asset
//! discovery (screener) and per-symbol metadata (chart endpoint).

use crate::constants::Symbol;
use crate::data::errors::{DataError, DataResult};
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::bar::Bar;
use crate::data::models::currency::Currency;
use crate::data::models::exchange::Exchange;
use crate::data::models::forex_pair::ForexPair;
use crate::data::models::interval::Interval;
use crate::data::providers::traits::DataProvider;
use crate::data::utils::canonical_symbol;
use crate::utils::http::{paginate, HttpClient};
use async_trait::async_trait;
use futures::future::join_all;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use strum::IntoEnumIterator;
use tokio::try_join;
use tracing::{debug, info, instrument};

/// Yahoo Finance data provider.
///
/// Wraps Yahoo's screener and chart APIs behind the [`DataProvider`] trait.
/// A valid session cookie and CSRF crumb are obtained during construction
/// and reused for all subsequent requests.
pub struct YahooFinance {
    /// Shared async HTTP client with a persistent cookie jar.
    client: HttpClient,

    /// CSRF crumb token tied to the active session cookie.
    crumb: String,
}

impl YahooFinance {
    /// Seeds the session cookie; response body is discarded.
    const COOKIE_SEED_URL: &str = "https://fc.yahoo.com";

    /// Returns a one-time CSRF crumb bound to the active session.
    const CRUMB_URL: &str = "https://query2.finance.yahoo.com/v1/test/getcrumb";

    /// Custom POST screener — accepts arbitrary query predicates.
    const SCREENER_URL: &str = "https://query2.finance.yahoo.com/v1/finance/screener";

    /// Yahoo-managed predefined screeners (e.g. `all_cryptocurrencies_us`).
    const PREDEFINED_SCREENER_URL: &str =
        "https://query2.finance.yahoo.com/v1/finance/screener/predefined/saved";

    /// Per-symbol OHLCV bars, exchange timestamps, and metadata.
    const CHART_URL: &str = "https://query2.finance.yahoo.com/v8/finance/chart";

    /// Maximum results Yahoo returns per screener page.
    const PAGE_SIZE: usize = 100;

    // ────────────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────────────

    /// Create a new [`YahooFinance`] instance by opening an authenticated session.
    ///
    /// It performs two HTTP requests:
    /// 1. `GET` [`Self::COOKIE_SEED_URL`] — populates the cookie jar.
    /// 2. `GET` [`Self::CRUMB_URL`]       — retrieves the CSRF crumb.
    pub async fn new() -> DataResult<Self> {
        let client = HttpClient::new()?;
        let crumb = Self::fetch_crumb(&client).await?;

        info!("Yahoo Finance session established");
        Ok(Self {
            client,
            crumb,
        })
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private API
    // ────────────────────────────────────────────────────────────────────────

    /// Seed the cookie jar and retrieve a fresh CSRF crumb.
    ///
    /// `fc.yahoo.com` commonly returns 404 but still sets the required
    /// session cookie, so its status code is intentionally ignored.
    async fn fetch_crumb(client: &HttpClient) -> DataResult<String> {
        debug!("Seeding Yahoo session cookie");
        let _ = client.inner.get(Self::COOKIE_SEED_URL).send().await;

        let crumb_resp = client
            .get(Self::CRUMB_URL, None)
            .await
            .map_err(|e| DataError::Auth(format!("Crumb request failed: {e}")))?;

        let crumb = crumb_resp
            .text()
            .await
            .map_err(|e| DataError::Auth(format!("Failed to read crumb: {e}")))?;

        if crumb.is_empty() {
            return Err(DataError::Auth(
                "Yahoo returned an empty crumb — session cookie may be missing".to_owned(),
            ));
        }

        debug!("CSRF crumb acquired");
        Ok(crumb)
    }

    /// Paginate a Yahoo predefined screener by its screener ID.
    #[instrument(skip(self), fields(scr_id, limit))]
    async fn fetch_predefined_screener(
        &self,
        scr_id: &str,
        limit: usize,
    ) -> DataResult<Vec<Asset>> {
        let quotes = paginate(limit, Self::PAGE_SIZE, |batch, offset| async move {
            let resp = self
                .client
                .get(
                    Self::PREDEFINED_SCREENER_URL,
                    Some(&[
                        ("scrIds", scr_id),
                        ("count ", &batch.to_string()),
                        ("start", &offset.to_string()),
                        ("crumb", &self.crumb),
                        ("lang", "en-US"),
                        ("region", "US"),
                    ]),
                )
                .await?;

            Self::parse_quotes(resp).await
        })
        .await?;

        quotes.into_iter().map(Asset::try_from).collect()
    }

    /// Paginate the custom POST screener with an arbitrary query predicate.
    ///
    /// Results are sorted by descending day-volume on the Yahoo side.
    #[instrument(skip(self, query), fields(quote_type, limit))]
    async fn paginate_screener(
        &self,
        quote_type: &str,
        query: &Value,
        limit: usize,
    ) -> DataResult<Vec<Asset>> {
        let quotes = paginate(limit, Self::PAGE_SIZE, |batch, offset| {
            let payload = json!({
                "size": batch,
                "offset": offset,
                "sortField": "dayvolume",
                "sortType": "DESC",
                "quoteType": quote_type,
                "query": query,
                "userId": "",
                "userIdType": "guid",
            });

            async move {
                let resp = self
                    .client
                    .post(
                        Self::SCREENER_URL,
                        &[("crumb", &self.crumb), ("lang", "en-US"), ("region", "US")],
                        &payload,
                    )
                    .await?;

                Self::parse_quotes(resp).await
            }
        })
        .await?;

        quotes.into_iter().map(Asset::try_from).collect()
    }

    /// Validate HTTP status then deserialize a screener response into quotes.
    async fn parse_quotes(resp: reqwest::Response) -> DataResult<Vec<YahooQuote>> {
        let parsed = HttpClient::json::<ScreenerResponse>(resp).await?;

        parsed
            .finance
            .result
            .into_iter()
            .next()
            .map(|r| r.quotes)
            .ok_or(DataError::UnexpectedResponse("empty screener result array".to_owned()))
    }

    /// Map a Yahoo instrument type to [`AssetType`].
    fn parse_asset_type(instrument_type: &str) -> DataResult<AssetType> {
        match instrument_type {
            "EQUITY" => Ok(AssetType::Stocks),
            "ETF" => Ok(AssetType::Etf),
            "CURRENCY" => Ok(AssetType::Forex),
            "CRYPTOCURRENCY" => Ok(AssetType::Crypto),
            other => Err(DataError::UnexpectedResponse(format!("Unknown asset type: {other:?}"))),
        }
    }

    /// Derive `(base, quote)` currency strings from a Yahoo symbol.
    ///
    /// - Equity `"AAPL"`    → `(None, currency)`
    /// - Equity `"PBR-A"`   → `(None, currency)`  (dash is part of the ticker)
    /// - Forex `"EURUSD=X"` → `(Some("EUR"), "USD")`
    /// - Forex `"JPY=X"`    → `(Some("USD"), "JPY")` (implicit USD base)
    /// - Crypto `"BTC-USD"` → `(Some("BTC"), "USD")`
    fn parse_base_quote(
        symbol: &str,
        currency: &str,
        asset_type: AssetType,
    ) -> DataResult<(Option<String>, String)> {
        if let Some(symbol) = symbol.strip_suffix("=X") {
            return match symbol.len() {
                3 => Ok((Some(Currency::USD.to_string()), symbol.to_owned())),
                6 => Ok((Some(symbol[..3].to_owned()), symbol[3..].to_owned())),
                _ => Err(DataError::UnexpectedResponse("invalid symbol".to_owned())),
            };
        }

        // Only treat '-' as a base/quote delimiter for crypto pairs.
        // Stock tickers can contain dashes as part of the name (e.g., "PBR-A", "BRK-B").
        if asset_type == AssetType::Crypto {
            if let Some((base, quote)) = symbol.split_once('-') {
                return Ok((Some(base.to_owned()), quote.to_owned()));
            }
        }

        Ok((None, currency.to_string()))
    }

    /// Convert a canonical symbol to yahoo format.
    fn parse_canonical_symbol(symbol: &str, asset_type: AssetType) -> DataResult<String> {
        match asset_type {
            AssetType::Forex | AssetType::Crypto => {
                let (base, quote) = symbol
                    .split_once('-')
                    .ok_or_else(|| DataError::SymbolNotFound(symbol.to_owned()))?;

                if base == quote {
                    return Err(DataError::SymbolNotFound(symbol.to_owned()));
                }

                if asset_type == AssetType::Forex {
                    if base == Currency::USD.to_string() {
                        Ok(format!("{quote}=X"))
                    } else {
                        Ok(format!("{base}{quote}=X"))
                    }
                } else {
                    Ok(format!("{base}-{quote}"))
                }
            },
            _ => Ok(symbol.to_owned()),
        }
    }

    /// Download one chunk of bars from the Yahoo chart endpoint.
    ///
    /// `chunk_start`/`chunk_end` define the HTTP request window, while
    /// `filter_start`/`filter_end` are the overall requested range used to
    /// filter out bars that fall outside the caller's interest.
    async fn download_chart_chunk(
        &self,
        yahoo_symbol: &str,
        iv: &str,
        chunk_start: u64,
        chunk_end: u64,
        filter_start: u64,
        filter_end: u64,
    ) -> DataResult<Vec<Bar>> {
        let resp = self
            .client
            .get(
                &format!("{}/{}", Self::CHART_URL, yahoo_symbol),
                Some(&[
                    ("period1", chunk_start.to_string().as_str()),
                    ("period2", chunk_end.to_string().as_str()),
                    ("interval", iv),
                    ("crumb", &self.crumb),
                ]),
            )
            .await?;

        let parsed = HttpClient::json::<ChartResponse>(resp).await?;

        if let Some(err) = parsed.chart.error {
            let msg = err.description.unwrap_or_else(|| "unknown chart error".to_owned());
            return Err(DataError::UnexpectedResponse(msg));
        }

        let result = parsed
            .chart
            .result
            .into_iter()
            .next()
            .ok_or_else(|| DataError::UnexpectedResponse("empty chart result".to_owned()))?;

        let timestamps = result.timestamp.unwrap_or_default();
        let indicators = result.indicators.ok_or_else(|| {
            DataError::UnexpectedResponse("no indicators in chart result".to_owned())
        })?;

        let quote = indicators
            .quote
            .into_iter()
            .next()
            .ok_or_else(|| DataError::UnexpectedResponse("no quote indicators".to_owned()))?;

        let adj_close_arr = indicators
            .adjclose
            .and_then(|v| v.into_iter().next())
            .map(|a| a.adjclose)
            .unwrap_or_default();

        let mut bars = vec![];
        for (i, raw_ts) in timestamps.into_iter().enumerate() {
            if raw_ts < 0 {
                continue;
            }

            let open_ts = raw_ts as u64;
            let open = quote.open.get(i).and_then(|v| *v);
            let high = quote.high.get(i).and_then(|v| *v);
            let low = quote.low.get(i).and_then(|v| *v);
            let close = quote.close.get(i).and_then(|v| *v);
            let volume = quote.volume.get(i).and_then(|v| *v);
            let adj = adj_close_arr.get(i).and_then(|v| *v);

            if let (Some(o), Some(h), Some(l), Some(c), Some(v)) = (open, high, low, close, volume)
            {
                if open_ts >= filter_start && open_ts < filter_end {
                    bars.push(Bar {
                        open_ts,
                        close_ts: open_ts,
                        open_ts_exchange: open_ts,
                        open: o,
                        high: h,
                        low: l,
                        close: c,
                        adj_close: adj.unwrap_or(c),
                        volume: v,
                        n_trades: None,
                    });
                }
            }
        }

        debug!("Chunk [{chunk_start}–{chunk_end}] returned {} bars", bars.len());
        Ok(bars)
    }
}

#[async_trait]
impl DataProvider for YahooFinance {
    /// Fetch metadata for a single symbol via the chart endpoint.
    #[instrument(skip(self), fields(%symbol))]
    async fn get_asset(&self, symbol: &Symbol, asset_type: AssetType) -> DataResult<Asset> {
        let symbol = Self::parse_canonical_symbol(symbol, asset_type)?;

        let resp = self
            .client
            .get(
                &format!("{}/{symbol}", Self::CHART_URL),
                Some(&[("range", "1d"), ("interval", "1d"), ("crumb", &self.crumb)]),
            )
            .await
            .map_err(|_| DataError::SymbolNotFound(symbol.clone()))?;

        let parsed = HttpClient::json::<ChartResponse>(resp).await?;

        if let Some(err) = parsed.chart.error {
            let msg = err.description.unwrap_or_else(|| "unknown chart error".to_owned());
            return Err(DataError::UnexpectedResponse(msg));
        }

        let asset = parsed
            .chart
            .result
            .into_iter()
            .next()
            .map(|r| Asset::try_from(r.meta))
            .ok_or(DataError::UnexpectedResponse("empty chart result".to_owned()))??;

        Ok(asset)
    }

    /// Returns the usable download range for an asset at a given interval.
    ///
    /// For intraday intervals the start is clamped to the provider's rolling
    /// history window (e.g. 7 days for 1m), so the value reflects what is actually
    /// downloadable rather than the asset's listing date.
    #[instrument(skip(self), fields(symbol = %asset.symbol, ?interval))]
    async fn get_download_range(&self, asset: Asset, interval: Interval) -> DataResult<(u64, u64)> {
        let symbol = Self::parse_canonical_symbol(&asset.symbol, asset.asset_type)?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let lookback = match interval {
            Interval::OneWeek => 14 * 24 * 3600,
            Interval::OneDay | Interval::FourHours | Interval::OneHour => 2 * 24 * 3600,
            _ => 24 * 3600,
        };

        // Yahoo uses 1wk instead of 1w
        let iv = if interval == Interval::OneWeek {
            "1wk".to_string()
        } else {
            interval.to_string()
        };

        let resp = self
            .client
            .get(
                &format!("{}/{}", Self::CHART_URL, symbol),
                Some(&[
                    ("period1", (now - lookback).to_string().as_str()),
                    ("period2", now.to_string().as_str()),
                    ("interval", iv.as_str()),
                    ("crumb", &self.crumb),
                ]),
            )
            .await?;

        let parsed = HttpClient::json::<ChartResponse>(resp).await?;

        if let Some(err) = parsed.chart.error {
            let msg = err.description.unwrap_or_else(|| "unknown chart error".to_owned());
            return Err(DataError::UnexpectedResponse(msg));
        }

        let meta = parsed
            .chart
            .result
            .into_iter()
            .next()
            .map(|r| r.meta)
            .ok_or_else(|| DataError::UnexpectedResponse("Empty chart result".to_owned()))?;

        let latest_ts = meta.regular_market_time.map(|x| x.max(0) as u64).ok_or_else(|| {
            DataError::UnexpectedResponse(format!("no latest_ts for symbol: {}", symbol))
        })?;

        let cap_secs = match interval {
            Interval::OneMinute => Some(7 * 24 * 3600_i64),
            Interval::FiveMinutes | Interval::FifteenMinutes | Interval::ThirtyMinutes => {
                Some(60 * 24 * 3600)
            },
            Interval::OneHour | Interval::FourHours => Some(730 * 24 * 3600),
            _ => None,
        };

        let earliest_ts = match meta.first_trade_date {
            Some(x) => {
                // Yahoo enforces rolling windows relative to the current wall-clock time,
                // not `regular_market_time` (which can be hours behind if the market
                // hasn't opened yet today).
                if let Some(cap) = cap_secs {
                    x.max(now as i64 - cap).max(0) as u64
                } else {
                    x.max(0) as u64
                }
            },
            None => {
                // Some instruments omit `firstTradeDate`. Fall back to the rolling-window
                // cap when available, otherwise assume data may exist from the Unix epoch
                // and let Yahoo return whatever it has.
                debug!("firstTradeDate missing for symbol {symbol}. Using fallback.");
                cap_secs.map(|cap| (now as i64 - cap).max(0) as u64).unwrap_or(0)
            },
        };

        // End is exclusive — step back one interval so the current (potentially incomplete)
        // bar is excluded and the requested range stays strictly within the provider's
        // rolling window.
        let latest_ts = latest_ts.saturating_sub(interval.minutes() * 60);

        Ok((earliest_ts, latest_ts))
    }

    /// List the most liquid assets for a given asset type, capped at `limit`.
    ///
    /// - **Stocks / ETFs**: fans out across all known exchanges concurrently,
    ///   then picks the top `limit` results by volume × price.
    /// - **Forex**: returns all [`ForexPair`] variants with synthetic timestamps.
    /// - **Crypto**: merges US, EU, and GB predefined screeners concurrently.
    #[instrument(skip(self), fields(?asset_type, limit))]
    async fn list_assets(&self, asset_type: AssetType, limit: usize) -> DataResult<Vec<Asset>> {
        use Exchange::*;

        match asset_type {
            AssetType::Stocks | AssetType::Etf => {
                let (quote_type, operands) = match asset_type {
                    AssetType::Stocks => {
                        (
                            "equity",
                            vec![
                                json!({ "operator": "EQ", "operands": ["quoteType", "equity"] }),
                                // Only select large companies
                                json!({ "operator": "GT", "operands": ["intradaymarketcap", 1000000000] }),
                                // Remove penny stocks
                                json!({ "operator": "GT", "operands": ["regularMarketPrice", 5.0] }),
                            ],
                        )
                    },
                    AssetType::Etf => {
                        ("etf", vec![json!({ "operator": "EQ", "operands": ["quoteType", "etf"] })])
                    },
                    _ => unreachable!(),
                };

                // Fan out across major exchanges concurrently.
                let exchanges = [
                    XAMS, XASX, XETR, XHKG, XJPX, XKRX, XLON, XMAD, XNAS, XNSE, XNYS, XPAR, XSES,
                    XSHG, XSHE, XSWX,
                ];

                let tasks: Vec<_> = exchanges
                    .iter()
                    .map(|ex| {
                        let mut operands = operands.clone();
                        operands.push(
                            json!({ "operator": "EQ", "operands": ["exchange", ex.yahoo_code()] }),
                        );
                        let query = json!({ "operator": "AND", "operands": operands });
                        async move { self.paginate_screener(quote_type, &query, 100).await }
                    })
                    .collect();

                // Log exchange-level failures rather than aborting the whole call.
                let mut assets = Vec::new();
                for result in join_all(tasks).await {
                    match result {
                        Ok(batch) => assets.extend(batch),
                        Err(e) => debug!("Yahoo list_assets exchange error: {e}"),
                    }
                }

                // Select only the top `limit` assets by volume x price
                assets.sort_by(|a, b| {
                    b.volume_price().partial_cmp(&a.volume_price()).unwrap_or(Ordering::Equal)
                });
                assets.truncate(limit);

                Ok(assets)
            },

            AssetType::Forex => {
                let assets: Vec<Asset> = ForexPair::iter()
                    .map(|f| {
                        let symbol = f.to_string();
                        let base = Some(f.base().to_string());
                        let quote = f.quote().to_string();

                        Asset {
                            symbol: canonical_symbol(&symbol, &base, &quote),
                            name: f.to_string(),
                            base,
                            quote,
                            asset_type: AssetType::Forex,
                            exchange: "CCY".to_owned(), // "CCY" is Yahoo's placeholder code for the interbank FX market
                            volume: None,
                            price: None,
                        }
                    })
                    .collect();

                Ok(assets)
            },

            AssetType::Crypto => {
                let (us, eu, gb) = try_join!(
                    self.fetch_predefined_screener("all_cryptocurrencies_us", 100),
                    self.fetch_predefined_screener("all_cryptocurrencies_eu", 100),
                    self.fetch_predefined_screener("all_cryptocurrencies_gb", 100),
                )?;

                let mut assets: Vec<Asset> = us.into_iter().chain(eu).chain(gb).collect();

                assets.sort_by(|a, b| {
                    b.volume_price().partial_cmp(&a.volume_price()).unwrap_or(Ordering::Equal)
                });
                assets.truncate(limit);

                Ok(assets)
            },
        }
    }

    /// Download OHLCV bars for `symbol` at `interval` from `start` to `end`.
    ///
    /// For large date ranges the request is split into chunks (≈ 5 years each
    /// for daily bars) so that Yahoo never has to return an excessively large
    /// payload in a single response.  Results are merged and deduplicated.
    #[instrument(skip(self), fields(%symbol, ?interval, start, end))]
    async fn download_batch(
        &self,
        symbol: &str,
        asset_type: AssetType,
        interval: Interval,
        start: u64,
        end: u64,
    ) -> DataResult<Vec<Bar>> {
        let yahoo_symbol = Self::parse_canonical_symbol(symbol, asset_type)?;

        // Yahoo uses 1wk instead of 1w
        let iv = if interval == Interval::OneWeek {
            "1wk".to_string()
        } else {
            interval.to_string()
        };

        // Clamp `start` to the provider's rolling window at download time, since
        // `get_download_range` may have been called much earlier.
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let buffer = interval.minutes() * 60;
        let start = match interval {
            Interval::OneMinute => start.max(now.saturating_sub(7 * 24 * 3600 - buffer)),
            Interval::FiveMinutes | Interval::FifteenMinutes | Interval::ThirtyMinutes => {
                start.max(now.saturating_sub(60 * 24 * 3600 - buffer))
            },
            Interval::OneHour | Interval::FourHours => {
                start.max(now.saturating_sub(730 * 24 * 3600 - buffer))
            },
            _ => start,
        };

        // Yahoo's chart API can return ~10 000 bars per request.  Choose
        // chunk sizes that keep most real-world downloads to a single request
        // while still splitting truly enormous ranges.
        let chunk_secs: u64 = if interval.minutes() >= 24 * 60 {
            20 * 365 * 86400 // ~20 years  (≈5 000 daily bars)
        } else if interval.minutes() >= 60 {
            2 * 365 * 86400 // ~2 years   (≈4 400 hourly bars)
        } else {
            30 * 86400 // 30 days
        };

        // Build all chunk ranges up-front so they can be fetched concurrently.
        let mut chunks: Vec<(u64, u64)> = Vec::new();
        let mut cursor = start;
        while cursor < end {
            let chunk_end = (cursor + chunk_secs).min(end);
            chunks.push((cursor, chunk_end));
            cursor = chunk_end;
        }

        let results =
            join_all(chunks.iter().map(|&(cs, ce)| {
                self.download_chart_chunk(&yahoo_symbol, &iv, cs, ce, start, end)
            }))
            .await;

        let mut all_bars: Vec<Bar> = Vec::new();
        let mut first_err: Option<DataError> = None;
        for result in results {
            match result {
                Ok(mut b) => all_bars.append(&mut b),
                Err(e) => {
                    debug!("Yahoo chunk download error: {e}");
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                },
            }
        }

        // If every chunk failed, propagate the first error.
        if all_bars.is_empty() {
            if let Some(e) = first_err {
                return Err(e);
            }
        }

        all_bars.sort_by_key(|b| b.open_ts);
        all_bars.dedup_by_key(|b| b.open_ts);

        info!("Downloaded {} bars.", all_bars.len());
        Ok(all_bars)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Yahoo API objects
// ────────────────────────────────────────────────────────────────────────────

/// Raw quote entry from the Yahoo screener endpoint.
///
/// All fields are `Option` — Yahoo omits them inconsistently across asset types.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YahooQuote {
    /// Yahoo ticker symbol.
    pub symbol: String,

    /// Short display name, if available.
    pub short_name: Option<String>,

    /// Full legal name, if available.
    pub long_name: Option<String>,

    /// ISO 4217 quote currency reported by Yahoo.
    pub currency: Option<String>,

    /// Asset class string.
    pub quote_type: Option<String>,

    /// Short exchange code.
    pub exchange: Option<String>,

    /// Most recent session volume in units of the base asset.
    pub regular_market_volume: Option<u64>,

    /// Most recent traded price in `currency`.
    pub regular_market_price: Option<f64>,
}

impl TryFrom<YahooQuote> for Asset {
    type Error = DataError;

    fn try_from(q: YahooQuote) -> DataResult<Self> {
        let currency = q.currency.ok_or(DataError::UnexpectedResponse(format!(
            "no currency for symbol: {}",
            q.symbol
        )))?;

        let asset_type = q
            .quote_type
            .as_deref()
            .ok_or_else(|| {
                DataError::UnexpectedResponse(format!("no quote_type for symbol: {}", q.symbol))
            })
            .and_then(YahooFinance::parse_asset_type)?;

        let (base, quote) = YahooFinance::parse_base_quote(&q.symbol, &currency, asset_type)?;

        let exchange = {
            let s = q.exchange.ok_or_else(|| {
                DataError::UnexpectedResponse(format!("no exchange for symbol: {}", q.symbol))
            })?;
            Exchange::from_yahoo_code(&s).map(|ex| ex.to_string()).unwrap_or(s)
        };

        Ok(Asset {
            symbol: canonical_symbol(&q.symbol, &base, &quote),
            name: q.short_name.or(q.long_name).unwrap_or_else(|| quote.clone()),
            base,
            quote,
            asset_type,
            exchange,
            price: q.regular_market_price,
            volume: q.regular_market_volume,
        })
    }
}

// ── Screener response envelope ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ScreenerResponse {
    finance: ScreenerFinance,
}

#[derive(Debug, Deserialize)]
struct ScreenerFinance {
    result: Vec<ScreenerResult>,
}

#[derive(Debug, Deserialize)]
struct ScreenerResult {
    quotes: Vec<YahooQuote>,
}

// ── Chart response envelope ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: ChartBody,
}

#[derive(Debug, Deserialize)]
struct ChartBody {
    #[serde(default)]
    result: Vec<ChartResult>,
    error: Option<ChartError>,
}

#[derive(Debug, Deserialize)]
struct ChartError {
    #[allow(dead_code)]
    code: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    /// Symbol metadata.
    meta: ChartMeta,

    /// Bar open timestamps (Unix seconds).
    timestamp: Option<Vec<i64>>,

    /// OHLCV indicator arrays.
    indicators: Option<ChartIndicators>,
}

/// Symbol metadata returned by the Yahoo chart endpoint.
///
/// Used to derive [`Asset`] fields; all optional because Yahoo omits fields
/// inconsistently (particularly for less-traded or delisted instruments).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChartMeta {
    /// Yahoo ticker symbol.
    symbol: String,

    /// Short display name.
    short_name: Option<String>,

    /// Full legal name.
    long_name: Option<String>,

    /// ISO 4217 quote currency.
    currency: Option<String>,

    /// Asset class string.
    instrument_type: Option<String>,

    /// Short exchange code.
    exchange_name: Option<String>,

    /// Unix timestamp of the first ever traded bar.
    first_trade_date: Option<i64>,

    /// Unix timestamp of the most recent market event.
    regular_market_time: Option<i64>,

    /// Most recent session volume.
    regular_market_volume: Option<u64>,

    /// Most recent traded price.
    regular_market_price: Option<f64>,
}

impl TryFrom<ChartMeta> for Asset {
    type Error = DataError;

    fn try_from(m: ChartMeta) -> DataResult<Self> {
        let currency = m.currency.ok_or(DataError::UnexpectedResponse(format!(
            "no currency for symbol: {}",
            m.symbol
        )))?;

        let asset_type = m
            .instrument_type
            .as_deref()
            .ok_or_else(|| {
                DataError::UnexpectedResponse(format!(
                    "no instrument type for symbol: {}",
                    m.symbol
                ))
            })
            .and_then(YahooFinance::parse_asset_type)?;

        let (base, quote) = YahooFinance::parse_base_quote(&m.symbol, &currency, asset_type)?;

        let exchange = {
            let s = m.exchange_name.ok_or_else(|| {
                DataError::UnexpectedResponse(format!("no exchange for symbol: {}", m.symbol))
            })?;
            Exchange::from_yahoo_code(&s).map(|ex| ex.to_string()).unwrap_or(s)
        };

        Ok(Asset {
            symbol: canonical_symbol(&m.symbol, &base, &quote),
            name: m.short_name.or(m.long_name).unwrap_or_else(|| quote.clone()),
            base,
            quote,
            asset_type,
            exchange,
            price: m.regular_market_price,
            volume: m.regular_market_volume,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ChartIndicators {
    quote: Vec<ChartQuote>,
    adjclose: Option<Vec<ChartAdjClose>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ChartQuote {
    open: Vec<Option<f64>>,
    high: Vec<Option<f64>>,
    low: Vec<Option<f64>>,
    close: Vec<Option<f64>>,
    volume: Vec<Option<f64>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ChartAdjClose {
    adjclose: Vec<Option<f64>>,
}
