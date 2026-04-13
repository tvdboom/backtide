use crate::data::models::bar::Bar;
use crate::data::models::dividend::Dividend;

/// Bars and any associated corporate-action events.
///
/// Returns empty `bars` when the symbol had no trading activity that period.
pub struct BarDownload {
    pub bars: Vec<Bar>,
    pub dividends: Vec<Dividend>,
}
