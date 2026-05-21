use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

/// Register the Python interface for `backtide.core.sizers`.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "backtide.sizers")?;

    m.add_class::<EqualWeight>()?;
    m.add_class::<FixedFractional>()?;
    m.add_class::<FixedNotional>()?;
    m.add_class::<FixedQuantity>()?;
    m.add_class::<KellyCriterion>()?;
    m.add_class::<RiskBased>()?;
    m.add_class::<VolatilityScaled>()?;

    parent.add_submodule(&m)?;

    parent.py().import("sys")?.getattr("modules")?.set_item("backtide.core.sizers", &m)?;

    Ok(())
}

/// Trait carrying the actual sizing logic for every sizer.
pub trait Sizer {
    /// Calculate the quantity to trade.
    fn calculate(
        &self,
        equity: f64,
        price: f64,
        stop_distance: Option<f64>,
        atr: Option<f64>,
    ) -> PyResult<f64>;
}

/// Emit a `#[pymethods]` block exposing `calculate()` to Python.
///
/// The Python method delegates to `<$ty as Sizer>::calculate(...)`, so the
/// real sizing logic stays in the trait impl and only this macro carries
/// the user-facing docstring.
macro_rules! sizer_pymethods {
    ($ty:ident) => {
        #[pymethods]
        impl $ty {
            /// Calculate the order quantity for this sizer.
            ///
            /// Parameters
            /// ----------
            /// equity : float
            ///     Current portfolio equity in the same currency as `price`.
            ///     When a sizer is attached to an order, the engine passes
            ///     equity converted to that instrument's quote currency.
            ///
            /// price : float
            ///     Reference price of the instrument.
            ///
            /// stop_distance : float | None, default=None
            ///     Distance from entry to stop loss, in price units.
            ///
            /// atr : float | None, default=None
            ///     Current ATR value. Required for volatility-based sizers.
            ///
            /// Returns
            /// -------
            /// int | float
            ///     The number of units to trade. Positive for buys, negative for sells.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If a required input is missing or invalid.
            #[pyo3(signature = (equity: "float", price: "float", stop_distance: "float | None" = None, atr: "float | None" = None))]
            fn calculate(
                &self,
                equity: f64,
                price: f64,
                stop_distance: Option<f64>,
                atr: Option<f64>,
            ) -> PyResult<f64> {
                <$ty as Sizer>::calculate(self, equity, price, stop_distance, atr)
            }
        }
    };
}

/// Divide current equity equally across a fixed number of positions.
///
/// Computes `quantity = (equity / n_positions) / price`. Useful for
/// portfolio-level rotation strategies where every selected symbol gets
/// the same allocation regardless of price or volatility.
///
/// Parameters
/// ----------
/// n_positions : int
///     Number of concurrent positions to split the equity across.
///
/// See Also
/// --------
/// - backtide.sizers:BaseSizer
/// - backtide.sizers:FixedFractional
/// - backtide.sizers:FixedNotional
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.sizers")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EqualWeight {
    n_positions: u32,
}

#[pymethods]
impl EqualWeight {
    #[new]
    pub fn new(n_positions: u32) -> Self {
        EqualWeight {
            n_positions,
        }
    }

    fn __repr__(&self) -> String {
        format!("EqualWeight({})", self.n_positions)
    }
}

impl Sizer for EqualWeight {
    fn calculate(
        &self,
        equity: f64,
        price: f64,
        _stop_distance: Option<f64>,
        _atr: Option<f64>,
    ) -> PyResult<f64> {
        if equity <= 0.0 || price <= 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "equity and price must be positive",
            ));
        }
        if self.n_positions == 0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("n_positions must be > 0"));
        }
        let allocation_per_position = equity / (self.n_positions as f64);
        Ok(allocation_per_position / price)
    }
}

sizer_pymethods!(EqualWeight);

/// Allocate a fixed percentage of total current equity.
///
/// Computes `quantity = (equity * fraction) / price`. The position size
/// scales with the portfolio: as equity grows, allocations grow, and
/// vice versa. This is the most common sizing rule.
///
/// Parameters
/// ----------
/// fraction : float
///     Fraction of equity to allocate per trade. Must be in `(0, 1]`.
///
/// See Also
/// --------
/// - backtide.sizers:BaseSizer
/// - backtide.sizers:EqualWeight
/// - backtide.sizers:FixedNotional
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.sizers")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixedFractional {
    fraction: f64,
}

#[pymethods]
impl FixedFractional {
    #[new]
    pub fn new(fraction: f64) -> Self {
        FixedFractional {
            fraction,
        }
    }

    fn __repr__(&self) -> String {
        format!("FixedFractional({})", self.fraction)
    }
}

impl Sizer for FixedFractional {
    fn calculate(
        &self,
        equity: f64,
        price: f64,
        _stop_distance: Option<f64>,
        _atr: Option<f64>,
    ) -> PyResult<f64> {
        if equity <= 0.0 || price <= 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "equity and price must be positive",
            ));
        }
        if self.fraction <= 0.0 || self.fraction > 1.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "fraction must be between 0 and 1",
            ));
        }
        Ok((equity * self.fraction) / price)
    }
}

sizer_pymethods!(FixedFractional);

/// Buy a fixed amount of currency worth of the asset.
///
/// Computes `quantity = amount / price`. Keeps cash exposure consistent
/// across symbols regardless of price level, but ignores portfolio size.
///
/// Parameters
/// ----------
/// amount : float
///     Cash amount to spend per trade, in the instrument's quote currency.
///
/// See Also
/// --------
/// - backtide.sizers:BaseSizer
/// - backtide.sizers:FixedFractional
/// - backtide.sizers:FixedQuantity
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.sizers")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixedNotional {
    amount: f64,
}

#[pymethods]
impl FixedNotional {
    #[new]
    pub fn new(amount: f64) -> Self {
        FixedNotional {
            amount,
        }
    }

    fn __repr__(&self) -> String {
        format!("FixedNotional({})", self.amount)
    }
}

impl Sizer for FixedNotional {
    fn calculate(
        &self,
        _equity: f64,
        price: f64,
        _stop_distance: Option<f64>,
        _atr: Option<f64>,
    ) -> PyResult<f64> {
        if price <= 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("price must be positive"));
        }
        Ok(self.amount / price)
    }
}

sizer_pymethods!(FixedNotional);

/// Buy exactly N units.
///
/// Returns the configured `quantity` regardless of price or equity. Simple,
/// price-naive sizing — appropriate for crypto base units or quick prototyping.
///
/// Parameters
/// ----------
/// quantity : int | float
///     The exact number of units to trade per order.
///
/// See Also
/// --------
/// - backtide.sizers:BaseSizer
/// - backtide.sizers:FixedFractional
/// - backtide.sizers:FixedNotional
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.sizers")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixedQuantity {
    quantity: f64,
}

#[pymethods]
impl FixedQuantity {
    #[new]
    pub fn new(quantity: f64) -> Self {
        FixedQuantity {
            quantity,
        }
    }

    fn __repr__(&self) -> String {
        format!("FixedQuantity({})", self.quantity)
    }
}

impl Sizer for FixedQuantity {
    fn calculate(
        &self,
        _equity: f64,
        _price: f64,
        _stop_distance: Option<f64>,
        _atr: Option<f64>,
    ) -> PyResult<f64> {
        Ok(self.quantity)
    }
}

sizer_pymethods!(FixedQuantity);

/// Kelly Criterion sizing.
///
/// Computes the theoretically optimal fraction of capital to risk for long-run
/// geometric growth: `kelly_pct = win_rate - ((1 - win_rate) / (avg_win / avg_loss))`,
/// then `quantity = (equity * kelly_pct * fraction) / price`. The `fraction`
/// multiplier (e.g., 0.25 for "quarter Kelly") tames drawdowns at the cost of
/// slower growth.
///
/// Parameters
/// ----------
/// win_rate : float
///     Historical fraction of winning trades, in `[0, 1]`.
///
/// avg_win : float
///     Average profit of winning trades. Must be positive.
///
/// avg_loss : float
///     Average loss of losing trades, expressed as a positive number.
///
/// fraction : float
///     Kelly multiplier (typically 0.25–0.5 for half/quarter Kelly).
///
/// See Also
/// --------
/// - backtide.sizers:BaseSizer
/// - backtide.sizers:FixedFractional
/// - backtide.sizers:RiskBased
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.sizers")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KellyCriterion {
    win_rate: f64,
    avg_win: f64,
    avg_loss: f64,
    fraction: f64,
}

#[pymethods]
impl KellyCriterion {
    #[new]
    #[pyo3(signature = (win_rate: "float", avg_win: "float", avg_loss: "float", fraction: "float" = 0.25))]
    pub fn new(win_rate: f64, avg_win: f64, avg_loss: f64, fraction: f64) -> Self {
        KellyCriterion {
            win_rate,
            avg_win,
            avg_loss,
            fraction,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "KellyCriterion({}, {}, {}, {})",
            self.win_rate, self.avg_win, self.avg_loss, self.fraction
        )
    }
}

impl Sizer for KellyCriterion {
    fn calculate(
        &self,
        equity: f64,
        price: f64,
        _stop_distance: Option<f64>,
        _atr: Option<f64>,
    ) -> PyResult<f64> {
        if equity <= 0.0 || price <= 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "equity and price must be positive",
            ));
        }
        if self.win_rate < 0.0 || self.win_rate > 1.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("win_rate must be 0-1"));
        }
        if self.avg_win <= 0.0 || self.avg_loss <= 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "avg_win and avg_loss must be positive",
            ));
        }
        let win_loss_ratio = self.avg_win / self.avg_loss;
        let kelly_pct = self.win_rate - ((1.0 - self.win_rate) / win_loss_ratio);
        let kelly_pct = kelly_pct.max(0.0);
        let allocation = equity * kelly_pct * self.fraction;
        Ok(allocation / price)
    }
}

sizer_pymethods!(KellyCriterion);

/// Size based on acceptable risk and stop loss distance.
///
/// Computes `quantity = (equity * risk_pct) / stop_distance`. Industry standard
/// approach: you define how much equity you're willing to lose and the distance
/// to your stop, and the sizer works backwards. Requires `stop_distance` to be
/// passed to `calculate()`.
///
/// Parameters
/// ----------
/// risk_pct : float
///     Fraction of equity at risk per trade (e.g. `0.01` for 1%).
///
/// See Also
/// --------
/// - backtide.sizers:BaseSizer
/// - backtide.sizers:KellyCriterion
/// - backtide.sizers:VolatilityScaled
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.sizers")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiskBased {
    risk_pct: f64,
}

#[pymethods]
impl RiskBased {
    #[new]
    pub fn new(risk_pct: f64) -> Self {
        RiskBased {
            risk_pct,
        }
    }

    fn __repr__(&self) -> String {
        format!("RiskBased({})", self.risk_pct)
    }
}

impl Sizer for RiskBased {
    fn calculate(
        &self,
        equity: f64,
        _price: f64,
        stop_distance: Option<f64>,
        _atr: Option<f64>,
    ) -> PyResult<f64> {
        if equity <= 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("equity must be positive"));
        }
        let stop_dist = stop_distance.ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>("RiskBased requires stop_distance")
        })?;
        if stop_dist <= 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "stop_distance must be positive",
            ));
        }
        Ok((equity * self.risk_pct) / stop_dist)
    }
}

sizer_pymethods!(RiskBased);

/// Size based on volatility (ATR).
///
/// Computes `quantity = (equity * risk_pct) / atr`. Like [`RiskBased`] but uses
/// the instrument's Average True Range as a proxy for stop distance, automatically
/// shrinking positions on volatile assets and growing them on calm ones. Requires
/// `atr` to be passed to `calculate()`.
///
/// Parameters
/// ----------
/// risk_pct : float
///     Fraction of equity to risk per trade (e.g., `0.02` for 2%).
///
/// See Also
/// --------
/// - backtide.sizers:BaseSizer
/// - backtide.sizers:FixedFractional
/// - backtide.sizers:RiskBased
#[pyclass(skip_from_py_object, get_all, set_all, module = "backtide.sizers")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VolatilityScaled {
    risk_pct: f64,
}

#[pymethods]
impl VolatilityScaled {
    #[new]
    pub fn new(risk_pct: f64) -> Self {
        VolatilityScaled {
            risk_pct,
        }
    }

    fn __repr__(&self) -> String {
        format!("VolatilityScaled({})", self.risk_pct)
    }
}

impl Sizer for VolatilityScaled {
    fn calculate(
        &self,
        equity: f64,
        _price: f64,
        _stop_distance: Option<f64>,
        atr: Option<f64>,
    ) -> PyResult<f64> {
        if equity <= 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("equity must be positive"));
        }
        let atr_val = atr.ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>("VolatilityScaled requires atr")
        })?;
        if atr_val <= 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("atr must be positive"));
        }
        Ok((equity * self.risk_pct) / atr_val)
    }
}

sizer_pymethods!(VolatilityScaled);

#[cfg(test)]
mod tests {
    //! Tests for every built-in sizer.
    //!
    //! These exercise the [`Sizer`] trait implementations directly (the
    //! Python-facing `calculate()` method is a one-line delegator to the
    //! same trait, so covering the trait is sufficient). For each sizer we
    //! check:
    //!
    //! * a happy-path numeric result against the documented formula,
    //! * validation errors for the inputs the sizer rejects, and
    //! * that optional arguments it does not consume are silently ignored.

    use super::*;

    /// Helper: assert that `result` is a `PyValueError` containing `needle`.
    ///
    /// PyO3 needs a Python interpreter to inspect a `PyErr`'s message, so the
    /// helper wraps the check in `Python::attach`.
    fn assert_value_error<T: std::fmt::Debug>(result: PyResult<T>, needle: &str) {
        let err = result.expect_err("expected PyValueError");
        Python::attach(|py| {
            assert!(
                err.is_instance_of::<pyo3::exceptions::PyValueError>(py),
                "expected ValueError, got {err:?}",
            );
            let msg = err.value(py).to_string();
            assert!(msg.contains(needle), "expected message to contain {needle:?}, got {msg:?}");
        });
    }

    // ── EqualWeight ──────────────────────────────────────────────────────

    #[test]
    fn equal_weight_splits_equity_across_positions() {
        // 10_000 / 4 slots = 2_500 per slot; at price 50 → 50 units.
        let sizer = EqualWeight::new(4);
        let qty = sizer.calculate(10_000.0, 50.0, None, None).unwrap();
        assert!((qty - 50.0).abs() < MIN_POSITION);
    }

    #[test]
    fn equal_weight_ignores_stop_distance_and_atr() {
        // Passing irrelevant optional inputs must not change the result.
        let sizer = EqualWeight::new(2);
        let bare = sizer.calculate(1_000.0, 10.0, None, None).unwrap();
        let with_extras = sizer.calculate(1_000.0, 10.0, Some(5.0), Some(0.5)).unwrap();
        assert_eq!(bare, with_extras);
    }

    #[test]
    fn equal_weight_rejects_zero_positions() {
        let sizer = EqualWeight::new(0);
        assert_value_error(sizer.calculate(1_000.0, 10.0, None, None), "n_positions");
    }

    #[test]
    fn equal_weight_rejects_non_positive_equity_or_price() {
        let sizer = EqualWeight::new(2);
        assert_value_error(sizer.calculate(0.0, 10.0, None, None), "equity and price");
        assert_value_error(sizer.calculate(1_000.0, -1.0, None, None), "equity and price");
    }

    // ── FixedFractional ──────────────────────────────────────────────────

    #[test]
    fn fixed_fractional_uses_equity_fraction() {
        // 10% of 10_000 = 1_000; at price 25 → 40 units.
        let sizer = FixedFractional::new(0.10);
        let qty = sizer.calculate(10_000.0, 25.0, None, None).unwrap();
        assert!((qty - 40.0).abs() < MIN_POSITION);
    }

    #[test]
    fn fixed_fractional_rejects_fraction_out_of_range() {
        // fraction must be in (0, 1]. Boundary values 0.0 and >1.0 must
        // both error.
        assert_value_error(
            FixedFractional::new(0.0).calculate(1_000.0, 10.0, None, None),
            "fraction",
        );
        assert_value_error(
            FixedFractional::new(1.5).calculate(1_000.0, 10.0, None, None),
            "fraction",
        );
    }

    #[test]
    fn fixed_fractional_rejects_non_positive_equity_or_price() {
        let sizer = FixedFractional::new(0.5);
        assert_value_error(sizer.calculate(-1.0, 10.0, None, None), "equity and price");
        assert_value_error(sizer.calculate(1_000.0, 0.0, None, None), "equity and price");
    }

    // ── FixedNotional ────────────────────────────────────────────────────

    #[test]
    fn fixed_notional_ignores_equity() {
        // The whole point of FixedNotional is that it ignores `equity`.
        let sizer = FixedNotional::new(500.0);
        let small_equity = sizer.calculate(100.0, 50.0, None, None).unwrap();
        let big_equity = sizer.calculate(1_000_000.0, 50.0, None, None).unwrap();
        assert_eq!(small_equity, big_equity);
        assert!((small_equity - 10.0).abs() < MIN_POSITION);
    }

    #[test]
    fn fixed_notional_rejects_non_positive_price() {
        let sizer = FixedNotional::new(500.0);
        assert_value_error(sizer.calculate(10_000.0, 0.0, None, None), "price");
        assert_value_error(sizer.calculate(10_000.0, -5.0, None, None), "price");
    }

    // ── FixedQuantity ────────────────────────────────────────────────────

    #[test]
    fn fixed_quantity_returns_configured_value() {
        // Returns the configured quantity regardless of any input.
        let sizer = FixedQuantity::new(3.5);
        assert_eq!(sizer.calculate(0.0, 0.0, None, None).unwrap(), 3.5);
        assert_eq!(sizer.calculate(1.0e9, 1.0e-9, Some(0.01), Some(0.02)).unwrap(), 3.5);
    }

    // ── KellyCriterion ───────────────────────────────────────────────────

    #[test]
    fn kelly_criterion_computes_expected_fraction() {
        // win_rate=0.6, avg_win=2, avg_loss=1 → kelly_pct = 0.6 - 0.4/2 = 0.4.
        // With fraction=0.5 (half-Kelly): allocation = 10_000 * 0.4 * 0.5 = 2_000.
        // At price 100 → 20 units.
        let sizer = KellyCriterion::new(0.6, 2.0, 1.0, 0.5);
        let qty = sizer.calculate(10_000.0, 100.0, None, None).unwrap();
        assert!((qty - 20.0).abs() < MIN_POSITION);
    }

    #[test]
    fn kelly_criterion_clamps_negative_edge_to_zero() {
        // win_rate=0.3, avg_win=avg_loss → kelly = 0.3 - 0.7 = -0.4, clamped at 0.
        let sizer = KellyCriterion::new(0.3, 1.0, 1.0, 1.0);
        let qty = sizer.calculate(10_000.0, 100.0, None, None).unwrap();
        assert_eq!(qty, 0.0);
    }

    #[test]
    fn kelly_criterion_validates_inputs() {
        // win_rate out of [0, 1]
        assert_value_error(
            KellyCriterion::new(1.5, 1.0, 1.0, 0.25).calculate(1_000.0, 10.0, None, None),
            "win_rate",
        );
        // non-positive avg_win / avg_loss
        assert_value_error(
            KellyCriterion::new(0.5, 0.0, 1.0, 0.25).calculate(1_000.0, 10.0, None, None),
            "avg_win",
        );
        assert_value_error(
            KellyCriterion::new(0.5, 1.0, -1.0, 0.25).calculate(1_000.0, 10.0, None, None),
            "avg_loss",
        );
    }

    // ── RiskBased ────────────────────────────────────────────────────────

    #[test]
    fn risk_based_scales_by_stop_distance() {
        // 1% of 10_000 = 100 at risk; stop_distance=2 → 50 units.
        let sizer = RiskBased::new(0.01);
        let qty = sizer.calculate(10_000.0, 100.0, Some(2.0), None).unwrap();
        assert!((qty - 50.0).abs() < MIN_POSITION);
    }

    #[test]
    fn risk_based_requires_stop_distance() {
        let sizer = RiskBased::new(0.01);
        assert_value_error(sizer.calculate(10_000.0, 100.0, None, None), "stop_distance");
        assert_value_error(sizer.calculate(10_000.0, 100.0, Some(0.0), None), "stop_distance");
    }

    // ── VolatilityScaled ─────────────────────────────────────────────────

    #[test]
    fn volatility_scaled_scales_by_atr() {
        // 2% of 10_000 = 200 at risk; atr=4 → 50 units.
        let sizer = VolatilityScaled::new(0.02);
        let qty = sizer.calculate(10_000.0, 100.0, None, Some(4.0)).unwrap();
        assert!((qty - 50.0).abs() < MIN_POSITION);
    }

    #[test]
    fn volatility_scaled_requires_atr() {
        let sizer = VolatilityScaled::new(0.02);
        assert_value_error(sizer.calculate(10_000.0, 100.0, None, None), "atr");
        assert_value_error(sizer.calculate(10_000.0, 100.0, None, Some(-1.0)), "atr");
    }

    // ── __repr__ outputs ─────────────────────────────────────────────────

    #[test]
    fn repr_matches_python_constructor_form() {
        assert_eq!(EqualWeight::new(4).__repr__(), "EqualWeight(4)");
        assert_eq!(FixedFractional::new(0.25).__repr__(), "FixedFractional(0.25)");
        assert_eq!(FixedNotional::new(500.0).__repr__(), "FixedNotional(500)");
        assert_eq!(FixedQuantity::new(3.0).__repr__(), "FixedQuantity(3)");
        assert_eq!(RiskBased::new(0.01).__repr__(), "RiskBased(0.01)");
        assert_eq!(VolatilityScaled::new(0.02).__repr__(), "VolatilityScaled(0.02)");
        assert_eq!(
            KellyCriterion::new(0.6, 2.0, 1.0, 0.5).__repr__(),
            "KellyCriterion(0.6, 2, 1, 0.5)"
        );
    }

    // ── Additional validation branches ───────────────────────────────────

    #[test]
    fn risk_based_rejects_non_positive_equity() {
        let sizer = RiskBased::new(0.01);
        assert_value_error(sizer.calculate(0.0, 100.0, Some(2.0), None), "equity");
        assert_value_error(sizer.calculate(-1.0, 100.0, Some(2.0), None), "equity");
    }

    #[test]
    fn volatility_scaled_rejects_non_positive_equity() {
        let sizer = VolatilityScaled::new(0.02);
        assert_value_error(sizer.calculate(0.0, 100.0, None, Some(4.0)), "equity");
        assert_value_error(sizer.calculate(-1.0, 100.0, None, Some(4.0)), "equity");
    }

    #[test]
    fn kelly_criterion_rejects_non_positive_equity_or_price() {
        let sizer = KellyCriterion::new(0.6, 2.0, 1.0, 0.5);
        assert_value_error(sizer.calculate(0.0, 100.0, None, None), "equity and price");
        assert_value_error(sizer.calculate(10_000.0, 0.0, None, None), "equity and price");
    }

    // ── Additional edge/coverage tests ────────────────────────────────

    #[test]
    fn fixed_fractional_fraction_one_allocates_full_equity() {
        let sizer = FixedFractional::new(1.0);
        let qty = sizer.calculate(1_000.0, 50.0, None, None).unwrap();
        assert!((qty - 20.0).abs() < MIN_POSITION);
    }

    #[test]
    fn fixed_fractional_rejects_negative_fraction() {
        assert_value_error(
            FixedFractional::new(-0.5).calculate(1_000.0, 10.0, None, None),
            "fraction",
        );
    }

    #[test]
    fn fixed_notional_amt_larger_than_price() {
        let sizer = FixedNotional::new(1_000.0);
        let qty = sizer.calculate(10_000.0, 100.0, None, None).unwrap();
        assert!((qty - 10.0).abs() < MIN_POSITION);
    }

    #[test]
    fn risk_based_large_stop_distance_yields_small_qty() {
        let sizer = RiskBased::new(0.01);
        let large = sizer.calculate(10_000.0, 100.0, Some(100.0), None).unwrap();
        let small = sizer.calculate(10_000.0, 100.0, Some(1.0), None).unwrap();
        assert!(large < small);
    }

    #[test]
    fn volatility_scaled_large_atr_yields_small_qty() {
        let sizer = VolatilityScaled::new(0.02);
        let large = sizer.calculate(10_000.0, 100.0, None, Some(100.0)).unwrap();
        let small = sizer.calculate(10_000.0, 100.0, None, Some(1.0)).unwrap();
        assert!(large < small);
    }

    #[test]
    fn kelly_criterion_full_kelly() {
        // Full kelly (fraction=1.0)
        let sizer = KellyCriterion::new(0.6, 2.0, 1.0, 1.0);
        let qty = sizer.calculate(10_000.0, 100.0, None, None).unwrap();
        // kelly_pct = 0.6 - 0.4/2 = 0.4; allocation = 10000 * 0.4 = 4000; qty = 40
        assert!((qty - 40.0).abs() < MIN_POSITION);
    }

    #[test]
    fn kelly_criterion_negative_win_rate_errors() {
        assert_value_error(
            KellyCriterion::new(-0.1, 1.0, 1.0, 0.25).calculate(1_000.0, 10.0, None, None),
            "win_rate",
        );
    }

    #[test]
    fn equal_weight_one_position() {
        let sizer = EqualWeight::new(1);
        let qty = sizer.calculate(5_000.0, 50.0, None, None).unwrap();
        assert!((qty - 100.0).abs() < MIN_POSITION);
    }
}
