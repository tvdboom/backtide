use crate::constants::Symbol;
use crate::data::models::Dividend;

/// A single dividend row as stored in the database, including its key columns.
pub struct StoredDividend {
    pub symbol: Symbol,
    pub provider: String,
    pub dividend: Dividend,
}
