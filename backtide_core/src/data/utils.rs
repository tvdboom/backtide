//! Utility functions for the data module.

use crate::constants::Symbol;

/// Create the canonical (provider independent) symbol.
pub fn canonical_symbol(symbol: &str, base: &Option<String>, quote: &String) -> Symbol {
    if let Some(base) = base {
        format!("{base}-{quote}")
    } else {
        symbol.to_owned()
    }
}
