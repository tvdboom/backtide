use crate::data::models::bar::Bar;

/// A single bar row as stored in the database, including its key columns.
pub struct StoredBar {
    pub symbol: String,
    pub interval: String,
    pub provider: String,
    pub bar: Bar,
}
