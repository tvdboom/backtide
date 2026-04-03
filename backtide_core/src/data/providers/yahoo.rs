//! Yahoo Finance data provider.
//!
//! Authenticates via a session cookie + CSRF crumb, then exposes asset
//! discovery (screener) and per-symbol metadata (chart endpoint).

use crate::constants::Symbol;
use crate::data::errors::{DataError, DataResult};
use crate::data::models::asset::Asset;
use crate::data::models::asset_type::AssetType;
use crate::data::models::currency::Currency;
use crate::data::models::exchange::Exchange;
use crate::data::models::forex::ForexPair;
use crate::data::providers::traits::DataProvider;
use crate::data::utils::canonical_symbol;
use crate::utils::http::{paginate, HttpClient};
use async_trait::async_trait;
use chrono::Utc;
use futures::future::join_all;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cmp::Ordering;
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

        Ok(quotes.into_iter().map(Asset::try_from).collect::<DataResult<_>>()?)
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

        Ok(quotes.into_iter().map(Asset::try_from).collect::<DataResult<_>>()?)
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
            other => Err(DataError::UnexpectedResponse(format!("unknown asset type: {other:?}"))),
        }
    }

    /// Derive `(base, quote)` currency strings from a Yahoo symbol.
    ///
    /// - Equity `"AAPL"`    → `(None, currency)`
    /// - Forex `"EURUSD=X"` → `(Some("EUR"), "USD")`
    /// - Forex `"JPY=X"`    → `(Some("USD"), "JPY")` (implicit USD base)
    /// - Crypto `"BTC-USD"` → `(Some("BTC"), "USD")`
    fn parse_base_quote(symbol: &str, currency: &str) -> DataResult<(Option<String>, String)> {
        if let Some(symbol) = symbol.strip_suffix("=X") {
            return match symbol.len() {
                3 => Ok((Some(Currency::USD.to_string()), symbol.to_owned())),
                6 => Ok((Some(symbol[..3].to_owned()), symbol[3..].to_owned())),
                _ => Err(DataError::UnexpectedResponse("invalid symbol".to_owned())),
            };
        }

        if let Some((base, quote)) = symbol.split_once('-') {
            return Ok((Some(base.to_owned()), quote.to_owned()));
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

        let asset = parsed
            .chart
            .result
            .into_iter()
            .next()
            .map(|r| Asset::try_from(r.meta))
            .ok_or(DataError::UnexpectedResponse("empty chart result".to_owned()))??;

        Ok(asset)
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
                    XAMS, XASX, XETR, XHKG, XJPX, XKRX, XLON, XNAS, XNSE, XNYS, XPAR, XSES, XSHG,
                    XSHE, XSWX,
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
                            earliest_ts: Some(0),
                            latest_ts: Some(Utc::now().timestamp() as u64),
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

        let (base, quote) = YahooFinance::parse_base_quote(&q.symbol, &currency)?;

        let asset_type = q
            .quote_type
            .as_deref()
            .ok_or_else(|| {
                DataError::UnexpectedResponse(format!("no quote_type for symbol: {}", q.symbol))
            })
            .and_then(YahooFinance::parse_asset_type)?;

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
            earliest_ts: None,
            latest_ts: None,
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
    result: Vec<ChartResult>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    meta: ChartMeta,
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

        let (base, quote) = YahooFinance::parse_base_quote(&m.symbol, &currency)?;

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
            earliest_ts: m.first_trade_date.map(|v| v.max(0) as u64),
            latest_ts: m.regular_market_time.map(|v| v.max(0) as u64),
        })
    }
}
