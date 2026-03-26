//! Configuration module.
//!
//! Owns a process-wide [`Config`] singleton initialized at startup.
//! After that point every caller gets a cheap `&'static` reference
//! through [config()].

use crate::constants::{DEFAULT_CONFIG_FILE_NAME, DEFAULT_STORAGE_PATH};
use crate::ingestion::provider::Provider;
use crate::models::currency::Currency;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pythonize::pythonize;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;
use thiserror::Error;

// ────────────────────────────────────────────────────────────────────────────
// Singleton logic
// ────────────────────────────────────────────────────────────────────────────

/// Process-wide configuration singleton.
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Return a `&'static` reference to the global configuration.
pub fn config() -> &'static Config {
    CONFIG.get_or_init(fetch_config)
}

// ────────────────────────────────────────────────────────────────────────────
// Configuration structs
// ────────────────────────────────────────────────────────────────────────────

/// Backtide configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// ISO 4217 code that all prices are normalized to, e.g. `"USD"`.
    pub base_currency: Currency,

    /// Settings that control how market data is fetched and stored.
    pub ingestion: IngestionConfig,

    /// Settings that control how values are presented in the frontend.
    pub display: DisplayConfig,
}

/// Data ingestion configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestionConfig {
    /// File-system path to the primary database file.
    pub storage_path: PathBuf,

    /// Which data provider to use for each asset type.
    pub providers: ProviderConfig,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Display dates in a `strftime`-compatible format string.
    pub date_format: String,

    /// IANA timezone name. `None` to use the system's local timezone.
    pub timezone: Option<String>,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            date_format: "%d-%m-%Y".to_owned(),
            timezone: None,
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Utilities
// ────────────────────────────────────────────────────────────────────────────

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

    /// [`set_config`] was called after the singleton was set.
    #[error("The configuration has already been used; set_config cannot be called anymore.")]
    AlreadySet,
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

// ────────────────────────────────────────────────────────────────────────────
// Python interface
// ────────────────────────────────────────────────────────────────────────────

/// Backtide configuration.
///
/// Read more in the [user guide][configuration].
///
/// Attributes
/// ----------
/// base_currency : str, default="EUR"
///     Currency (ISO 4217 code) that all prices are normalized to.
///
/// ingestion : [`IngestionConfig`]
///     Settings that control how market data is fetched and stored.
///
/// display : [`DisplayConfig`]
///     Settings that control how values are presented in the application's interface.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(name = "Config", get_all, set_all, from_py_object, module = "backtide.config")]
#[derive(Debug)]
pub struct PyConfig {
    pub base_currency: Currency,
    pub ingestion: Py<PyIngestionConfig>,
    pub display: Py<PyDisplayConfig>,
}

impl PyConfig {
    fn from_rust(py: Python<'_>, cfg: Config) -> PyResult<Self> {
        Ok(Self {
            base_currency: cfg.base_currency,
            ingestion: Py::new(py, PyIngestionConfig::from_rust(py, cfg.ingestion)?)?,
            display: Py::new(py, PyDisplayConfig::from_rust(cfg.display))?,
        })
    }

    fn to_config(&self, py: Python<'_>) -> Config {
        Config {
            base_currency: self.base_currency.clone(),
            ingestion: self.ingestion.borrow(py).to_config(py),
            display: self.display.borrow(py).to_config(),
        }
    }
}

impl Clone for PyConfig {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            base_currency: self.base_currency.clone(),
            ingestion: self.ingestion.clone_ref(py),
            display: self.display.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (base_currency="EUR", ingestion=None, display=None))]
    fn new(
        py: Python<'_>,
        base_currency: &str,
        ingestion: Option<Py<PyIngestionConfig>>,
        display: Option<Py<PyDisplayConfig>>,
    ) -> PyResult<Self> {
        let default = Self::from_rust(py, Config::default())?;

        Ok(Self {
            base_currency: Currency::from_str(base_currency).map_err(|_| {
                PyValueError::new_err(format!("Invalid base currency: {base_currency}"))
            })?,
            ingestion: ingestion.unwrap_or(default.ingestion),
            display: display.unwrap_or(default.display),
        })
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "Config(base_currency={:?}, ingestion={}, display={})",
            self.base_currency.to_string(),
            self.ingestion.borrow(py).__repr__(py),
            self.display.borrow(py).__repr__(),
        )
    }

    fn __richcmp__(&self, py: Python<'_>, other: PyRef<Self>, op: CompareOp) -> bool {
        match op {
            CompareOp::Eq => self.to_config(py) == other.to_config(py),
            CompareOp::Ne => self.to_config(py) != other.to_config(py),
            _ => false,
        }
    }

    /// Convert the configuration object to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> Py<PyAny> {
        pythonize(py, &self.to_config(py)).unwrap().unbind()
    }
}

/// Configuration for ingestion parameters.
///
/// The ingestion parameters control how and where market data is fetched and
/// stored. Read more in the [user guide][configuration].
///
/// Attributes
/// ----------
/// storage_path : str, default=".backtide/"
///     File-system path to the primary database file.
///
/// providers : [`ProviderConfig`]
///     Which data provider to use for each asset type.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(name = "IngestionConfig", get_all, set_all, from_py_object, module = "backtide.config")]
#[derive(Debug)]
pub struct PyIngestionConfig {
    pub storage_path: PathBuf,
    pub providers: Py<PyProviderConfig>,
}

impl PyIngestionConfig {
    fn from_rust(py: Python<'_>, cfg: IngestionConfig) -> PyResult<Self> {
        Ok(Self {
            storage_path: cfg.storage_path,
            providers: Py::new(py, PyProviderConfig::from_rust(cfg.providers))?,
        })
    }

    fn to_config(&self, py: Python<'_>) -> IngestionConfig {
        IngestionConfig {
            storage_path: self.storage_path.clone(),
            providers: self.providers.borrow(py).to_config(py),
        }
    }
}

impl Clone for PyIngestionConfig {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            storage_path: self.storage_path.clone(),
            providers: self.providers.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyIngestionConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (storage_path=".backtide/", providers=None))]
    fn new(py: Python<'_>, storage_path: &str, providers: Option<Py<PyProviderConfig>>) -> Self {
        Self {
            storage_path: PathBuf::from(format!("{storage_path}database.duckdb")),
            providers: providers.unwrap_or_else(|| {
                let default = Self::from_rust(py, IngestionConfig::default()).unwrap();
                default.providers
            }),
        }
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "IngestionConfig(storage_path={:?}, providers={})",
            self.storage_path,
            self.providers.borrow(py).__repr__(),
        )
    }

    fn __richcmp__(&self, py: Python<'_>, other: PyRef<Self>, op: CompareOp) -> bool {
        match op {
            CompareOp::Eq => self.to_config(py) == other.to_config(py),
            CompareOp::Ne => self.to_config(py) != other.to_config(py),
            _ => false,
        }
    }

    /// Convert the configuration object to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> Py<PyAny> {
        pythonize(py, &self.to_config(py)).unwrap().unbind()
    }
}

/// Configuration for provider parameters.
///
/// The provider parameters determine which data provider to use for each asset
/// type. Read more in the [user guide][configuration].
///
/// Attributes
/// ----------
/// stocks : str, default="yahoo"
///     Provider used for individual stock tickers.
///
/// etf : str, default="yahoo"
///     Provider used for exchange-traded funds.
///
/// forex : str, default="yahoo"
///     Provider used for spot foreign-exchange pairs.
///
/// crypto : str, default="binance"
///     Provider used for cryptocurrency spot pairs.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(name = "ProviderConfig", get_all, set_all, from_py_object, module = "backtide.config")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyProviderConfig {
    pub stocks: Provider,
    pub etf: Provider,
    pub forex: Provider,
    pub crypto: Provider,
}

impl PyProviderConfig {
    fn from_rust(cfg: ProviderConfig) -> Self {
        Self {
            stocks: cfg.stocks,
            etf: cfg.etf,
            forex: cfg.forex,
            crypto: cfg.crypto,
        }
    }

    fn to_config(&self, _py: Python<'_>) -> ProviderConfig {
        ProviderConfig {
            stocks: self.stocks,
            etf: self.etf,
            forex: self.forex,
            crypto: self.crypto,
        }
    }
}

#[pymethods]
impl PyProviderConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (stocks="yahoo", etf="yahoo", forex="yahoo", crypto="binance"))]
    fn new(stocks: &str, etf: &str, forex: &str, crypto: &str) -> PyResult<Self> {
        Ok(Self {
            stocks: Provider::from_str(stocks)
                .map_err(|_| PyValueError::new_err(format!("Invalid provider: {stocks}")))?,
            etf: Provider::from_str(etf)
                .map_err(|_| PyValueError::new_err(format!("Invalid provider: {etf}")))?,
            forex: Provider::from_str(forex)
                .map_err(|_| PyValueError::new_err(format!("Invalid provider: {forex}")))?,
            crypto: Provider::from_str(crypto)
                .map_err(|_| PyValueError::new_err(format!("Invalid provider: {crypto}")))?,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "ProviderConfig(stocks={:?}, etf={:?}, forex={:?}, crypto={:?})",
            self.stocks.to_string(),
            self.etf.to_string(),
            self.forex.to_string(),
            self.crypto.to_string(),
        )
    }

    fn __richcmp__(&self, other: PyRef<Self>, op: CompareOp) -> bool {
        match op {
            CompareOp::Eq => self == &*other,
            CompareOp::Ne => self != &*other,
            _ => false,
        }
    }

    /// Convert the configuration object to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> Py<PyAny> {
        pythonize(py, &self.to_config(py)).unwrap().unbind()
    }
}

/// Configuration for display parameters.
///
/// The display parameters control how values are presented in the UI
/// application. Read more in the [user guide][configuration].
///
/// Attributes
/// ----------
/// date_format : str, default="%d-%m-%Y"
///     Display dates in a `strftime`-compatible format string.
///
/// timezone : str or None, default=None
///     IANA timezone name. `None` to use the system's local timezone.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(name = "DisplayConfig", get_all, set_all, from_py_object, module = "backtide.config")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyDisplayConfig {
    pub date_format: String,
    pub timezone: Option<String>,
}

impl PyDisplayConfig {
    fn from_rust(cfg: DisplayConfig) -> Self {
        Self {
            date_format: cfg.date_format,
            timezone: cfg.timezone,
        }
    }

    fn to_config(&self) -> DisplayConfig {
        DisplayConfig {
            date_format: self.date_format.clone(),
            timezone: self.timezone.clone(),
        }
    }
}

#[pymethods]
impl PyDisplayConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (date_format="%d-%m-%Y", timezone=None))]
    fn new(date_format: &str, timezone: Option<&str>) -> Self {
        Self {
            date_format: date_format.to_owned(),
            timezone: timezone.map(|s| s.to_owned()),
        }
    }

    fn __repr__(&self) -> String {
        format!("DisplayConfig(date_format={:?}, timezone={:?})", self.date_format, self.timezone,)
    }

    fn __richcmp__(&self, other: PyRef<Self>, op: CompareOp) -> bool {
        match op {
            CompareOp::Eq => self == &*other,
            CompareOp::Ne => self != &*other,
            _ => false,
        }
    }

    /// Convert the configuration object to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> Py<PyAny> {
        pythonize(py, &self.to_config()).unwrap().unbind()
    }
}

/// Get a copy of the current global configuration.
///
/// Use this function to alter the configuration programmatically before
/// updating the current config with [`set_config`]. Read more in the
/// [user guide][configuration].
///
/// Returns
/// -------
/// [`Config`]
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
fn get_config(py: Python<'_>) -> PyResult<PyConfig> {
    let cfg = CONFIG.get().cloned().unwrap_or_else(fetch_config);
    PyConfig::from_rust(py, cfg)
}

/// Load a backtide configuration from a file.
///
/// Use this function to update a configuration programmatically before updating
/// the current config with [`set_config`]. The accepted file formats are: `toml`,
/// `yaml`, `yml`, `json`. Read more in the [user guide][configuration].
///
/// Parameters
/// ----------
/// path: str
///     Location of the config file to load.
///
/// Returns
/// -------
/// [`Config`]
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
/// set_config(load_config("path/to/config.toml")) # norun
/// ```
#[pyfunction]
fn load_config(py: Python<'_>, path: &str) -> PyResult<PyConfig> {
    let cfg = parse_config(path.as_ref()).map_err(|e| PyValueError::new_err(e.to_string()))?;
    PyConfig::from_rust(py, cfg)
}

/// Set the global configuration.
///
/// The configuration can only be set before it's used anywhere, so call this
/// function at thw start of the process. If the configuration is already used
/// by any backtide functionality, an exception is raised. Read more in the
/// [user guide][configuration].
///
/// Parameters
/// ----------
/// config: [`Config`]
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
/// cfg.base_currency = "EUR"
///
/// # Update backtide's configuration
/// set_config(cfg)  # norun
///
/// cfg = get_config()
/// print(cfg.base_currency)
/// ```
#[pyfunction]
fn set_config(py: Python<'_>, config: PyConfig) -> PyResult<()> {
    CONFIG
        .set(config.to_config(py))
        .map_err(|_| ConfigError::AlreadySet)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

/// Register all config types and free functions into `backtide.core.config`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.config")?;

    m.add_class::<PyConfig>()?;
    m.add_class::<PyIngestionConfig>()?;
    m.add_class::<PyProviderConfig>()?;
    m.add_class::<PyDisplayConfig>()?;

    m.add_function(wrap_pyfunction!(get_config, &m)?)?;
    m.add_function(wrap_pyfunction!(load_config, &m)?)?;
    m.add_function(wrap_pyfunction!(set_config, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.config", &m)?;

    Ok(())
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ────────────────────────────────────────────────────────────────────────────
    // Helpers
    // ────────────────────────────────────────────────────────────────────────────

    /// Write some content to a temporary file.
    fn write_temp(content: &str, ext: &str) -> NamedTempFile {
        let mut f = tempfile::Builder::new().suffix(&format!(".{ext}")).tempfile().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    /// Dummy configuration in TOML format.
    fn config_as_toml() -> &'static str {
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

    /// Dummy configuration in YAML format.
    fn config_as_yaml() -> &'static str {
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

    /// Dummy configuration in JSON format.
    fn config_as_json() -> &'static str {
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

    // ────────────────────────────────────────────────────────────────────────────
    // Parse config
    // ────────────────────────────────────────────────────────────────────────────

    #[test]
    fn parse_toml() {
        let f = write_temp(config_as_toml(), "toml");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.base_currency, Currency::USD);
        assert_eq!(cfg.display.timezone, Some("America/New_York".to_owned()));
        assert_eq!(cfg.display.date_format, "%Y-%m-%d");
        assert_eq!(cfg.ingestion.storage_path, PathBuf::from("/tmp/test.duckdb"));
    }

    #[test]
    fn parse_json() {
        let f = write_temp(config_as_json(), "json");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.base_currency, Currency::USD);
    }

    #[test]
    fn parse_yaml() {
        let f = write_temp(config_as_yaml(), "yaml");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.base_currency, Currency::USD);
    }

    #[test]
    fn parse_yml_extension() {
        let f = write_temp(config_as_yaml(), "yml");
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
