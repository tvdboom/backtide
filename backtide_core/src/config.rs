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

use crate::constants::{CONFIG_FILE_NAME, DEFAULT_STORAGE_PATH};
use crate::data::models::asset_type::AssetType;
use crate::data::models::currency::Currency;
use crate::data::providers::provider::Provider;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pythonize::pythonize;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;
use strum::{Display, EnumString, IntoEnumIterator};
use thiserror::Error;

// ────────────────────────────────────────────────────────────────────────────
// Configuration structs
// ────────────────────────────────────────────────────────────────────────────

/// Process-wide configuration singleton.
static CONFIG: OnceLock<Config> = OnceLock::new();

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

pub type ConfigResult<T> = Result<T, ConfigError>;

impl From<ConfigError> for PyErr {
    fn from(e: ConfigError) -> PyErr {
        PyRuntimeError::new_err(e.to_string())
    }
}

/// Tracing logging level.
#[pyclass(skip_from_py_object, module = "backtide.config")]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    Display,
    EnumString,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[strum(ascii_case_insensitive)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    #[default]
    Warn,
    Error,
}

#[pymethods]
impl LogLevel {
    #[classattr]
    const __RUST_ENUM__: bool = true;

    fn __repr__(&self) -> String {
        self.to_string().to_lowercase()
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for LogLevel {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, PyErr> {
        // First try a direct downcast
        if let Ok(bound) = obj.cast::<LogLevel>() {
            return Ok(bound.borrow().clone());
        }

        // Else parse from string
        let s: String = obj.extract()?;
        s.parse().map_err(|_| PyValueError::new_err(format!("Unknown log_level {s:?}.")))
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
fn parse_config(path: &Path) -> ConfigResult<Config> {
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
fn fetch_config() -> ConfigResult<Config> {
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
/// general : [`GeneralConfig`]
///     Portfolio-wide settings.
///
/// data : [`DataConfig`]
///     Settings that control how market data is fetched and stored.
///
/// display : [`DisplayConfig`]
///     Settings that control how values are presented in the application's frontend.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(name = "Config", get_all, set_all, from_py_object, module = "backtide.config")]
#[derive(Debug)]
pub struct PyConfig {
    pub general: Py<GeneralConfig>,
    pub data: Py<DataConfig>,
    pub display: Py<DisplayConfig>,
}

impl PyConfig {
    fn from_rust(py: Python<'_>, cfg: Config) -> PyResult<Self> {
        Ok(Self {
            general: Py::new(py, cfg.general)?,
            data: Py::new(py, cfg.data)?,
            display: Py::new(py, cfg.display)?,
        })
    }

    fn to_rust(&self, py: Python<'_>) -> Config {
        Config {
            general: self.general.borrow(py).clone(),
            data: self.data.borrow(py).clone(),
            display: self.display.borrow(py).clone(),
        }
    }
}

impl Clone for PyConfig {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            general: self.general.clone_ref(py),
            data: self.data.clone_ref(py),
            display: self.display.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (general=None, data=None, display=None))]
    fn new(
        py: Python<'_>,
        general: Option<Py<GeneralConfig>>,
        data: Option<Py<DataConfig>>,
        display: Option<Py<DisplayConfig>>,
    ) -> PyResult<Self> {
        let default = Self::from_rust(py, Config::default())?;

        Ok(Self {
            general: general.unwrap_or(default.general),
            data: data.unwrap_or(default.data),
            display: display.unwrap_or(default.display),
        })
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "Config(general={}, data={}, display={})",
            self.general.borrow(py).__repr__(),
            self.data.borrow(py).__repr__(),
            self.display.borrow(py).__repr__(),
        )
    }

    fn __richcmp__(&self, py: Python<'_>, other: PyRef<Self>, op: CompareOp) -> bool {
        match op {
            CompareOp::Eq => self.to_rust(py) == other.to_rust(py),
            CompareOp::Ne => self.to_rust(py) != other.to_rust(py),
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
        pythonize(py, &self.to_rust(py)).unwrap().unbind()
    }
}

/// Portfolio-wide settings.
///
/// Attributes
/// ----------
/// base_currency : str, default="USD"
///     ISO 4217 currency code that all prices are normalized to.
///
/// triangulation_fiat : str, default="USD"
///     The fiat currency used as an intermediate when no direct conversion
///     path exists between a fiat currency and `base_currency`. For example,
///     if converting `PLN → THB` and no `PLN-THB` pair is available, the engine
///     will route through this currency as `PLN` → `triangulation_fiat` → `THB`.
///     The chosen currency is expected to have pairs with all the currencies the
///     project works with.
///
/// triangulation_crypto : str, default="USDT"
///     The cryptocurrency used as an intermediate when no direct conversion
///     path exists between a crypto and `base_currency`. For example, to calculate
///     the value of `BTC`, the engine will route `BTC` → `triangulation_crypto` →
///     `triangulation_crypto_pegged` → `base_currency`. The selected crypto is
///     expected to be a stablecoin pegged to the `triangulation_crypto_pegged`
///     fiat currency.
///
/// triangulation_crypto_pegged : str, default="USD"
///     The fiat currency to which `triangulation_crypto` is pegged, for the
///     purposes of bridging between the crypto and fiat conversion graphs. When
///     a conversion path crosses the crypto/fiat boundary (e.g., `BTC → EUR`),
///     the engine treats `triangulation_crypto`/`triangulation_crypto_pegged`
///     as the crossing pair at parity 1:1.
///
/// log_level : str, defeault="warn"
///     Minimum tracing log level. Choose from: "error", "warn", "info",
///    "trace".
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.config")]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub base_currency: Currency,
    pub triangulation_fiat: Currency,
    pub triangulation_crypto: String,
    pub triangulation_crypto_pegged: Currency,
    pub log_level: LogLevel,
}

#[pymethods]
impl GeneralConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (base_currency="USD", triangulation_fiat="USD", triangulation_crypto="USDT", triangulation_crypto_pegged="USD", log_level="warn"))]
    fn new(
        base_currency: &str,
        triangulation_fiat: &str,
        triangulation_crypto: &str,
        triangulation_crypto_pegged: &str,
        log_level: &str,
    ) -> PyResult<Self> {
        Ok(Self {
            base_currency: Currency::from_str(base_currency).map_err(|_| {
                PyValueError::new_err(format!("Invalid base_currency: {base_currency}"))
            })?,
            triangulation_fiat: Currency::from_str(triangulation_fiat).map_err(|_| {
                PyValueError::new_err(format!("Invalid triangulation_fiat: {triangulation_fiat}"))
            })?,
            triangulation_crypto: triangulation_crypto.to_owned(),
            triangulation_crypto_pegged: Currency::from_str(triangulation_crypto_pegged).map_err(
                |_| {
                    PyValueError::new_err(format!(
                        "Invalid triangulation_crypto_pegged: {triangulation_crypto_pegged}"
                    ))
                },
            )?,
            log_level: LogLevel::from_str(log_level)
                .map_err(|_| PyValueError::new_err(format!("Invalid log_level: {log_level}")))?,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "GeneralConfig(base_currency={:?}, triangulation_fiat={:?}, triangulation_crypto={:?}, triangulation_crypto_pegged={:?}, log_level={:?})",
            self.base_currency.to_string(),
            self.triangulation_fiat.to_string(),
            self.triangulation_crypto,
            self.triangulation_crypto_pegged.to_string(),
            self.log_level.to_string().to_lowercase(),
        )
    }

    /// Convert the configuration object to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> Py<PyAny> {
        pythonize(py, &self).unwrap().unbind()
    }
}

/// Configuration for data parameters.
///
/// The data parameters control how and where market data is fetched and
/// stored. Read more in the [user guide][configuration].
///
/// Attributes
/// ----------
/// storage_path : str, default=".backtide"
///     File-system path to the location to store the database and cache.
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
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.config")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataConfig {
    pub storage_path: PathBuf,
    pub providers: HashMap<AssetType, Provider>,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from(DEFAULT_STORAGE_PATH),
            providers: AssetType::iter().map(|at| (at, at.default())).collect(),
        }
    }
}

#[pymethods]
impl DataConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (storage_path=".backtide", providers=None))]
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
            None => DataConfig::default().providers,
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

        format!("DataConfig(storage_path={:?}, providers={})", self.storage_path, providers,)
    }

    /// Convert the configuration object to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> Py<PyAny> {
        pythonize(py, &self).unwrap().unbind()
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
///     Format in which to display dates in [momentjs] style. Valid formats include
///     `YYYY/MM/DD`, `DD/MM/YYYY`, or `MM/DD/YYYY` and can also use a period (.) or
///     hyphen (-) as separators.
///
/// time_format : str, default="HH:MM"
///     Format in which to display timestamps in [momentjs] style. Valid formats
///     include `HH:MM:SS` (include seconds), `hh:mm a` (show am/pm).
///
/// timezone : str or None, default=None
///     IANA timezone name. `None` to use the system's local timezone.
///
/// logokit_api_key : str or None, default=None
///     API key for the [logokit] website, which is used to fetch images for assets.
///     If `None`, no images are loaded.
///
/// address : str | None, default=None
///     The address where the streamlit server will listen for client and browser
///     connections. Use this if you want to bind the server to a specific
///     address. If set, the server will only be available from this address,
///     and not from any aliases (like localhost).
///
/// port : int, default=8501
///     TCP port the server listens on.
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.config")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub date_format: String,
    pub time_format: String,
    pub timezone: Option<String>,
    pub logokit_api_key: Option<String>,
    pub address: Option<String>,
    pub port: u16,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            date_format: "YYYY-MM-DD".to_owned(),
            time_format: "HH:MM".to_owned(),
            timezone: None,
            logokit_api_key: None,
            address: None,
            port: 8501,
        }
    }
}

#[pymethods]
impl DisplayConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (date_format="YYYY-MM-DD", time_format="HH:MM", timezone=None, logokit_api_key=None, address=None, port=8501))]
    fn new(
        date_format: &str,
        time_format: &str,
        timezone: Option<&str>,
        logokit_api_key: Option<&str>,
        address: Option<&str>,
        port: u16,
    ) -> Self {
        Self {
            date_format: date_format.to_owned(),
            time_format: time_format.to_owned(),
            timezone: timezone.map(|s| s.to_owned()),
            logokit_api_key: logokit_api_key.map(|s| s.to_owned()),
            address: address.map(|a| a.to_owned()),
            port,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "DisplayConfig(date_format={:?}, time_format={:?}, timezone={:?}, logokit_api_key={:?}, address={:?}, port={:?})",
            self.date_format,
            self.time_format,
            self.timezone,
            self.logokit_api_key.as_deref().map_or("None".to_owned(), |s| format!("{s:?}")),
            self.address,
            self.port,
        )
    }

    /// Return the configuration's format for a datetime timestamp.
    ///
    /// Returns
    /// -------
    /// str
    ///     Datetime format.
    pub fn datetime_format(&self) -> String {
        format!("{} {}", self.date_format, self.time_format)
    }

    /// Convert the configuration object to a dictionary.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Self as dict.
    pub fn to_dict(&self, py: Python<'_>) -> Py<PyAny> {
        pythonize(py, &self).unwrap().unbind()
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
    let cfg = parse_config(path.as_ref())?;
    PyConfig::from_rust(py, cfg)
}

/// Set the global configuration.
///
/// The configuration can only be set before it's used anywhere, so call this
/// function at the start of the process. If the configuration is already used
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
/// cfg.general.base_currency = "USD"
///
/// # Update backtide's configuration
/// set_config(cfg)  # norun
///
/// cfg = get_config()
/// print(cfg.general.base_currency)
/// ```
#[pyfunction]
fn set_config(py: Python<'_>, config: PyConfig) -> PyResult<()> {
    CONFIG
        .set(config.to_rust(py))
        .map_err(|_| ConfigError::AlreadySet)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

/// Register all config types and free functions into `backtide.core.config`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.config")?;

    m.add_class::<PyConfig>()?;
    m.add_class::<LogLevel>()?;
    m.add_class::<DataConfig>()?;
    m.add_class::<DisplayConfig>()?;
    m.add_class::<GeneralConfig>()?;

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

        [data]
        storage_path = "/tmp/test.duckdb"

        [data.providers]
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
        data:
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
            "data": {
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
        assert_eq!(cfg.general.base_currency, Currency::USD);
        assert_eq!(cfg.display.timezone, Some("America/New_York".to_owned()));
        assert_eq!(cfg.display.date_format, "%Y-%m-%d");
        assert_eq!(cfg.data.storage_path, PathBuf::from("/tmp/test.duckdb"));
    }

    #[test]
    fn parse_json() {
        let f = write_temp(config_as_json(), "json");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.general.base_currency, Currency::USD);
    }

    #[test]
    fn parse_yaml() {
        let f = write_temp(config_as_yaml(), "yaml");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.general.base_currency, Currency::USD);
    }

    #[test]
    fn parse_yml_extension() {
        let f = write_temp(config_as_yaml(), "yml");
        let cfg = parse_config(f.path()).unwrap();
        assert_eq!(cfg.general.base_currency, Currency::USD);
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
