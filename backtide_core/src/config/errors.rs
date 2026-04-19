use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

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

