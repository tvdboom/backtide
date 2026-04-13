//! Constants and types shared across the package.

use std::time::Duration;

// ────────────────────────────────────────────────────────────────────────────
// Types
// ────────────────────────────────────────────────────────────────────────────

/// Canonical (provider-independent) symbol name.
pub type Symbol = String;

// ────────────────────────────────────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────────────────────────────────────

/// Name the configuration file must have.
pub const CONFIG_FILE_NAME: &str = "backtide.config";

/// Default location where backtide stores data on disk.
pub const DEFAULT_STORAGE_PATH: &str = ".backtide";

/// Maximum number of concurrent HTTP requests for downloads / resolves.
pub const MAX_CONCURRENT_REQUESTS: usize = 50;

/// Tick interval for progress bar / spinner animations.
pub const TICK_INTERVAL: Duration = Duration::from_millis(100);

