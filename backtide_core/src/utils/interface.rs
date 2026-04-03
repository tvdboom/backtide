//! Python interface for the engine's utilities.

use crate::config::LogLevel;
use crate::engine::Engine;
use pyo3::prelude::*;
use std::sync::OnceLock;
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

// ────────────────────────────────────────────────────────────────────────────
// Private API
// ────────────────────────────────────────────────────────────────────────────

/// Process-wide lock to set the tracing.
static TRACING: OnceLock<()> = OnceLock::new();

pub fn init_logging_with_level(level: LogLevel) {
    TRACING.get_or_init(|| {
        tracing_subscriber::registry()
            .with(EnvFilter::new(level.to_string().to_lowercase()))
            .with(
                fmt::layer()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .compact(),
            )
            .init();

        info!("Backtide logging level set to: {level}.");
    });
}

// ────────────────────────────────────────────────────────────────────────────
// Public interface
// ────────────────────────────────────────────────────────────────────────────

/// Clears/invalidates all cache stored by the engine.
///
/// See Also
/// --------
/// - backtide.utils:init_logging
#[pyfunction]
pub fn clear_cache() -> PyResult<()> {
    Ok(Engine::get()?.clear_cache())
}

/// Initialize the global logging subscriber.
///
/// The logging level can only be set before it's used anywhere, so call this
/// function at the start of the process. If logging was already initialized
/// this results in a no-op.
///
/// Parameters
/// ----------
/// log_level : str | [`LogLevel`]
///     Minimum tracing log level. Choose from: "error", "warn", "info",
///    "trace".
///
/// See Also
/// --------
/// - backtide.utils:clear_cache
#[pyfunction]
pub fn init_logging(log_level: Bound<'_, PyAny>) -> PyResult<()> {
    let level = log_level.extract::<LogLevel>()?;
    init_logging_with_level(level);
    Ok(())
}
