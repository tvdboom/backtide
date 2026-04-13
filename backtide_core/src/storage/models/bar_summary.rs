//! Pre-aggregated summary of a bar group (one row per symbol × interval × provider).

/// Lightweight summary row returned by [`Storage::get_bars_summary`].
pub struct BarSummary {
    pub symbol: String,
    pub instrument_type: String,
    pub interval: String,
    pub provider: String,
    pub first_ts: u64,
    pub last_ts: u64,
    pub n_rows: u64,
    /// Last 365 `adj_close` values, ordered by `open_ts` ascending.
    pub sparkline: Vec<f64>,
}
