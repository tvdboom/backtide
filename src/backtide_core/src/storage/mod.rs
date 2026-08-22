use crate::storage::interface::*;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub mod duckdb;
mod engine;
pub mod errors;
mod interface;
pub mod models;
pub mod traits;

/// Register the Python interface for `backtide.core.storage`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.storage")?;

    m.add_function(wrap_pyfunction!(query_bars, &m)?)?;
    m.add_function(wrap_pyfunction!(query_bars_summary, &m)?)?;
    m.add_function(wrap_pyfunction!(query_dividends, &m)?)?;
    m.add_function(wrap_pyfunction!(query_experiments, &m)?)?;
    m.add_function(wrap_pyfunction!(query_strategy_runs, &m)?)?;
    m.add_function(wrap_pyfunction!(query_instruments, &m)?)?;
    m.add_function(wrap_pyfunction!(delete_symbols, &m)?)?;
    m.add_function(wrap_pyfunction!(delete_experiment, &m)?)?;
    m.add_function(wrap_pyfunction!(_write_live_session, &m)?)?;
    m.add_function(wrap_pyfunction!(_append_live_session_event, &m)?)?;
    m.add_function(wrap_pyfunction!(_write_live_session_warmup, &m)?)?;
    m.add_function(wrap_pyfunction!(_query_live_sessions, &m)?)?;
    m.add_function(wrap_pyfunction!(_query_live_session, &m)?)?;
    m.add_function(wrap_pyfunction!(_query_live_session_events, &m)?)?;
    m.add_function(wrap_pyfunction!(_query_live_session_warmup, &m)?)?;
    m.add_function(wrap_pyfunction!(_delete_live_session, &m)?)?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.storage", &m)?;

    Ok(())
}
