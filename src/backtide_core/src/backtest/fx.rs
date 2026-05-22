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

use crate::backtest::models::ConversionPeriod;
use crate::constants::{Cash, CashAmount, MIN_POSITION};
use crate::data::models::Currency;
use chrono::{DateTime, Datelike, Utc};
use itertools::Itertools;
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

/// Try to debit `amount` of `ccy` from `cash`.
///
/// If `ccy` doesn't have enough, fall back to the base currency and then any
/// other foreign bucket. Conversions are made at the FX rate observed at `ts`.
/// Returns `false` if no combination of available cash covers the debit.
pub fn try_debit(
    cash: &mut Cash,
    ccy: Currency,
    amount: f64,
    base: Currency,
    fx: &FxTable,
    ts: i64,
) -> bool {
    if amount <= 0.0 {
        return true;
    }

    // 1. Pay directly out of `ccy`.
    let avail = cash.amount(&ccy);
    if avail >= amount {
        *cash.entry(ccy).or_insert(0.0) -= amount;
        return true;
    }

    // 2. Drain the existing `ccy` bucket first, remember the residual.
    let mut remaining = amount - avail.max(0.0);

    // 3. Cover the residual from the base currency at the current FX rate.
    let base_avail = if ccy == base {
        0.0
    } else {
        cash.amount(&base)
    };

    let needed_base = match fx.rate(&ccy.to_string(), &base.to_string(), ts) {
        Some(r) if r > 0.0 => remaining * r,
        _ => f64::INFINITY,
    };

    if needed_base.is_finite() && base_avail >= needed_base {
        cash.remove(&ccy);
        *cash.entry(base).or_insert(0.0) -= needed_base;
        return true;
    }

    // 4. Drain other foreign buckets in deterministic order.
    let buckets: Vec<(Currency, f64)> = cash
        .iter()
        .filter(|(c, v)| **c != ccy && **c != base && v.is_finite() && **v > 0.0)
        .map(|(c, v)| (*c, *v))
        .sorted_by(|a, b| a.0.to_string().cmp(&b.0.to_string()))
        .collect();

    // Tentatively zero `ccy` and reduce base.
    let mut staged: Vec<(Currency, f64)> = Vec::new();
    let staged_ccy_drain = avail.max(0.0);
    let mut staged_base_drain = if base_avail > 0.0 {
        base_avail
    } else {
        0.0
    };

    if needed_base.is_finite() {
        staged_base_drain = staged_base_drain.min(needed_base);
        let covered_in_ccy = if staged_base_drain > 0.0 {
            match fx.rate(&base.to_string(), &ccy.to_string(), ts) {
                Some(r) if r > 0.0 => staged_base_drain * r,
                _ => 0.0,
            }
        } else {
            0.0
        };

        remaining = (remaining - covered_in_ccy).max(0.0);
    } else {
        staged_base_drain = 0.0;
    }

    for (other_ccy, other_avail) in buckets {
        if remaining <= 0.0 {
            break;
        }

        let r = match fx.rate(&other_ccy.to_string(), &ccy.to_string(), ts) {
            Some(r) if r > 0.0 => r,
            _ => continue,
        };

        let other_in_ccy = other_avail * r;
        if other_in_ccy >= remaining {
            staged.push((other_ccy, remaining / r));
            remaining = 0.0;
        } else {
            staged.push((other_ccy, other_avail));
            remaining -= other_in_ccy;
        }
    }

    if remaining > 0.0 {
        return false;
    }

    // Commit drains.
    if staged_ccy_drain > 0.0 {
        *cash.entry(ccy).or_insert(0.0) -= staged_ccy_drain;
    }

    if staged_base_drain > 0.0 {
        *cash.entry(base).or_insert(0.0) -= staged_base_drain;
    }

    for (c, v) in staged {
        *cash.entry(c).or_insert(0.0) -= v;
    }

    // Remove buckets drained to zero so they don't linger in equity snapshots.
    cash.retain(|_, v| v.abs() > MIN_POSITION);

    true
}

/// Sweep every non-base currency bucket into the base currency
///
/// Conversion is done at the FX rate observed at `ts`. If `threshold` is
/// `Some(t)`, only buckets whose value in base currency is `>= t` are swept,
/// otherwise every foreign bucket with a positive (or negative) finite
/// balance is converted.
pub fn sweep_foreign_to_base(
    cash: &mut Cash,
    base: Currency,
    fx: &FxTable,
    ts: i64,
    threshold: Option<f64>,
) {
    let foreign: Vec<Currency> = cash
        .iter()
        .filter(|(c, v)| **c != base && v.is_finite() && v.abs() > 0.0)
        .map(|(c, _)| *c)
        .collect();

    for ccy in foreign {
        let amount = match cash.get(&ccy) {
            Some(v) if v.is_finite() && v.abs() > 0.0 => v,
            _ => continue,
        };

        let in_base = match fx.convert(*amount, &ccy.to_string(), &base.to_string(), ts) {
            Some(v) => v,
            None => continue,
        };

        if let Some(t) = threshold {
            if in_base.abs() < t {
                continue;
            }
        }

        cash.remove(&ccy);
        *cash.entry(base).or_insert(0.0) += in_base;
    }
}

/// Return a coarse "bucket" identifier for `ts` under the given
/// conversion period. Two timestamps falling into different buckets
/// trigger an end-of-period sweep.
pub fn period_bucket(ts: i64, period: ConversionPeriod) -> i64 {
    let dt = DateTime::<Utc>::from_timestamp(ts, 0)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());

    match period {
        ConversionPeriod::Day => ts.div_euclid(86_400),
        ConversionPeriod::Week => {
            let iso = dt.iso_week(); // ISO week-year combined identifier.
            (iso.year() as i64) * 100 + iso.week() as i64
        },
        ConversionPeriod::Month => (dt.year() as i64) * 12 + (dt.month0() as i64),
        ConversionPeriod::Year => dt.year() as i64,
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
    use crate::constants::MIN_POSITION;

    #[test]
    fn direct_and_inverse() {
        let mut fx = FxTable::new("EUR");
        // 1 EUR = 1.10 USD
        fx.add_series("EUR", "USD", vec![(0, 1.10)]);
        assert_eq!(fx.rate("EUR", "USD", 0), Some(1.10));
        assert!((fx.rate("USD", "EUR", 0).unwrap() - 1.0 / 1.10).abs() < MIN_POSITION);
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
        assert!((got - 0.13 * 1.10).abs() < MIN_POSITION);
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
        assert!((got - 80.0).abs() < MIN_POSITION);
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
        assert!((eth_usdt - 3_000.0).abs() < MIN_POSITION);
        let usdt_usd = fx.rate("USDT", "USD", 0).unwrap();
        assert!((usdt_usd - 1.0).abs() < MIN_POSITION);
        let usd_chf = fx.rate("USD", "CHF", 0).unwrap();
        assert!((usd_chf - 0.90).abs() < MIN_POSITION);
    }

    #[test]
    fn crypto_asset_as_key() {
        let mut fx = FxTable::new("USD");
        fx.add_series("BTC", "USD", vec![(0, 50_000.0)]);
        assert_eq!(fx.rate("BTC", "USD", 0), Some(50_000.0));
        assert!((fx.convert(2.0, "BTC", "USD", 0).unwrap() - 100_000.0).abs() < 1e-6);
    }
}
