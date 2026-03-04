//! Low-level Yahoo Finance HTTP helpers.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::error::MarketDataError;
use crate::models::YahooQuote;

/// Number of times to retry a failed HTTP request.
const MAX_RETRIES: u32 = 3;

/// Milliseconds to wait between retries.
const RETRY_SLEEP_MS: u64 = 100;

/// Maximum number of results Yahoo returns in a single screener page.
const PAGE_SIZE: usize = 100;

const SCREENER_URL: &str = "https://query2.finance.yahoo.com/v1/finance/screener";
const PREDEFINED_URL: &str =
    "https://query2.finance.yahoo.com/v1/finance/screener/predefined/saved";

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

// ─── Retry macro ─────────────────────────────────────────────────────────────

/// Retry an async expression up to [`MAX_RETRIES`] times with [`RETRY_SLEEP_MS`]
/// ms between attempts. Uses a macro to avoid async-closure lifetime issues.
macro_rules! retry {
    ($expr:expr) => {{
        let mut last_err: Option<MarketDataError> = None;
        let mut result = None;

        for attempt in 0..MAX_RETRIES {
            match $expr {
                // ← no .await here
                Ok(v) => {
                    result = Some(v);
                    break;
                },
                Err(e) => {
                    if attempt < MAX_RETRIES - 1 {
                        sleep(Duration::from_millis(RETRY_SLEEP_MS)).await;
                    }
                    last_err = Some(e);
                },
            }
        }

        result.ok_or_else(|| last_err.expect("at least one attempt"))
    }};
}

// ─── Shared response parsing ──────────────────────────────────────────────────

/// Parse a [`reqwest::Response`] into a list of [`YahooQuote`]s.
async fn parse_quotes(resp: reqwest::Response) -> Result<Vec<YahooQuote>, MarketDataError> {
    resp.error_for_status_ref().map_err(|e| MarketDataError::Http {
        retries: MAX_RETRIES,
        source: e,
    })?;

    let parsed = resp.json::<ScreenerResponse>().await.map_err(|e| MarketDataError::Http {
        retries: MAX_RETRIES,
        source: e,
    })?;

    parsed
        .finance
        .result
        .into_iter()
        .next()
        .map(|r| r.quotes)
        .ok_or_else(|| MarketDataError::UnexpectedResponse("Empty result array".to_string()))
}

// ─── Predefined screener ──────────────────────────────────────────────────────

async fn fetch_predefined_page(
    client: &Client,
    crumb: &str,
    scr_id: &str,
    count: usize,
    start: usize,
) -> Result<Vec<YahooQuote>, MarketDataError> {
    let resp = retry!(client
        .get(PREDEFINED_URL)
        .query(&[
            ("scrIds", scr_id),
            ("count", &count.to_string()),
            ("start", &start.to_string()),
            ("crumb", crumb),
        ])
        .send()
        .await // ← await in call site
        .map_err(|e| MarketDataError::Http {
            retries: MAX_RETRIES,
            source: e,
        }))?;

    parse_quotes(resp).await
}

/// Paginate through a predefined screener until `limit` results are collected.
pub async fn fetch_predefined(
    client: &Client,
    crumb: &str,
    scr_id: &str,
    limit: usize,
) -> Result<Vec<YahooQuote>, MarketDataError> {
    let mut results = Vec::with_capacity(limit);
    let mut offset = 0;

    while results.len() < limit {
        let batch = PAGE_SIZE.min(limit - results.len());
        let page = fetch_predefined_page(client, crumb, scr_id, batch, offset).await?;
        let n = page.len();
        results.extend(page);
        offset += n;
        if n < batch {
            break;
        }
    }

    Ok(results)
}

// ─── POST screener ────────────────────────────────────────────────────────────

async fn fetch_screener_page(
    client: &Client,
    crumb: &str,
    query: &Value,
    count: usize,
    offset: usize,
) -> Result<Vec<YahooQuote>, MarketDataError> {
    let payload = json!({
        "size": count,
        "offset": offset,
        "sortField": "dayvolume",
        "sortType": "DESC",
        "quoteType": "EQUITY",
        "query": query,
        "userId": "",
        "userIdType": "guid",
    });

    let resp = retry!(client
        .post(SCREENER_URL)
        .query(&[("crumb", crumb), ("lang", "en-US"), ("region", "US")])
        .json(&payload)
        .send()
        .await // ← await in call site
        .map_err(|e| MarketDataError::Http {
            retries: MAX_RETRIES,
            source: e,
        }))?;

    parse_quotes(resp).await
}

/// Fetch the top `limit` equities for the given exchange codes, sorted by volume.
pub async fn fetch_by_exchanges(
    client: &Client,
    crumb: &str,
    exchanges: &[&str],
    limit: usize,
) -> Result<Vec<YahooQuote>, MarketDataError> {
    let query = json!({
        "operator": "AND",
        "operands": [
            {"operator": "EQ", "operands": ["quoteType", "EQUITY"]},
            {
                "operator": "OR",
                "operands": exchanges
                    .iter()
                    .map(|ex| json!({"operator": "EQ", "operands": ["exchange", ex]}))
                    .collect::<Vec<_>>()
            }
        ]
    });

    paginate_screener(client, crumb, &query, limit).await
}

/// Fetch the top `limit` assets for the given `quoteType`, sorted by volume.
pub async fn fetch_by_quote_type(
    client: &Client,
    crumb: &str,
    quote_type: &str,
    limit: usize,
) -> Result<Vec<YahooQuote>, MarketDataError> {
    let query = json!({
        "operator": "EQ",
        "operands": ["quoteType", quote_type]
    });

    paginate_screener(client, crumb, &query, limit).await
}

async fn paginate_screener(
    client: &Client,
    crumb: &str,
    query: &Value,
    limit: usize,
) -> Result<Vec<YahooQuote>, MarketDataError> {
    let mut results = Vec::with_capacity(limit);
    let mut offset = 0;

    while results.len() < limit {
        let batch = PAGE_SIZE.min(limit - results.len());
        let page = fetch_screener_page(client, crumb, query, batch, offset).await?;
        let n = page.len();
        results.extend(page);
        offset += n;
        if n < batch {
            break;
        }
    }

    Ok(results)
}
