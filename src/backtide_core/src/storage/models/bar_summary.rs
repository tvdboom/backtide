//! Pre-aggregated summary of a bar group (one row per symbol x interval x provider).

/// Lightweight summary row returned by [`Storage::query_bars_summary`].
pub struct BarSummary {
    pub symbol: String,
    pub instrument_type: String,
    pub interval: String,
    pub provider: String,
    pub name: Option<String>,
    pub base: Option<String>,
    pub quote: Option<String>,
    pub exchange: Option<String>,
    pub first_ts: u64,
    pub last_ts: u64,
    pub n_rows: u64,
    /// Last 365 `adj_close` values, ordered by `open_ts` ascending.
    pub sparkline: Vec<f64>,
}
