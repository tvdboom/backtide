//! Tracing initialization for the Backtide core library.

use pyo3::pyfunction;
use std::sync::OnceLock;
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

/// Process-wide lock to set the tracing.
static TRACING: OnceLock<()> = OnceLock::new();

/// Initialize the global tracing subscriber at most once.
///
/// Streamlit (and any other Python caller) can call this early in startup to
/// control the log level. If tracing was already initialized by a prior Rust
/// call the `level` argument is silently ignored.
#[pyfunction]
pub fn init_tracing(level: &str) {
    TRACING.get_or_init(|| {
        info!("Backtide logging level set to: {level}");
        tracing_subscriber::registry()
            .with(EnvFilter::new(level.to_lowercase()))
            .with(
                fmt::layer()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .compact(),
            )
            .init();
    });
}
