use crate::data::models::bar::Bar;

/// Trait for all built-in indicators.
pub trait Indicator {
    /// Short ticker-style acronym (e.g. `"SMA"`).
    const ACRONYM: &'static str;

    /// Human-readable name (e.g. `"Simple Moving Average"`).
    const NAME: &'static str;

    /// One-sentence explanation of what the indicator measures.
    const DESCRIPTION: &'static str;

    /// Compute the indicator values from a slice of [`Bar`].
    fn compute_inner(&self, bars: &[Bar]) -> Vec<Vec<f64>>;
}
