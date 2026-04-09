use crate::config::errors::{ConfigError, ConfigResult};
use crate::config::interface::Config;
use crate::constants::CONFIG_FILE_NAME;
use crate::data::models::asset_type::AssetType;
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

/// Deserialize providers, filling in missing asset types with their defaults.
pub fn deserialize_providers<'de, D>(
    deserializer: D,
) -> Result<HashMap<AssetType, Provider>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    use strum::IntoEnumIterator;

    let explicit: HashMap<AssetType, Provider> = HashMap::deserialize(deserializer)?;
    let mut providers: HashMap<AssetType, Provider> =
        AssetType::iter().map(|at| (at, at.default())).collect();
    providers.extend(explicit);
    Ok(providers)
}
