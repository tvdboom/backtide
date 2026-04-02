//! Tracing initialization for the Backtide core library.
//!
//! Exposes [`ensure_tracing`] — a lazy, idempotent initializer that can be
//! called from any Rust entry point — and [`init_tracing`], a thin
//! [`#[pyfunction]`] wrapper for explicit control from Python (e.g. Streamlit
//! startup).

use crate::config::{Config, ConfigResult};
use pyo3::{pyfunction, PyResult};
use std::sync::OnceLock;
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

/// Process-wide lock to set the tracing.
static TRACING: OnceLock<()> = OnceLock::new();

/// Initialize the global tracing subscriber at most once.
pub fn ensure_tracing(level: Option<&str>) -> ConfigResult<()> {
    let cfg = Config::get()?;

    TRACING.get_or_init(|| {
        let level = level
            .map(str::to_owned)
            .unwrap_or_else(|| cfg.general.log_level.to_string().to_lowercase());

        info!("Backtide logging level set to: {level}");
        tracing_subscriber::registry()
            .with(EnvFilter::new(level))
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

    Ok(())
}

/// Explicitly initialize tracing from Python.
///
/// Streamlit (and any other Python caller) can call this early in startup to
/// control the log level. If tracing was already initialized by a prior Rust
/// call the `level` argument is silently ignored.
#[pyfunction]
pub fn init_tracing(level: Option<&str>) -> PyResult<()> {
    ensure_tracing(level)?;
    Ok(())
}
