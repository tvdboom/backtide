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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_symbol_with_base() {
        let result = canonical_symbol("BTCUSD", &Some("BTC".to_owned()), &"USD".to_owned());
        assert_eq!(result, "BTC-USD");
    }

    #[test]
    fn test_canonical_symbol_without_base() {
        let result = canonical_symbol("AAPL", &None, &"USD".to_owned());
        assert_eq!(result, "AAPL");
    }
}
