mod backtest;
mod config;
mod constants;
mod data;
mod engine;
mod errors;
mod storage;
mod utils;

use pyo3::prelude::*;

// Required for Windows/MSVC builds when using DuckDB.
// DuckDB internally uses the Windows Restart Manager API (RmStartSession, etc),
// which lives in `rstrtmgr.lib`. The MSVC linker does not auto-link this system
// library, and build tools like maturin may ignore Cargo rustflags/config,
// leading to unresolved externals (LNK2019).
//
// This forces the linker to include `rstrtmgr.lib` in all build contexts
// (cargo, maturin, pip/PEP517) without relying on external configuration.
#[cfg(target_os = "windows")]
#[link(name = "rstrtmgr")]
extern "system" {}

/// Register the Python interface for `backtide.core`.
#[pymodule]
fn core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    backtest::register(m)?;
    config::register(m)?;
    data::register(m)?;
    storage::register(m)?;
    utils::register(m)?;
    Ok(())
}
