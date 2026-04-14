//! Utilities for HTTP requests.

use reqwest::cookie::Jar;
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{sleep, Instant};
use tracing::warn;

/// Errors that can occur during an HTTP request or paginated fetch.
#[derive(Debug, Error)]
pub enum HttpError {
    /// Failed to build the underlying `reqwest` client.
    #[error("Failed to build HTTP client: {0}")]
    ClientBuild(#[source] reqwest::Error),

    /// The request failed on every retry attempt.
    #[error("Request failed after {attempts} attempt(s): {source}")]
    Exhausted {
        attempts: u32,
        #[source]
        source: reqwest::Error,
    },

    /// The server returned a non-2xx status code.
    ///
    /// `body` contains a human-readable error message extracted from the
    /// response body when possible, falling back to the raw status + body text.
    #[error("{body}")]
    Status {
        status: StatusCode,
        body: String,
    },

    /// The response body could not be decoded.
    #[error("Failed to decode response body: {0}")]
    Decode(#[source] reqwest::Error),

    /// The HTTP layer succeeded but the response structure was unexpected.
    #[error("Unexpected response payload: {0}")]
    UnexpectedPayload(String),
}

/// HTTP client wrapper with retry logic, concurrency limiting, and rate limiting.
pub struct HttpClient {
    /// The `reqwest` client.
    pub(crate) inner: Client,

    /// Limits the number of concurrent in-flight HTTP requests.
    semaphore: Arc<Semaphore>,

    /// Minimum interval between consecutive requests (rate limiter).
    /// When `None`, no rate limiting is applied beyond the concurrency semaphore.
    min_request_interval: Option<Duration>,

    /// Tracks the last time a request was dispatched. Used together with
    /// `min_request_interval` to throttle the overall request rate.
    last_request: Arc<Mutex<Instant>>,
}

/// Per-provider tunables for [`HttpClient`].
pub struct HttpClientConfig {
    /// Maximum number of concurrent in-flight HTTP requests.
    pub max_concurrent_requests: usize,

    /// Maximum time to wait for a TCP connection to be established.
    pub connect_timeout: Duration,

    /// Maximum time to wait for an entire HTTP response (including body).
    pub request_timeout: Duration,

    /// Minimum interval between consecutive HTTP requests (rate limiter).
    ///
    /// When `None` (the default), requests are only gated by the concurrency
    /// semaphore. Set this to e.g. `Duration::from_millis(200)` to cap the
    /// throughput at ~5 req/s regardless of how many tasks are waiting.
    pub min_request_interval: Option<Duration>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            min_request_interval: None,
        }
    }
}

impl HttpClient {
    /// Number of times to retry a failed HTTP request.
    const MAX_RETRIES: u32 = 5;

    /// Base delay for exponential back-off (doubles each attempt).
    const BACKOFF_BASE: Duration = Duration::from_millis(500);

    /// Maximum value we honor from a `Retry-After` header.
    const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

    /// User-agent sent with every request.
    const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

    // ────────────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────────────

    /// Create a new [`HttpClient`] with default settings.
    pub fn new() -> Result<Self, HttpError> {
        Self::with_config(HttpClientConfig::default())
    }

    /// Create a new [`HttpClient`] with provider-specific settings.
    pub fn with_config(config: HttpClientConfig) -> Result<Self, HttpError> {
        let inner = Client::builder()
            .cookie_provider(Arc::new(Jar::default()))
            .user_agent(Self::USER_AGENT)
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .pool_max_idle_per_host(config.max_concurrent_requests)
            .build()
            .map_err(HttpError::ClientBuild)?;

        Ok(Self {
            inner,
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_requests)),
            min_request_interval: config.min_request_interval,
            last_request: Arc::new(Mutex::new(Instant::now())),
        })
    }

    /// Send a `GET` request to `url`.
    pub async fn get(
        &self,
        url: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<Response, HttpError> {
        self.retry(|| async {
            self.throttle().await;
            let _permit = self.semaphore.acquire().await.expect("semaphore closed");
            let mut req = self.inner.get(url);
            if let Some(p) = params {
                req = req.query(p);
            }
            req.send().await
        })
        .await
    }

    /// Send a `POST` request with a JSON body and optional query parameters.
    pub async fn post<B: Serialize + Sync>(
        &self,
        url: &str,
        params: &[(&str, &str)],
        body: &B,
    ) -> Result<Response, HttpError> {
        self.retry(|| async {
            self.throttle().await;
            let _permit = self.semaphore.acquire().await.expect("semaphore closed");
            self.inner.post(url).query(params).json(body).send().await
        })
        .await
    }

    /// Deserialize a response body as JSON, mapping failures to [`HttpError::Decode`].
    pub async fn json<T: DeserializeOwned>(resp: Response) -> Result<T, HttpError> {
        resp.json::<T>().await.map_err(HttpError::Decode)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private API
    // ────────────────────────────────────────────────────────────────────────

    /// Enforce the minimum interval between consecutive requests.
    ///
    /// If `min_request_interval` is set, this method sleeps until enough time
    /// has passed since the last dispatched request, then updates the timestamp.
    /// This serializes the rate-limiting decision but not the actual I/O.
    async fn throttle(&self) {
        if let Some(interval) = self.min_request_interval {
            let mut last = self.last_request.lock().await;
            let elapsed = last.elapsed();
            if elapsed < interval {
                sleep(interval - elapsed).await;
            }
            *last = Instant::now();
        }
    }

    /// Execute an async request factory up to [`Self::MAX_RETRIES`] times.
    async fn retry<F, Fut>(&self, mut f: F) -> Result<Response, HttpError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<Response, reqwest::Error>>,
    {
        let mut last_err: Option<HttpError> = None;

        for attempt in 0..Self::MAX_RETRIES {
            match f().await {
                Ok(resp) => {
                    let status = resp.status();

                    if status.is_success() {
                        return Ok(resp);
                    } else if status == StatusCode::TOO_MANY_REQUESTS {
                        // 429: honor Retry-After header or use exponential back-off.
                        let delay = resp
                            .headers()
                            .get(reqwest::header::RETRY_AFTER)
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .map(Duration::from_secs)
                            .unwrap_or_else(|| Self::BACKOFF_BASE * 2u32.saturating_pow(attempt))
                            .min(Self::MAX_RETRY_AFTER);

                        warn!(
                            "Rate limited (429). Backing off for {:.1}s (attempt {}/{})",
                            delay.as_secs_f64(),
                            attempt + 1,
                            Self::MAX_RETRIES,
                        );

                        last_err = Some(HttpError::Status {
                            status,
                            body: format!("Rate limited ({status})"),
                        });

                        sleep(delay).await;
                        continue;
                    } else if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
                    {
                        // 401/403: Yahoo often uses these as rate-limit signals
                        // (expired crumb, temporary IP block). Retry with backoff.
                        last_err = Some(HttpError::Status {
                            status,
                            body: format!("Access denied ({status}), retrying..."),
                        });
                    } else if status.is_client_error() {
                        // Other 4xx: no point retrying, the request itself is wrong.
                        let body = resp.text().await.unwrap_or_default();
                        let body = Self::extract_api_message(&body)
                            .unwrap_or(format!("Server returned {status}: {body}"));

                        return Err(HttpError::Status {
                            status,
                            body,
                        });
                    } else {
                        // 5xx: server-side problem, worth retrying.
                        last_err = Some(HttpError::Status {
                            status,
                            body: format!("Server error ({status})"),
                        });
                    }
                },
                Err(e) => {
                    last_err = Some(HttpError::Exhausted {
                        attempts: attempt + 1,
                        source: e,
                    });
                },
            }

            if attempt < Self::MAX_RETRIES - 1 {
                sleep(Self::BACKOFF_BASE * 2u32.saturating_pow(attempt)).await;
            }
        }

        Err(last_err.expect("loop runs at least once"))
    }

    /// Try to extract a human-readable error message from a JSON error body.
    fn extract_api_message(body: &str) -> Option<String> {
        let json: serde_json::Value = serde_json::from_str(body).ok()?;

        let candidates = [
            json.pointer("/chart/error/description"),
            json.pointer("/error/description"),
            json.pointer("/error/message"),
            json.pointer("/error"),
            json.pointer("/message"),
        ];

        let result =
            candidates.into_iter().flatten().find_map(|v| v.as_str().map(|s| s.to_owned()));
        result
    }
}

/// Drive a paginated endpoint until `limit` items are collected.
pub async fn paginate<T, E, F, Fut>(
    limit: usize,
    page_size: usize,
    mut fetch_page: F,
) -> Result<Vec<T>, E>
where
    F: FnMut(usize, usize) -> Fut,
    Fut: Future<Output = Result<Vec<T>, E>>,
{
    let mut results = Vec::with_capacity(limit);
    let mut offset = 0;

    while results.len() < limit {
        let batch = page_size.min(limit - results.len());
        let page = fetch_page(batch, offset).await?;
        let n = page.len();
        results.extend(page);
        offset += n;
        if n < batch {
            break;
        }
    }

    Ok(results)
}
