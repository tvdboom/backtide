//! Yahoo Finance authentication.
//!
//! Yahoo Finance requires a session cookie obtained from `fc.yahoo.com` and a
//! crumb token from `query2.finance.yahoo.com/v1/test/getcrumb`.  Both must be
//! attached to every subsequent screener request.

use std::sync::Arc;

use reqwest::cookie::Jar;
use reqwest::Client;

use crate::error::MarketDataError;

/// Holds a validated crumb and the cookie jar that produced it.
#[derive(Debug, Clone)]
pub struct YahooAuth {
    pub crumb: String,
    pub jar: Arc<Jar>,
}

impl YahooAuth {
    /// Fetch a fresh crumb and populate the cookie jar.
    ///
    /// Makes two requests:
    /// 1. `GET https://fc.yahoo.com` — seeds the cookie jar.
    /// 2. `GET https://query2.finance.yahoo.com/v1/test/getcrumb` — returns the crumb.
    ///
    /// # Errors
    ///
    /// Returns [`MarketDataError::Auth`] if either request fails or the crumb
    /// response body is empty.
    pub async fn fetch() -> Result<Self, MarketDataError> {
        let jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(jar.clone())
            .user_agent("Mozilla/5.0")
            .build()
            .map_err(|e| MarketDataError::Auth(e.to_string()))?;

        // Seed cookies
        client
            .get("https://fc.yahoo.com")
            .send()
            .await
            .map_err(|e| MarketDataError::Auth(format!("Cookie seed failed: {e}")))?;

        // Fetch crumb
        let crumb = client
            .get("https://query2.finance.yahoo.com/v1/test/getcrumb")
            .send()
            .await
            .map_err(|e| MarketDataError::Auth(format!("Crumb request failed: {e}")))?
            .text()
            .await
            .map_err(|e| MarketDataError::Auth(format!("Crumb read failed: {e}")))?;

        if crumb.is_empty() {
            return Err(MarketDataError::Auth("Empty crumb response".to_string()));
        }

        Ok(Self {
            crumb,
            jar,
        })
    }
}
