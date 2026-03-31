//! Constants shared across the package.

use std::time::Duration;

/// Name the configuration file must have.
pub const CONFIG_FILE_NAME: &str = "backtide.config";

/// Default location where backtide stores data on disk.
pub const DEFAULT_STORAGE_PATH: &str = ".backtide";

/// Duration for which loaded assets are cached.
pub const ASSET_CACHE_TTL: Duration = Duration::from_secs(60 * 60 * 2);
