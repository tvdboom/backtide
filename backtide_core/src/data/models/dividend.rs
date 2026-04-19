use serde::{Deserialize, Serialize};

/// A single dividend event for one symbol.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dividend {
    /// Ex-dividend date as a Unix timestamp (seconds).
    pub ex_date: u64,

    /// Dividend amount per share in the instrument's quote currency.
    pub amount: f64,
}
