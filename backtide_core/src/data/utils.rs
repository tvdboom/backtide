//! Utility functions for the data module.

use crate::data::models::asset::Symbol;

/// Create the canonical (provider independent) symbol.
pub fn canonical_symbol(symbol: &String, base: &Option<String>, quote: &String) -> Symbol {
    if let Some(base) = base {
        format!("{base}-{quote}")
    } else {
        symbol.clone()
    }
}
