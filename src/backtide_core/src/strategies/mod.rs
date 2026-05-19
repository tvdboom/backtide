use pyo3::prelude::*;
use pyo3::{Bound, PyResult};
use crate::strategies::interface::*;

pub mod interface;
mod utils;
mod traits;

/// Register the Python interface for `backtide.core.strategies`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.strategies")?;

    m.add_class::<AdaptiveRsi>()?;
    m.add_class::<AlphaRsiPro>()?;
    m.add_class::<BollingerMeanReversion>()?;
    m.add_class::<BuyAndHold>()?;
    m.add_class::<DoubleTop>()?;
    m.add_class::<HybridAlphaRsi>()?;
    m.add_class::<Macd>()?;
    m.add_class::<Momentum>()?;
    m.add_class::<MultiBollingerRotation>()?;
    m.add_class::<RiskAverse>()?;
    m.add_class::<Roc>()?;
    m.add_class::<RocRotation>()?;
    m.add_class::<Rsi>()?;
    m.add_class::<Rsrs>()?;
    m.add_class::<RsrsRotation>()?;
    m.add_class::<SmaCrossover>()?;
    m.add_class::<SmaNaive>()?;
    m.add_class::<TripleRsiRotation>()?;
    m.add_class::<TurtleTrading>()?;
    m.add_class::<Vcp>()?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.strategies", &m)?;

    Ok(())
}
