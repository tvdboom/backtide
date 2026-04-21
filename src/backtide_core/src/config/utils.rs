use crate::config::errors::{ConfigError, ConfigResult};
use crate::config::interface::Config;
use crate::constants::CONFIG_FILE_NAME;
use crate::data::models::instrument_type::InstrumentType;
use crate::data::models::provider::Provider;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Search CWD or its parent for a recognized config file.
pub fn find_config_file() -> Option<PathBuf> {
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
pub fn parse_config(path: &Path) -> ConfigResult<Config> {
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
pub fn fetch_config() -> ConfigResult<Config> {
    find_config_file().map(|path| parse_config(&path)).unwrap_or(Ok(Config::default()))
}

/// Deserialize providers, filling in missing instrument types with their defaults.
pub fn deserialize_providers<'de, D>(
    deserializer: D,
) -> Result<HashMap<InstrumentType, Provider>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    use strum::IntoEnumIterator;

    let explicit: HashMap<InstrumentType, Provider> = HashMap::deserialize(deserializer)?;
    let mut providers: HashMap<InstrumentType, Provider> =
        InstrumentType::iter().map(|it| (it, it.default_provider())).collect();
    providers.extend(explicit);
    Ok(providers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_config_toml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("backtide.config.toml");
        fs::write(&path, "[general]\nbase_currency = \"EUR\"\n").unwrap();
        let cfg = parse_config(&path).unwrap();
        assert_eq!(cfg.general.base_currency.to_string(), "EUR");
    }

    #[test]
    fn test_parse_config_yaml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("backtide.config.yaml");
        fs::write(&path, "general:\n  base_currency: EUR\n").unwrap();
        let cfg = parse_config(&path).unwrap();
        assert_eq!(cfg.general.base_currency.to_string(), "EUR");
    }

    #[test]
    fn test_parse_config_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("backtide.config.json");
        fs::write(&path, r#"{"general":{"base_currency":"EUR"}}"#).unwrap();
        let cfg = parse_config(&path).unwrap();
        assert_eq!(cfg.general.base_currency.to_string(), "EUR");
    }

    #[test]
    fn test_parse_config_unsupported_format() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("backtide.config.xml");
        fs::write(&path, "<config/>").unwrap();
        let result = parse_config(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_config_no_extension() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config");
        fs::write(&path, "").unwrap();
        let result = parse_config(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_config_nonexistent_file() {
        let result = parse_config(Path::new("/nonexistent/path/config.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_fetch_config_defaults() {
        // When no config file exists in CWD, defaults are returned.
        // This is environment-dependent but should not panic.
        let result = fetch_config();
        assert!(result.is_ok());
    }

    #[test]
    fn test_deserialize_providers_fills_defaults() {
        let toml_str = r#"
        [data.providers]
        crypto = "kraken"
        "#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        // Crypto should be overridden
        assert_eq!(*cfg.data.providers.get(&InstrumentType::Crypto).unwrap(), Provider::Kraken);
        // Others should have defaults
        assert_eq!(*cfg.data.providers.get(&InstrumentType::Stocks).unwrap(), Provider::Yahoo);
    }

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.general.base_currency.to_string(), "USD");
        assert_eq!(cfg.data.storage_path, PathBuf::from(".backtide"));
        assert_eq!(cfg.display.port, 8501);
    }
}
