//! Configuration module.

use crate::constants::{CONFIG_FILE_NAME, DEFAULT_STORAGE_PATH};
use crate::ingestion::provider::Provider;
use crate::models::asset::AssetType;
use crate::models::currency::Currency;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pythonize::pythonize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;
use strum::IntoEnumIterator;
use thiserror::Error;
// ────────────────────────────────────────────────────────────────────────────
// Configuration structs
// ────────────────────────────────────────────────────────────────────────────

/// Process-wide configuration singleton.
static CONFIG: OnceLock<Config> = OnceLock::new();

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

impl Config {
    /// Return a `&'static` reference to the global configuration.
    pub fn get() -> Result<&'static Config, ConfigError> {
        // Replace block with get_or_try_init when it becomes stable
        if let Some(cfg) = CONFIG.get() {
            Ok(cfg)
        } else {
            let _ = CONFIG.set(fetch_config()?);
            Ok(CONFIG.get().unwrap())
        }
    }
}

/// Data ingestion configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestionConfig {
    /// File-system path to the primary database file.
    pub storage_path: PathBuf,

    /// Which data provider to use for each asset type.
    pub providers: HashMap<AssetType, Provider>,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from(DEFAULT_STORAGE_PATH),
            providers: AssetType::iter().map(|at| (at, at.default())).collect(),
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

    /// API key for the logokit website.
    pub logokit_api_key: Option<String>,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            date_format: "YYYY-MM-DD".to_owned(),
            timezone: None,
            logokit_api_key: None,
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

impl From<ConfigError> for PyErr {
    fn from(e: ConfigError) -> PyErr {
        PyRuntimeError::new_err(e.to_string())
    }
}

/// Search CWD or its parent for a recognized config file.
fn find_config_file() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let candidates = [cwd.as_path(), cwd.parent()?];

    for dir in candidates {
        for ext in ["toml", "yaml", "yml", "json"] {
            let path = dir.join(format!("{CONFIG_FILE_NAME}.{ext}"));
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
fn fetch_config() -> Result<Config, ConfigError> {
    find_config_file().map(|path| parse_config(&path)).unwrap_or(Ok(Config::default()))
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
/// base_currency : str, default="USD"
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
            ingestion: Py::new(py, PyIngestionConfig::from_rust(cfg.ingestion))?,
            display: Py::new(py, PyDisplayConfig::from_rust(cfg.display))?,
        })
    }

    fn to_config(&self, py: Python<'_>) -> Config {
        Config {
            base_currency: self.base_currency.clone(),
            ingestion: self.ingestion.borrow(py).to_config(),
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
    #[pyo3(signature = (base_currency="USD", ingestion=None, display=None))]
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
            self.ingestion.borrow(py).__repr__(),
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
/// providers : dict[str, str] | None, default=None
///     Which data provider to use for each asset type. If `None`, it
///     defaults to `{"stocks": "yahoo", "etf": "yahoo", "forex": "yahoo",
///     "crypto": "binance"}`.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(
    name = "IngestionConfig",
    get_all,
    set_all,
    eq,
    from_py_object,
    module = "backtide.config"
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyIngestionConfig {
    pub storage_path: PathBuf,
    pub providers: HashMap<AssetType, Provider>,
}

impl PyIngestionConfig {
    fn from_rust(cfg: IngestionConfig) -> Self {
        Self {
            storage_path: cfg.storage_path,
            providers: cfg.providers,
        }
    }

    fn to_config(&self) -> IngestionConfig {
        IngestionConfig {
            storage_path: self.storage_path.clone(),
            providers: self.providers.clone(),
        }
    }
}

#[pymethods]
impl PyIngestionConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (storage_path=".backtide/", providers=None))]
    fn new(storage_path: &str, providers: Option<HashMap<String, String>>) -> PyResult<Self> {
        let providers = match providers {
            Some(map) => map
                .into_iter()
                .map(|(k, v)| {
                    let asset_type = AssetType::from_str(&k)
                        .map_err(|_| PyValueError::new_err(format!("Invalid asset type: {k}")))?;
                    let provider = Provider::from_str(&v)
                        .map_err(|_| PyValueError::new_err(format!("Invalid provider: {v}")))?;
                    Ok((asset_type, provider))
                })
                .collect::<PyResult<HashMap<_, _>>>()?,
            None => IngestionConfig::default().providers,
        };

        Ok(Self {
            storage_path: PathBuf::from(storage_path),
            providers,
        })
    }

    fn __repr__(&self) -> String {
        let providers = {
            let pairs: Vec<String> = AssetType::iter()
                .map(|k| {
                    let default = k.default();
                    format!("\"{k}\": \"{}\"", self.providers.get(&k).unwrap_or(&default))
                })
                .collect();
            format!("{{{}}}", pairs.join(", "))
        };

        format!("IngestionConfig(storage_path={:?}, providers={})", self.storage_path, providers,)
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

/// Configuration for display parameters.
///
/// The display parameters control how values are presented in the UI
/// application. Read more in the [user guide][configuration].
///
/// Attributes
/// ----------
/// date_format : str, default="YYYY-MM-DD"
///     Format in which to display dates. The format should be one of `YYYY/MM/DD`,
///     `DD/MM/YYYY`, or `MM/DD/YYYY` and can also use a period (.) or hyphen (-)
///     as separators.
///
/// timezone : str or None, default=None
///     IANA timezone name. `None` to use the system's local timezone.
///
/// logokit_api_key : str or None, default=None
///     API key for the [logokit] website, which is used to fetch images for assets.
///     If `None`, no images are loaded.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(name = "DisplayConfig", get_all, set_all, eq, from_py_object, module = "backtide.config")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyDisplayConfig {
    pub date_format: String,
    pub timezone: Option<String>,
    pub logokit_api_key: Option<String>,
}

impl PyDisplayConfig {
    fn from_rust(cfg: DisplayConfig) -> Self {
        Self {
            date_format: cfg.date_format,
            timezone: cfg.timezone,
            logokit_api_key: cfg.logokit_api_key,
        }
    }

    fn to_config(&self) -> DisplayConfig {
        DisplayConfig {
            date_format: self.date_format.clone(),
            timezone: self.timezone.clone(),
            logokit_api_key: self.logokit_api_key.clone(),
        }
    }
}

#[pymethods]
impl PyDisplayConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (date_format="YYYY-MM-DD", timezone=None, logokit_api_key=None))]
    fn new(date_format: &str, timezone: Option<&str>, logokit_api_key: Option<&str>) -> Self {
        Self {
            date_format: date_format.to_owned(),
            timezone: timezone.map(|s| s.to_owned()),
            logokit_api_key: logokit_api_key.map(|s| s.to_owned()),
        }
    }

    fn __repr__(&self) -> String {
        format!("DisplayConfig(date_format={:?}, timezone={:?}, logokit_api_key={:?})", self.date_format, self.timezone, self.logokit_api_key)
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
    let cfg = CONFIG.get().cloned().unwrap_or(fetch_config()?);
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
/// cfg.base_currency = "USD"
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
