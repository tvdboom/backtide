use crate::data::models::dividend::Dividend;

/// A single dividend row as stored in the database, including its key columns.
pub struct StoredDividend {
    pub symbol: String,
    pub provider: String,
    pub dividend: Dividend,
}
