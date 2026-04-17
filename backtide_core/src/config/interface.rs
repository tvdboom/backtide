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

use crate::config::errors::{ConfigError, ConfigResult};
use crate::config::models::dataframe_backend::DataframeBackend;
use crate::config::models::log_level::LogLevel;
use crate::config::models::triangulation_strategy::TriangulationStrategy;
use crate::config::utils::{fetch_config, parse_config};
use crate::constants::DEFAULT_STORAGE_PATH;
use crate::data::models::currency::Currency;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::provider::Provider;
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pythonize::pythonize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use strum::IntoEnumIterator;

// ────────────────────────────────────────────────────────────────────────────
// Singleton logic
// ────────────────────────────────────────────────────────────────────────────

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

// ────────────────────────────────────────────────────────────────────────────
// Python API
// ────────────────────────────────────────────────────────────────────────────

/// Backtide configuration.
///
/// Read more in the [user guide][configuration].
///
/// Attributes
/// ----------
/// general : [GeneralConfig] | None, default=None
///     Portfolio-wide settings. If `None`, uses `GeneralConfig` defaults.
///
/// data : [DataConfig] | None, default=None
///     Settings that control how market data is fetched and stored. If
///     `None`, uses `DataConfig` defaults.
///
/// display : [DisplayConfig] | None, default=None
///     Settings that control how values are presented in the application's
///     frontend. If `None`, uses `DisplayConfig` defaults.
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
    #[pyo3(signature = (general: "GeneralConfig | None"=None, data: "DataConfig | None"=None, display: "DisplayConfig | None"=None))]
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
/// base_currency : str | [Currency], default="USD"
///     ISO 4217 currency code that all prices are normalized to.
///
/// triangulation_strategy : str | [TriangulationStrategy], default="direct"
///     With which approach to convert currencies to `base_currency`. Read more
///     in the [user guide][currency-conversion].
///
/// triangulation_fiat : str | [Currency], default="USD"
///     The fiat currency used as an intermediate between a fiat currency and
///     `base_currency`. This method is chosen when no direct conversion path exists
///     or when this method has longer history and `triangulation_strategy="earliest"`
///     For example, if converting `PLN -> THB` and no `PLN-THB` pair is available, the
///     engine will route through this currency as `PLN` -> `triangulation_fiat` -> `THB`.
///     The chosen currency is expected to have pairs with all the currencies the
///     project works with.
///
/// triangulation_crypto : str, default="USDT"
///     The cryptocurrency used as an intermediate when no direct conversion
///     path exists between a crypto and `base_currency`. For example, to calculate
///     the value of `BTC`, the engine will route `BTC` -> `triangulation_crypto` ->
///     `triangulation_crypto_pegged` -> `base_currency`. The selected crypto is
///     expected to be a stablecoin pegged to the `triangulation_crypto_pegged`
///     fiat currency.
///
/// triangulation_crypto_pegged : str, default="USD"
///     The fiat currency to which `triangulation_crypto` is pegged, for the
///     purposes of bridging between the crypto and fiat conversion graphs. When
///     a conversion path crosses the crypto/fiat boundary,
///     the engine treats `triangulation_crypto`/`triangulation_crypto_pegged`
///     as the crossing pair at parity 1:1.
///
/// log_level : str, default="warn"
///     Minimum tracing log level. Choose from: "error", "warn", "info", "debug".
///
/// See Also
/// --------
/// - backtide.config:get_config
/// - backtide.config:load_config
/// - backtide.config:set_config
#[pyclass(get_all, set_all, eq, from_py_object, module = "backtide.config")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub base_currency: Currency,
    pub triangulation_strategy: TriangulationStrategy,
    pub triangulation_fiat: Currency,
    pub triangulation_crypto: String,
    pub triangulation_crypto_pegged: Currency,
    pub log_level: LogLevel,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            base_currency: Currency::default(),
            triangulation_strategy: TriangulationStrategy::default(),
            triangulation_fiat: Currency::default(),
            triangulation_crypto: "USDT".to_owned(),
            triangulation_crypto_pegged: Currency::default(),
            log_level: LogLevel::default(),
        }
    }
}

#[pymethods]
impl GeneralConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (
        base_currency: "str | Currency" = Currency::default(),
        triangulation_strategy: "str | TriangulationStrategy" = TriangulationStrategy::default(),
        triangulation_fiat: "str | Currency" = Currency::default(),
        triangulation_crypto: "str" = "USDT",
        triangulation_crypto_pegged: "str | Currency" = Currency::default(),
        log_level: "str | LogLevel" = LogLevel::default()
    ))]
    fn new(
        base_currency: Currency,
        triangulation_strategy: TriangulationStrategy,
        triangulation_fiat: Currency,
        triangulation_crypto: &str,
        triangulation_crypto_pegged: Currency,
        log_level: LogLevel,
    ) -> Self {
        Self {
            base_currency,
            triangulation_strategy,
            triangulation_fiat,
            triangulation_crypto: triangulation_crypto.to_owned(),
            triangulation_crypto_pegged,
            log_level,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "GeneralConfig(base_currency={:?}, triangulation_strategy={:?}, triangulation_fiat={:?}, triangulation_crypto={:?}, triangulation_crypto_pegged={:?}, log_level={:?})",
            self.base_currency.to_string(),
            self.triangulation_strategy.to_string(),
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
/// providers : dict[[InstrumentType], [Provider]]
///     Which data provider to use for each instrument type. When constructing,
///     it defaults to: `{"stocks": "yahoo", "etf": "yahoo", "forex": "yahoo",
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
    #[serde(deserialize_with = "crate::config::utils::deserialize_providers")]
    pub providers: HashMap<InstrumentType, Provider>,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from(DEFAULT_STORAGE_PATH),
            providers: InstrumentType::iter().map(|at| (at, at.default_provider())).collect(),
        }
    }
}

#[pymethods]
impl DataConfig {
    #[classattr]
    const __RUST_DATACLASS__: bool = true;

    #[new]
    #[pyo3(signature = (storage_path: "str"=".backtide", providers: "dict[str | InstrumentType, str | Provider] | None"=None))]
    fn new(storage_path: &str, providers: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
        let mut resolved = DataConfig::default().providers;
        if let Some(obj) = providers {
            let dict = obj.cast::<pyo3::types::PyDict>()?;
            for (k, v) in dict.iter() {
                let instrument_type = k.extract::<InstrumentType>()?;
                let provider = v.extract::<Provider>()?;
                resolved.insert(instrument_type, provider);
            }
        }

        Ok(Self {
            storage_path: PathBuf::from(storage_path),
            providers: resolved,
        })
    }

    fn __repr__(&self) -> String {
        let providers = {
            let pairs: Vec<String> = InstrumentType::iter()
                .map(|k| {
                    let default = k.default_provider();
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
/// dataframe_backend : str | [DataframeBackend], default="pandas"
///     Which dataframe library to use when providing data to the frontend (i.e.,
///     the return of storage functions or parameters in the strategy function).
///     Choose from: "pandas", "polars".
///
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
///     API key for the [logokit] website, which is used to fetch images for instruments.
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
    pub dataframe_backend: DataframeBackend,
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
            dataframe_backend: DataframeBackend::default(),
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
    #[pyo3(signature = (
        dataframe_backend: "str | DataframeBackend" = DataframeBackend::default(),
        date_format: "str"="YYYY-MM-DD",
        time_format: "str"="HH:MM",
        timezone: "str | None"=None,
        logokit_api_key: "str | None"=None,
        address: "str | None"=None,
        port: "int"=8501
    ))]
    fn new(
        dataframe_backend: DataframeBackend,
        date_format: &str,
        time_format: &str,
        timezone: Option<&str>,
        logokit_api_key: Option<&str>,
        address: Option<&str>,
        port: u16,
    ) -> Self {
        Self {
            dataframe_backend,
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
            "DisplayConfig(dataframe_backend={:?}, date_format={:?}, time_format={:?}, timezone={:?}, logokit_api_key={:?}, address={:?}, port={:?})",
            self.dataframe_backend.to_string().to_lowercase(),
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
pub fn get_config(py: Python<'_>) -> PyResult<PyConfig> {
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
/// set_config(load_config("path/to/config.toml")) # norun
/// ```
#[pyfunction]
pub fn load_config(py: Python<'_>, path: &str) -> PyResult<PyConfig> {
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
/// cfg.general.base_currency = "USD"
///
/// # Update backtide's configuration
/// set_config(cfg)  # norun
///
/// cfg = get_config()
/// print(cfg.general.base_currency)
/// ```
#[pyfunction]
pub fn set_config(py: Python<'_>, config: PyConfig) -> PyResult<()> {
    CONFIG
        .set(config.to_rust(py))
        .map_err(|_| ConfigError::AlreadySet)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}
