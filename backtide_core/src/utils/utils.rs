//! Utility functions.

use pyo3::{pyfunction, PyResult};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

/// Initialize a tracing subscriber.
#[pyfunction]
pub fn init_tracing(log_level: Option<&str>) -> PyResult<()> {
    // Create env filter from RUST_LOG env variable (default to "warn")
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(log_level.unwrap_or("warn"))),
        )
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .compact(),
        )
        .init();

    Ok(())
}
