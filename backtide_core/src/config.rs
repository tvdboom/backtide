//! Backtide configuration module.
//!
//! Owns a process-wide [Config] singleton initialized at startup.
//! After that point every caller gets a cheap `&'static` reference
//! through [config()].

use crate::constants::{DEFAULT_CONFIG_FILE_NAME, DEFAULT_STORAGE_PATH};
use crate::ingestion::provider::Provider;
use crate::models::currency::Currency;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pythonize::pythonize;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use thiserror::Error;

/// Process-wide configuration singleton.
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Return a `&'static` reference to the global configuration.
pub fn config() -> &'static Config {
    CONFIG.get_or_init(fetch_config)
}

/// Errors that can occur while loading configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error reading config: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parse failure.
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    /// JSON parse failure.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML parse failure.
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yml::Error),

    /// The file extension is not one of `.toml`, `.yaml`, `.yml`, `.json`.
    #[error("unsupported config format '{0}'; expected toml, yaml, yml, or json")]
    UnsupportedFormat(String),

    /// [set_config] was called after the singleton was set.
    #[error("The configuration has already been used; set_config cannot be called anymore.")]
    AlreadySet,
}

/// Backtide configuration.
///
/// !!! warning
///     The class has no constructor. An instance is returned from the
///     [get_config] and [load_config] functions.
///
/// Attributes
/// ----------
/// base_currency : str, default="EUR"
///     Currency (ISO 4217 code) that all prices are normalized to.
///
/// ingestion : IngestionConfig
///     Settings that control how market data is fetched and stored.
///
///     - storage_path : str, default = ".backtide/database.duckdb"
///       Location to store the primary database file.
///     - providers : ProviderConfig
///       Which data provider to use per asset class (`stocks`, `etf`, `forex`, `crypto`).
///
/// display : DisplayConfig
///     Settings that control how values are presented in the application interface.
///
///     - date_format : str, default="%d-%m-%Y"
///     `strftime`-compatible date format string.
///     - timezone : str, default=None
///     IANA timezone name in which to display the timestamps. `None` to use system's
///     local timezone.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
///
/// Examples
/// --------
/// ```pycon
/// from backtide.config import get_config, set_config
///
/// # Load the current configuration and change a value
/// cfg = get_config()
/// cfg.ingestion.providers.crypto = "kraken"
///
/// # Update backtide's configuration
/// set_config(cfg)
///
/// # Check that the provider is indeed "kraken" now
/// cfg = get_config()
/// print(cfg.ingestion.providers.crypto)
/// ```
#[pyclass(get_all, set_all, from_py_object, module = "backtide.config")]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// ISO 4217 code that all prices are normalized to, e.g. `"USD"`.
    pub base_currency: Currency,

    /// Settings that control how market data is fetched and stored.
    pub ingestion: IngestionConfig,

    /// Settings that control how values are presented in the frontend.
    pub display: DisplayConfig,
}

#[pymethods]
impl Config {
    fn __repr__(&self) -> String {
        format!(
            "Config(base_currency={:?}, ingestion={}, display={})",
            self.base_currency.to_string(),
            self.ingestion.__repr__(),
            self.display.__repr__(),
        )
    }

    /// Convert the configuration object to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python) -> Py<PyAny> {
        pythonize(py, self).unwrap().unbind()
    }
}

/// Data ingestion configuration.
#[pyclass(get_all, set_all, from_py_object)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestionConfig {
    /// File-system path to the primary database file.
    pub storage_path: PathBuf,

    /// Which data provider to use for each asset class.
    pub providers: ProviderConfig,
}

#[pymethods]
impl IngestionConfig {
    fn __repr__(&self) -> String {
        format!(
            "IngestionConfig(storage_path={:?}, providers={})",
            self.storage_path,
            self.providers.__repr__(),
        )
    }
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from(format!("{DEFAULT_STORAGE_PATH}database.duckdb")),
            providers: ProviderConfig::default(),
        }
    }
}

/// Which data provider to use per asset type.
#[pyclass(get_all, set_all, from_py_object)]
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider used for individual stock tickers.
    pub stocks: Provider,

    /// Provider used for exchange-traded funds.
    pub etf: Provider,

    /// Provider used for spot foreign-exchange pairs.
    pub forex: Provider,

    /// Provider used for cryptocurrency spot pairs.
    pub crypto: Provider,
}

#[pymethods]
impl ProviderConfig {
    fn __repr__(&self) -> String {
        format!(
            "ProviderConfig(stocks={:?}, etf={:?}, forex={:?}, crypto={:?})",
            self.stocks, self.etf, self.forex, self.crypto,
        )
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            stocks: Provider::Yahoo,
            etf: Provider::Yahoo,
            forex: Provider::Yahoo,
            crypto: Provider::Binance,
        }
    }
}

/// UI and formatting preferences.
///
/// These settings affect how values are displayed in the frontend.
/// They have no effect on computation.
#[pyclass(get_all, set_all, from_py_object)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// `strftime`-compatible date format string.
    pub date_format: String,

    /// IANA timezone name. `None` to use the system's local timezone.
    pub timezone: Option<String>,
}

#[pymethods]
impl DisplayConfig {
    fn __repr__(&self) -> String {
        format!("DisplayConfig(date_format={:?}, timezone={:?})", self.date_format, self.timezone,)
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            date_format: "%d-%m-%Y".to_owned(),
            timezone: None,
        }
    }
}

/// Search CWD or its parent for a recognized config file.
fn find_config_file() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let candidates = [cwd.as_path(), cwd.parent()?];

    for dir in candidates {
        for ext in ["toml", "yaml", "yml", "json"] {
            let path = dir.join(format!("{DEFAULT_CONFIG_FILE_NAME}.{ext}"));
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

/// Deserialize a config file, dispatching on its extension.
fn parse_config(path: &Path) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path)?;
    match path.extension().and_then(|e| e.to_str()) {
        Some("toml") => Ok(toml::from_str(&text)?),
        Some("yaml" | "yml") => Ok(serde_yml::from_str(&text)?),
        Some("json") => Ok(serde_json::from_str(&text)?),
        Some(ext) => Err(ConfigError::UnsupportedFormat(ext.to_owned())),
        None => Err(ConfigError::UnsupportedFormat(String::new())),
    }
}

/// Load the config without updating the singleton.
fn fetch_config() -> Config {
    find_config_file()
        .map_or_else(Config::default, |path| parse_config(&path).expect("failed to parse config"))
}

/// Register all config types and free functions into `backtide.core.config`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.config")?;

    m.add_class::<Config>()?;
    m.add_class::<IngestionConfig>()?;
    m.add_class::<DisplayConfig>()?;

    m.add_function(wrap_pyfunction!(get_config, &m)?)?;
    m.add_function(wrap_pyfunction!(load_config, &m)?)?;
    m.add_function(wrap_pyfunction!(set_config, &m)?)?;

    parent.add_submodule(&m)?;

    // Required for `from backtide.core.config import ...` to resolve
    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.config", &m)?;

    Ok(())
}

/// Get a copy of the current global configuration.
///
/// Use this function to alter the configuration programmatically before
/// updating the current config with [set_config].
///
/// Returns
/// -------
/// [Config]
///     The current configuration.
///
/// See Also
/// --------
/// - backtide.config:load_config
/// - backtide.config:set_config
///
/// Examples
/// --------
/// ```pycon
/// from pprint import pprint
/// from backtide.config import get_config
///
/// # Load and display the current configuration
/// cfg = get_config()
/// pprint(cfg.to_dict())
/// ```
#[pyfunction]
fn get_config() -> Config {
    // Clone only at the Python boundary
    CONFIG.get().cloned().unwrap_or_else(fetch_config)
}

/// Load a backtide configuration from a file.
///
/// Use this function to update a configuration programmatically before updating
/// the current config with [set_config]. The accepted file formats are: `toml`,
/// `yaml`, `yml`, `json`.
///
/// Parameters
/// ----------
/// path: str
///     Location of the config file to load.
///
/// Returns
/// -------
/// [Config]
///     The loaded configuration.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:set_config
///
/// Examples
/// --------
/// ```pycon
/// from backtide.config import load_config, set_config
///
/// # Use the configuration from a custom file location
/// set_config(load_config("path/to/config.toml"))
/// ```
#[pyfunction]
fn load_config(path: &str) -> PyResult<Config> {
    let cfg = parse_config(path.as_ref());
    cfg.map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Set the global configuration.
///
/// The configuration can only be set before it's used anywhere, so call this
/// function at thw start of the process. If the configuration is already used
/// by any backtide functionality, an exception is raised.
///
/// Parameters
/// ----------
/// config: [Config]
///     Configuration to set.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
///
/// Examples
/// --------
/// ```pycon
/// from backtide.config import get_config, set_config
///
/// # Load the current configuration and change a value
/// cfg = get_config()
/// cfg.base_currency = "USD"
///
/// # Update backtide's configuration
/// set_config(cfg)
///
/// # Check that the base_currency is indeed "USD" now
/// cfg = get_config()
/// print(cfg.base_currency)
/// ```
#[pyfunction]
fn set_config(config: Config) -> PyResult<()> {
    CONFIG
        .set(config)
        .map_err(|_| ConfigError::AlreadySet)
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn write_temp(content: &str, ext: &str) -> NamedTempFile {
        let mut f = tempfile::Builder::new().suffix(&format!(".{ext}")).tempfile().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    fn usd_toml() -> &'static str {
        r#"
        base_currency = "usd"

        [ingestion]
        storage_path = "/tmp/test.duckdb"

        [ingestion.providers]
        stocks = "Yahoo"
        etf    = "Yahoo"
        forex  = "Yahoo"
        crypto = "Binance"

        [display]
        date_format = "%Y-%m-%d"
        timezone    = "America/New_York"
        "#
    }

    fn usd_json() -> &'static str {
        r#"{
            "base_currency": "USD",
            "ingestion": {
                "storage_path": "/tmp/test.duckdb",
                "providers": {
                    "stocks": "Yahoo",
                    "etf":    "Yahoo",
                    "forex":  "Yahoo",
                    "crypto": "Binance"
                }
            },
            "display": {
                "date_format": "%Y-%m-%d",
                "timezone":    "America/New_York"
            }
        }"#
    }

    fn usd_yaml() -> &'static str {
        r#"
        base_currency: USD
        ingestion:
          storage_path: /tmp/test.duckdb
          providers:
            stocks: yahoo
            etf:    yahoo
            forex:  yahoo
            crypto: binance
        display:
          date_format: "%Y-%m-%d"
          timezone: America/New_York
        "#
    }

    // ── parse_config ─────────────────────────────────────────────────────────

    #[test]
    fn parse_toml() {
        let f = write_temp(usd_toml(), "toml");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.base_currency, Currency::USD);
        assert_eq!(cfg.display.timezone, Some("America/New_York".to_owned()));
        assert_eq!(cfg.display.date_format, "%Y-%m-%d");
        assert_eq!(cfg.ingestion.storage_path, PathBuf::from("/tmp/test.duckdb"));
    }

    #[test]
    fn parse_json() {
        let f = write_temp(usd_json(), "json");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.base_currency, Currency::USD);
    }

    #[test]
    fn parse_yaml() {
        let f = write_temp(usd_yaml(), "yaml");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.base_currency, Currency::USD);
    }

    #[test]
    fn parse_yml_extension() {
        let f = write_temp(usd_yaml(), "yml");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.base_currency, Currency::USD);
    }

    #[test]
    fn parse_unsupported_extension() {
        let f = write_temp("", "xml");
        let err = parse_config(f.path()).unwrap_err();
        assert!(matches!(err, ConfigError::UnsupportedFormat(ext) if ext == "xml"));
    }

    #[test]
    fn parse_no_extension() {
        // NamedTempFile with no suffix has no extension.
        let mut f = tempfile::Builder::new().tempfile().unwrap();
        f.write_all(b"").unwrap();
        let err = parse_config(f.path()).unwrap_err();
        assert!(matches!(err, ConfigError::UnsupportedFormat(ext) if ext.is_empty()));
    }

    #[test]
    fn parse_missing_file() {
        let err = parse_config(Path::new("/nonexistent/backtide.config.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::Io(_)));
    }

    #[test]
    fn parse_malformed_toml() {
        let f = write_temp("base_currency = [[[", "toml");
        assert!(matches!(parse_config(f.path()).unwrap_err(), ConfigError::Toml(_)));
    }

    #[test]
    fn parse_malformed_json() {
        let f = write_temp("{bad json", "json");
        assert!(matches!(parse_config(f.path()).unwrap_err(), ConfigError::Json(_)));
    }
}
