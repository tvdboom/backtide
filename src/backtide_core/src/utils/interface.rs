//! Python interface for the engine's utilities.

use crate::config::interface::Config;
use crate::config::models::log_level::LogLevel;
use crate::engine::Engine;
use crate::utils::experiment_log::ExperimentFileLayer;
use pyo3::prelude::*;
use std::sync::OnceLock;
use tracing::info;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};
// ────────────────────────────────────────────────────────────────────────────
// Private API
// ────────────────────────────────────────────────────────────────────────────

/// Process-wide lock to set the tracing.
static TRACING: OnceLock<()> = OnceLock::new();

/// `tracing_subscriber` time formatter that prints timestamps in the
/// timezone declared in `config.display.timezone`, or the system local
/// timezone when that field is unset / unparseable.
struct TzTime;

impl FormatTime for TzTime {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let now = chrono::Utc::now();
        let tz = Config::get()
            .ok()
            .and_then(|c| c.display.timezone.as_deref())
            .and_then(|s| s.trim().parse::<chrono_tz::Tz>().ok());
        match tz {
            Some(tz) => {
                write!(w, "{}", now.with_timezone(&tz).format("%Y-%m-%dT%H:%M:%S%.3f%:z"))
            },
            None => write!(
                w,
                "{}",
                now.with_timezone(&chrono::Local).format("%Y-%m-%dT%H:%M:%S%.3f%:z")
            ),
        }
    }
}

pub fn init_logging_with_level(level: LogLevel) {
    TRACING.get_or_init(|| {
        // The user-facing console layer honours the configured log level.
        let console_filter = EnvFilter::new(format!(
            "{},h2=warn,hyper=warn,hyper_util=warn,reqwest=warn,cookie_store=warn",
            level.to_string().to_lowercase()
        ));

        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_timer(TzTime)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .compact()
                    .with_filter(console_filter),
            )
            // Mirrors events emitted inside an "experiment" span to a
            // per-experiment `logs.txt` file. We attach a *separate*,
            // permissive filter (DEBUG+) so the experiment log is always
            // captured in full regardless of the user-facing log level —
            // otherwise a `log_level = "warn"` config would suppress the
            // INFO-level experiment span entirely and no file would ever
            // be opened.
            .with(ExperimentFileLayer.with_filter(LevelFilter::DEBUG))
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
    Engine::get()?.clear_cache();
    Ok(())
}

/// Initialize the global logging subscriber.
///
/// The logging level can only be set before it's used anywhere, so call this
/// function at the start of the process. If logging was already initialized
/// this results in a no-op.
///
/// Parameters
/// ----------
/// log_level : str | [LogLevel]
///     Minimum tracing log level. Choose from: "error", "warn", "info",
///    "debug".
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
