use crate::indicators::interface::*;
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub mod interface;
pub mod traits;
pub mod utils;

/// Register the Python interface for `backtide.core.indicators`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.indicators")?;

    m.add_function(wrap_pyfunction!(_indicator_deterministic_name, &m)?)?;

    m.add_class::<AverageDirectionalIndex>()?;
    m.add_class::<AverageTrueRange>()?;
    m.add_class::<BollingerBands>()?;
    m.add_class::<CommodityChannelIndex>()?;
    m.add_class::<ExponentialMovingAverage>()?;
    m.add_class::<MovingAverageConvergenceDivergence>()?;
    m.add_class::<OnBalanceVolume>()?;
    m.add_class::<RelativeStrengthIndex>()?;
    m.add_class::<SimpleMovingAverage>()?;
    m.add_class::<StochasticOscillator>()?;
    m.add_class::<VolumeWeightedAveragePrice>()?;
    m.add_class::<WeightedMovingAverage>()?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.indicators", &m)?;

    Ok(())
}
