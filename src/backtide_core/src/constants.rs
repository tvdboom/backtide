//! Constants and types shared across the package.

use crate::data::models::Currency;
use std::collections::HashMap;
use std::time::Duration;
// ────────────────────────────────────────────────────────────────────────────
// Types
// ────────────────────────────────────────────────────────────────────────────

/// Canonical (provider-independent) symbol name.
pub type Symbol = String;

/// A key in the `bars` table, i.e., `(symbol, interval, provider)`.
pub type BarKey = (Symbol, String, String);

/// Cash values in a portfolio.
pub type Cash = HashMap<Currency, f64>;

/// Symbol positions in a portfolio.
pub type Positions = HashMap<Symbol, f64>;

// ────────────────────────────────────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────────────────────────────────────

/// Name the configuration file must have.
pub const CONFIG_FILE_NAME: &str = "backtide.config";

/// Default location where backtide stores data on disk.
pub const DEFAULT_STORAGE_PATH: &str = ".backtide";

/// Maximum number of concurrent download / resolve tasks.
pub const MAX_CONCURRENT_REQUESTS: usize = 50;

/// Maximum wall-clock time for a single `download_bars` call before it is
/// canceled. Prevents a hung provider request from blocking the pipeline.
pub const TASK_TIMEOUT: Duration = Duration::from_secs(300);

/// Number of consecutive download failures before the circuit breaker trips
/// and all remaining tasks are skipped.
pub const CIRCUIT_BREAKER_THRESHOLD: usize = 20;

/// Name used for the benchmark run.
pub const BENCHMARK: &str = "Benchmark";

/// Seconds in a year.
pub const SECS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;
