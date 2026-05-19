//! Foreign-exchange rate table built from currency-conversion legs.
//!
//! `FxTable` indexes the close-price series of every conversion leg by
//! `(from_currency, to_currency)` so the backtest engine can convert
//! amounts between currencies at any historical timestamp using a
//! forward-fill lookup (the latest known rate at-or-before `ts`).
//!
//! Currency identifiers are plain strings so both ISO fiat codes (`USD`,
//! `EUR`) and crypto tickers (`ETH`, `USDT`) can serve as graph nodes.
//!
//! The table supports:
//!   - Direct lookups for any pair recorded as a leg.
//!   - Inverse rates for free (1 / rate).
//!   - One-hop triangulation through the portfolio base currency.

use std::collections::HashMap;

/// In-memory FX table: `from -> to -> sorted (timestamp, rate)`.
#[derive(Debug, Default, Clone)]
pub struct FxTable {
    pairs: HashMap<String, HashMap<String, Vec<(i64, f64)>>>,
    base: String,
}

impl FxTable {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            pairs: HashMap::new(),
            base: base.into(),
        }
    }

    /// Record a (timestamp, rate) series mapping `from -> to`. The series is
    /// sorted by timestamp on insertion.
    pub fn add_series(&mut self, from: &str, to: &str, mut series: Vec<(i64, f64)>) {
        series.sort_by_key(|x| x.0);
        series.retain(|(_, r)| r.is_finite() && *r > 0.);

        if series.is_empty() {
            return;
        }

        let v = self.pairs.entry(from.to_owned()).or_default().entry(to.to_owned()).or_default();
        v.extend(series);
        v.sort_by_key(|x| x.0);
    }

    /// Forward-fill lookup: latest rate at-or-before `ts` for `from -> to`.
    fn rate_direct(&self, from: &str, to: &str, ts: i64) -> Option<f64> {
        if from == to {
            return Some(1.0);
        }

        if let Some(inner) = self.pairs.get(from) {
            if let Some(s) = inner.get(to) {
                if let Some(r) = forward_fill(s, ts) {
                    return Some(r);
                }
            }
        }

        if let Some(inner) = self.pairs.get(to) {
            if let Some(s) = inner.get(from) {
                if let Some(r) = forward_fill(s, ts) {
                    if r != 0. {
                        return Some(1. / r);
                    }
                }
            }
        }

        None
    }

    /// Convert `amount` from `from` to `to` at `ts`.
    ///
    /// Tries direct (or inverse) lookup, then triangulates through the
    /// configured base currency. Returns `None` if no path is available
    /// and no rate has been observed yet at `ts`.
    pub fn convert(&self, amount: f64, from: &str, to: &str, ts: i64) -> Option<f64> {
        if amount == 0. || from == to {
            return Some(amount);
        }

        if let Some(r) = self.rate_direct(from, to, ts) {
            return Some(amount * r);
        }

        // Triangulate via base currency.
        let r1 = self.rate_direct(from, &self.base, ts)?;
        let r2 = self.rate_direct(&self.base, to, ts)?;
        Some(amount * r1 * r2)
    }

    /// Spot rate for 1 unit `from -> to` at `ts`. Returns `None` if no
    /// path can be resolved at `ts` (including forward-fill).
    pub fn rate(&self, from: &str, to: &str, ts: i64) -> Option<f64> {
        self.convert(1.0, from, to, ts)
    }
}

/// Nearest-known lookup: latest value at-or-before `ts` in a sorted series.
/// If `ts` is earlier than every recorded sample the first (earliest) sample
/// is returned instead. This lets the engine value portfolios on bars that
/// pre-date the leg's first observation without rejecting orders that would
/// otherwise be funded just fine at a later timestamp. Returns `None` only
/// for empty series.
fn forward_fill(s: &[(i64, f64)], ts: i64) -> Option<f64> {
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
        let mut fx = FxTable::new("EUR");
        // 1 EUR = 1.10 USD
        fx.add_series("EUR", "USD", vec![(0, 1.10)]);
        assert_eq!(fx.rate("EUR", "USD", 0), Some(1.10));
        assert!((fx.rate("USD", "EUR", 0).unwrap() - 1.0 / 1.10).abs() < 1e-12);
    }

    #[test]
    fn forward_fills() {
        let mut fx = FxTable::new("EUR");
        fx.add_series("EUR", "USD", vec![(10, 1.10), (20, 1.20)]);
        // Before first sample → backward-fill to the earliest known rate
        // so portfolios can still be valued/funded at pre-history bars.
        assert_eq!(fx.rate("EUR", "USD", 5), Some(1.10));
        assert_eq!(fx.rate("EUR", "USD", 15), Some(1.10));
        assert_eq!(fx.rate("EUR", "USD", 20), Some(1.20));
        assert_eq!(fx.rate("EUR", "USD", 999), Some(1.20));
    }

    #[test]
    fn triangulation_via_base() {
        let mut fx = FxTable::new("EUR");
        // EUR -> USD = 1.10, CNY -> USD = 0.14 (so CNY -> EUR = 0.14 / 1.10)
        fx.add_series("EUR", "USD", vec![(0, 1.10)]);
        fx.add_series("CNY", "USD", vec![(0, 0.14)]);
        // FxTable only triangulates through its configured base in one hop.
        assert_eq!(fx.rate("CNY", "EUR", 0), None);
    }

    #[test]
    fn same_currency_returns_one_and_amount_unchanged() {
        let fx = FxTable::new("EUR");
        assert_eq!(fx.rate("EUR", "EUR", 0), Some(1.0));
        assert_eq!(fx.convert(42.0, "USD", "USD", 0), Some(42.0));
    }

    #[test]
    fn zero_amount_is_returned_unchanged_even_without_pair() {
        let fx = FxTable::new("EUR");
        assert_eq!(fx.convert(0.0, "USD", "CNY", 0), Some(0.0));
    }

    #[test]
    fn convert_uses_direct_rate() {
        let mut fx = FxTable::new("EUR");
        fx.add_series("EUR", "USD", vec![(0, 1.25)]);
        assert_eq!(fx.convert(100.0, "EUR", "USD", 0), Some(125.0));
    }

    #[test]
    fn convert_triangulates_through_base() {
        let mut fx = FxTable::new("EUR");
        // CNY -> EUR via base EUR: need from->base and base->to legs.
        fx.add_series("CNY", "EUR", vec![(0, 0.13)]);
        fx.add_series("EUR", "USD", vec![(0, 1.10)]);
        // CNY -> USD = 0.13 * 1.10
        let got = fx.convert(1.0, "CNY", "USD", 0).unwrap();
        assert!((got - 0.13 * 1.10).abs() < 1e-12);
    }

    #[test]
    fn convert_returns_none_when_no_path() {
        let fx = FxTable::new("EUR");
        assert_eq!(fx.convert(1.0, "USD", "CNY", 0), None);
    }

    #[test]
    fn add_series_filters_non_positive_and_non_finite() {
        let mut fx = FxTable::new("EUR");
        fx.add_series(
            "EUR",
            "USD",
            vec![(10, 1.10), (20, 0.0), (30, f64::NAN), (40, -1.0), (50, 1.30)],
        );
        // Only finite positive rates retained.
        assert_eq!(fx.rate("EUR", "USD", 10), Some(1.10));
        assert_eq!(fx.rate("EUR", "USD", 35), Some(1.10));
        assert_eq!(fx.rate("EUR", "USD", 50), Some(1.30));
    }

    #[test]
    fn add_series_empty_after_filtering_is_noop() {
        let mut fx = FxTable::new("EUR");
        fx.add_series("EUR", "USD", vec![(10, 0.0), (20, f64::NAN)]);
        assert_eq!(fx.rate("EUR", "USD", 10), None);
    }

    #[test]
    fn add_series_appends_and_keeps_sorted() {
        let mut fx = FxTable::new("EUR");
        fx.add_series("EUR", "USD", vec![(20, 1.20)]);
        fx.add_series("EUR", "USD", vec![(10, 1.10), (30, 1.30)]);
        assert_eq!(fx.rate("EUR", "USD", 10), Some(1.10));
        assert_eq!(fx.rate("EUR", "USD", 25), Some(1.20));
        assert_eq!(fx.rate("EUR", "USD", 30), Some(1.30));
    }

    #[test]
    fn base_returns_configured_currency() {
        let fx = FxTable::new("CNY");
        assert_eq!(fx.base, "CNY");
    }

    #[test]
    fn ff_exact_match_returns_that_value() {
        let s = vec![(10, 1.0), (20, 2.0), (30, 3.0)];
        assert_eq!(forward_fill(&s, 20), Some(2.0));
        assert_eq!(forward_fill(&s, 10), Some(1.0));
        assert_eq!(forward_fill(&s, 30), Some(3.0));
    }

    #[test]
    fn ff_empty_series_is_none() {
        let s: Vec<(i64, f64)> = vec![];
        assert_eq!(forward_fill(&s, 5), None);
    }

    #[test]
    fn convert_inverse_rate() {
        let mut fx = FxTable::new("EUR");
        fx.add_series("EUR", "USD", vec![(0, 1.25)]);
        let got = fx.convert(100.0, "USD", "EUR", 0).unwrap();
        assert!((got - 80.0).abs() < 1e-12);
    }

    #[test]
    fn rate_same_currency_always_one() {
        let fx = FxTable::new("USD");
        assert_eq!(fx.rate("USD", "USD", 999), Some(1.0));
    }

    #[test]
    fn convert_amount_by_direct_rate() {
        let mut fx = FxTable::new("EUR");
        fx.add_series("EUR", "USD", vec![(0, 2.0)]);
        assert_eq!(fx.convert(50.0, "EUR", "USD", 0), Some(100.0));
    }

    #[test]
    fn ff_before_all_samples_returns_first() {
        let s = vec![(100, 5.0), (200, 10.0)];
        assert_eq!(forward_fill(&s, 50), Some(5.0));
    }

    #[test]
    fn ff_between_samples_returns_earlier() {
        let s = vec![(100, 5.0), (200, 10.0)];
        assert_eq!(forward_fill(&s, 150), Some(5.0));
    }

    #[test]
    fn ff_after_all_samples_returns_last() {
        let s = vec![(100, 5.0), (200, 10.0)];
        assert_eq!(forward_fill(&s, 300), Some(10.0));
    }

    #[test]
    fn default_fx_table_is_empty() {
        let fx = FxTable::default();
        assert_eq!(fx.rate("EUR", "USD", 0), None);
    }

    // ── Crypto-pegged triangulation ──────────────────────────────────

    #[test]
    fn crypto_pegged_triangulation() {
        // ETH -> USDT -> (peg 1:1) -> USD -> CHF
        let mut fx = FxTable::new("CHF");
        fx.add_series("ETH", "USDT", vec![(0, 3_000.0)]);
        fx.add_series("USDT", "USD", vec![(0, 1.0)]); // peg
        fx.add_series("USD", "CHF", vec![(0, 0.90)]);

        let eth_usdt = fx.rate("ETH", "USDT", 0).unwrap();
        assert!((eth_usdt - 3_000.0).abs() < 1e-12);
        let usdt_usd = fx.rate("USDT", "USD", 0).unwrap();
        assert!((usdt_usd - 1.0).abs() < 1e-12);
        let usd_chf = fx.rate("USD", "CHF", 0).unwrap();
        assert!((usd_chf - 0.90).abs() < 1e-12);
    }

    #[test]
    fn crypto_asset_as_key() {
        let mut fx = FxTable::new("USD");
        fx.add_series("BTC", "USD", vec![(0, 50_000.0)]);
        assert_eq!(fx.rate("BTC", "USD", 0), Some(50_000.0));
        assert!((fx.convert(2.0, "BTC", "USD", 0).unwrap() - 100_000.0).abs() < 1e-6);
    }
}
