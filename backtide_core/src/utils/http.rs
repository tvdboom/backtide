//! Utilities for HTTP requests.

use reqwest::cookie::Jar;
use reqwest::Client;
use serde::Serialize;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

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
    /// Not retried — a 4xx/5xx is a definitive answer from the server.
    #[error("Server returned {status}: {source}")]
    Status {
        status: reqwest::StatusCode,
        #[source]
        source: reqwest::Error,
    },

    /// The response body could not be decoded (e.g. invalid JSON or charset).
    #[error("Failed to decode response body: {0}")]
    Decode(#[source] reqwest::Error),

    /// The HTTP layer succeeded but the response structure was unexpected
    /// (e.g. a missing or empty result array in the parsed body).
    #[error("Unexpected response payload: {0}")]
    UnexpectedPayload(String),
}

/// HTTP client wrapper with retry logic.
pub struct HttpClient {
    /// `reqwest` client.
    pub(crate) inner: Client,
}

impl HttpClient {
    /// Number of times to retry a failed HTTP request.
    const MAX_RETRIES: u32 = 3;

    /// How long to wait between retry attempts.
    const RETRY_SLEEP: Duration = Duration::from_millis(100);

    /// User-agent sent with every request.
    const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

    // ────────────────────────────────────────────────────────────────────────
    // Public API
    // ────────────────────────────────────────────────────────────────────────

    pub fn new() -> Result<Self, HttpError> {
        let inner = Client::builder()
            .cookie_provider(Arc::new(Jar::default()))
            .user_agent(Self::USER_AGENT)
            .build()
            .map_err(HttpError::ClientBuild)?;

        Ok(Self {
            inner,
        })
    }

    /// Send a `GET` request to `url`.
    pub async fn get(
        &self,
        url: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<reqwest::Response, HttpError> {
        self.retry(|| {
            let mut req = self.inner.get(url);
            if let Some(p) = params {
                req = req.query(p);
            }
            req.send()
        })
        .await
    }

    /// Send a `POST` request with a JSON body and optional query parameters.
    pub async fn post<B: Serialize>(
        &self,
        url: &str,
        params: &[(&str, &str)],
        body: &B,
    ) -> Result<reqwest::Response, HttpError> {
        self.retry(|| self.inner.post(url).query(params).json(body).send()).await
    }

    // ────────────────────────────────────────────────────────────────────────
    // Private API
    // ────────────────────────────────────────────────────────────────────────

    /// Execute an async request factory up to [`Self::MAX_RETRIES`] times.
    async fn retry<F, Fut>(&self, mut f: F) -> Result<reqwest::Response, HttpError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<reqwest::Response, reqwest::Error>>,
    {
        let mut last_err: Option<HttpError> = None;

        for attempt in 0..Self::MAX_RETRIES {
            match f().await {
                Ok(resp) => {
                    let status = resp.status();

                    if status.is_success() {
                        return Ok(resp);
                    } else if status.is_client_error() {
                        // 4xx: no point retrying, the request itself is wrong.
                        return Err(HttpError::Status {
                            status,
                            source: resp.error_for_status().unwrap_err(),
                        });
                    } else {
                        // 5xx: server-side problem, worth retrying.
                        last_err = Some(HttpError::Status {
                            status,
                            source: resp.error_for_status().unwrap_err(),
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
                sleep(Self::RETRY_SLEEP).await;
            }
        }

        Err(last_err.expect("loop runs at least once"))
    }

    /// Deserialize a response body as JSON, mapping failures to [`HttpError::Decode`].
    pub async fn json<T: serde::de::DeserializeOwned>(
        resp: reqwest::Response,
    ) -> Result<T, HttpError> {
        resp.json::<T>().await.map_err(HttpError::Decode)
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
            break; // source exhausted
        }
    }

    Ok(results)
}
