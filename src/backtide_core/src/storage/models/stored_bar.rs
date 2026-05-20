use crate::constants::Symbol;
use crate::data::models::Bar;

/// A single bar row as stored in the database, including its key columns.
pub struct StoredBar {
    pub symbol: Symbol,
    pub interval: String,
    pub provider: String,
    pub bar: Bar,
}
