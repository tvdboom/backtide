//! Live market data and deterministic paper trading.

use crate::live::interface::{
    collect_market_updates, provider_live_support, LiveMarketFeed, PaperTradingSession,
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
    m.add_class::<PaperFill>()?;
    m.add_class::<PaperTradingConfig>()?;
    m.add_class::<PaperTradingSession>()?;
    m.add_class::<PaperTradingSnapshot>()?;
    m.add_class::<PaperTradingUpdate>()?;

    m.add_function(wrap_pyfunction!(collect_market_updates, &m)?)?;
    m.add_function(wrap_pyfunction!(provider_live_support, &m)?)?;

    parent.add_submodule(&m)?;
    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.live", &m)?;

    Ok(())
}
