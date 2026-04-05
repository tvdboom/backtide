//! Configuration module.
//!
//! Config is loaded once into a process-wide singleton ([`Config::get`]) from
//! the first `backtide.{toml,yaml,yml,json}` file found in the working
//! directory or its parent. If no file is found, defaults are used.
//!
//! ## Structure
//!
//! | Section     | Purpose                                              |
//! |-------------|------------------------------------------------------|
//! | `[general]` | Portfolio-wide settings                              |
//! | `[data]`    | Data fetching and storage settings                   |
//! | `[display]` | UI / Streamlit app                                   |

use crate::config::errors::ConfigResult;
use crate::config::interface::{DataConfig, DisplayConfig, GeneralConfig};
use crate::config::utils::fetch_config;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Process-wide configuration singleton.
pub static CONFIG: OnceLock<Config> = OnceLock::new();

/// Backtide configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Portfolio-wide settings.
    pub general: GeneralConfig,

    /// Settings that control how market data is fetched and stored.
    pub data: DataConfig,

    /// Settings that control how values are presented in the frontend.
    pub display: DisplayConfig,
}

impl Config {
    /// Return a `&'static` reference to the global configuration.
    ///
    /// Initializes from disk on first call; subsequent calls are free.
    pub fn get() -> ConfigResult<&'static Config> {
        // Replace block with get_or_try_init when it becomes stable
        if let Some(cfg) = CONFIG.get() {
            Ok(cfg)
        } else {
            let _ = CONFIG.set(fetch_config()?);
            Ok(CONFIG.get().unwrap())
        }
    }
}
