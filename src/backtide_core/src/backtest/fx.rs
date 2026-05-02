//! Foreign-exchange rate table built from currency-conversion legs.
//!
//! `FxTable` indexes the close-price series of every conversion leg by
//! `(from_currency, to_currency)` so the backtest engine can convert
//! amounts between currencies at any historical timestamp using a
//! forward-fill lookup (the latest known rate at-or-before `ts`).
//!
//! The table supports:
//!   * direct lookups for any pair recorded as a leg,
//!   * inverse rates for free (1 / rate),
//!   * one-hop triangulation through the portfolio base currency.
//!
//! Triangulated legs are already resolved by `Engine::resolve_legs` (see
//! `data/engine.rs`), so a base = EUR experiment trading e.g. THB-quoted
//! crypto will receive both the pegged and the cross legs and either path
//! resolves to a usable rate.

use crate::data::models::currency::Currency;
use std::collections::HashMap;

/// In-memory FX table: `(from, to) -> sorted (timestamp, rate)`.
#[derive(Debug, Default, Clone)]
pub struct FxTable {
    pairs: HashMap<(Currency, Currency), Vec<(i64, f64)>>,
    base: Currency,
}

impl FxTable {
    pub fn new(base: Currency) -> Self {
        Self {
            pairs: HashMap::new(),
            base,
        }
    }

    /// Record a (timestamp, rate) series mapping `from -> to`.
    /// The series is sorted by timestamp on insertion.
    pub fn add_series(&mut self, from: Currency, to: Currency, mut series: Vec<(i64, f64)>) {
        series.sort_by_key(|x| x.0);
        series.retain(|(_, r)| r.is_finite() && *r > 0.0);
        if series.is_empty() {
            return;
        }
        self.pairs.entry((from, to)).or_default().extend(series);
        if let Some(v) = self.pairs.get_mut(&(from, to)) {
            v.sort_by_key(|x| x.0);
        }
    }

    /// Forward-fill lookup: latest rate at-or-before `ts` for `from -> to`.
    fn rate_direct(&self, from: Currency, to: Currency, ts: i64) -> Option<f64> {
        if from == to {
            return Some(1.0);
        }
        if let Some(s) = self.pairs.get(&(from, to)) {
            if let Some(r) = ff(s, ts) {
                return Some(r);
            }
        }
        if let Some(s) = self.pairs.get(&(to, from)) {
            if let Some(r) = ff(s, ts) {
                if r != 0.0 {
                    return Some(1.0 / r);
                }
            }
        }
        None
    }

    /// Convert `amount` from `from` to `to` at `ts`.
    ///
    /// Tries direct (or inverse) lookup, then triangulates through the
    /// configured base currency. Returns `None` if no path is available
    /// and no rate has been observed yet at `ts` (caller decides
    /// whether to fall back to 1.0, skip the order, or error out).
    pub fn convert(&self, amount: f64, from: Currency, to: Currency, ts: i64) -> Option<f64> {
        if amount == 0.0 || from == to {
            return Some(amount);
        }
        if let Some(r) = self.rate_direct(from, to, ts) {
            return Some(amount * r);
        }
        // Triangulate via base currency.
        let r1 = self.rate_direct(from, self.base, ts)?;
        let r2 = self.rate_direct(self.base, to, ts)?;
        Some(amount * r1 * r2)
    }

    /// Spot rate for 1 unit `from -> to` at `ts`. Returns `None` if no
    /// path can be resolved at `ts` (including forward-fill).
    pub fn rate(&self, from: Currency, to: Currency, ts: i64) -> Option<f64> {
        self.convert(1.0, from, to, ts)
    }

    /// Portfolio base currency this table was built for.
    pub fn base(&self) -> Currency {
        self.base
    }
}

/// Nearest-known lookup: latest value at-or-before `ts` in a sorted
/// series. If `ts` is earlier than every recorded sample the *first*
/// (earliest) sample is returned instead — this lets the engine value
/// portfolios on bars that pre-date the leg's first observation
/// without rejecting orders that would otherwise be funded just fine
/// at a later timestamp. Returns `None` only for empty series.
fn ff(s: &[(i64, f64)], ts: i64) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    match s.binary_search_by_key(&ts, |x| x.0) {
        Ok(i) => Some(s[i].1),
        Err(0) => Some(s[0].1),
        Err(i) => Some(s[i - 1].1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_and_inverse() {
        let mut fx = FxTable::new(Currency::EUR);
        // 1 EUR = 1.10 USD
        fx.add_series(Currency::EUR, Currency::USD, vec![(0, 1.10)]);
        assert_eq!(fx.rate(Currency::EUR, Currency::USD, 0), Some(1.10));
        assert!((fx.rate(Currency::USD, Currency::EUR, 0).unwrap() - 1.0 / 1.10).abs() < 1e-12);
    }

    #[test]
    fn forward_fill() {
        let mut fx = FxTable::new(Currency::EUR);
        fx.add_series(Currency::EUR, Currency::USD, vec![(10, 1.10), (20, 1.20)]);
        // Before first sample → backward-fill to the earliest known rate
        // so portfolios can still be valued/funded at pre-history bars.
        assert_eq!(fx.rate(Currency::EUR, Currency::USD, 5), Some(1.10));
        assert_eq!(fx.rate(Currency::EUR, Currency::USD, 15), Some(1.10));
        assert_eq!(fx.rate(Currency::EUR, Currency::USD, 20), Some(1.20));
        assert_eq!(fx.rate(Currency::EUR, Currency::USD, 999), Some(1.20));
    }

    #[test]
    fn triangulation_via_base() {
        let mut fx = FxTable::new(Currency::EUR);
        // EUR -> USD = 1.10, CNY -> USD = 0.14 (so CNY -> EUR = 0.14 / 1.10)
        fx.add_series(Currency::EUR, Currency::USD, vec![(0, 1.10)]);
        fx.add_series(Currency::CNY, Currency::USD, vec![(0, 0.14)]);
        let r = fx.rate(Currency::CNY, Currency::EUR, 0).unwrap();
        assert!((r - 0.14 / 1.10).abs() < 1e-12);
    }
}

// Currency code reference: EUR, USD, CNY are guaranteed by
// `data/models/currency.rs`. Any unit-test additions must use codes
// from that enum.
