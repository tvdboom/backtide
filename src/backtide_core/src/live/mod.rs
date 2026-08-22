//! Live market data and deterministic simulated execution.

use crate::live::interface::{
    collect_market_updates, list_live_instruments, LiveMarketFeed, Session,
};
use crate::live::models::*;
use pyo3::prelude::*;

pub mod engine;
pub mod interface;
pub mod models;
pub mod providers;

/// Register the Python interface for `backtide.core.live`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.live")?;

    m.add_class::<MarketUpdate>()?;
    m.add_class::<LiveMarketFeed>()?;
    m.add_class::<SessionFill>()?;
    m.add_class::<SessionConfig>()?;
    m.add_class::<Session>()?;
    m.add_class::<SessionSnapshot>()?;
    m.add_class::<SessionUpdate>()?;

    m.add_function(wrap_pyfunction!(collect_market_updates, &m)?)?;
    m.add_function(wrap_pyfunction!(list_live_instruments, &m)?)?;

    parent.add_submodule(&m)?;
    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.live", &m)?;

    Ok(())
}
