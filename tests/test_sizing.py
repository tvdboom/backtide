"""Tests for position sizing strategies.

Verifies that all sizers calculate quantities correctly.
"""

import pytest

from backtide.sizers import (
    EqualWeight,
    FixedFractional,
    FixedNotional,
    FixedQuantity,
    KellyCriterion,
    RiskBased,
    VolatilityScaled,
)


class TestFixedQuantity:
    """Test FixedQuantity sizer."""

    def test_fixed_quantity_basic(self):
        """Test basic fixed quantity calculation."""
        sizer = FixedQuantity(10.0)
        qty = sizer.calculate(equity=10000, price=100)
        assert qty == 10.0

    def test_fixed_quantity_different_prices(self):
        """Quantity should be the same regardless of price."""
        sizer = FixedQuantity(10.0)
        qty1 = sizer.calculate(equity=10000, price=50)
        qty2 = sizer.calculate(equity=10000, price=200)
        assert qty1 == qty2 == 10.0


class TestFixedNotional:
    """Test FixedNotional sizer."""

    def test_fixed_notional_basic(self):
        """Quantity = amount / price."""
        sizer = FixedNotional(500)
        qty = sizer.calculate(equity=10000, price=100)
        assert qty == 5.0

    def test_fixed_notional_different_prices(self):
        """Higher prices should result in fewer units."""
        sizer = FixedNotional(500)
        qty1 = sizer.calculate(equity=10000, price=100)
        qty2 = sizer.calculate(equity=10000, price=200)
        assert qty1 == 5.0
        assert qty2 == 2.5

    def test_fixed_notional_ignore_equity(self):
        """Notional should not vary with equity."""
        sizer = FixedNotional(500)
        qty1 = sizer.calculate(equity=10000, price=100)
        qty2 = sizer.calculate(equity=100000, price=100)
        assert qty1 == qty2 == 5.0


class TestFixedFractional:
    """Test FixedFractional sizer."""

    def test_fixed_fractional_basic(self):
        """Quantity = (equity * fraction) / price."""
        sizer = FixedFractional(0.02)
        qty = sizer.calculate(equity=10000, price=100)
        assert qty == 2.0

    def test_fixed_fractional_scaling(self):
        """Position size should scale with equity."""
        sizer = FixedFractional(0.02)
        qty1 = sizer.calculate(equity=10000, price=100)
        qty2 = sizer.calculate(equity=20000, price=100)
        assert qty2 == 2 * qty1  # 4.0 and 2.0

    def test_fixed_fractional_different_prices(self):
        """Higher prices should result in fewer units."""
        sizer = FixedFractional(0.02)
        qty1 = sizer.calculate(equity=10000, price=100)
        qty2 = sizer.calculate(equity=10000, price=200)
        assert qty1 == 2.0
        assert qty2 == 1.0

    def test_fixed_fractional_invalid_fraction(self):
        """Fraction must be between 0 and 1."""
        sizer = FixedFractional(1.5)
        with pytest.raises(ValueError, match="fraction must be between 0 and 1"):
            sizer.calculate(equity=10000, price=100)

        sizer = FixedFractional(-0.05)
        with pytest.raises(ValueError, match="fraction must be between 0 and 1"):
            sizer.calculate(equity=10000, price=100)


class TestRiskBased:
    """Test RiskBased sizer."""

    def test_risk_based_basic(self):
        """Quantity = (equity * risk_pct) / stop_distance."""
        sizer = RiskBased(0.01)
        qty = sizer.calculate(equity=10000, price=100, stop_distance=5)
        # (10000 * 0.01) / 5 = 20
        assert qty == 20.0

    def test_risk_based_requires_stop_distance(self):
        """stop_distance is required."""
        sizer = RiskBased(0.01)
        with pytest.raises(ValueError, match="stop_distance"):
            sizer.calculate(equity=10000, price=100)

    def test_risk_based_different_stops(self):
        """Larger stop distance should result in larger position."""
        sizer = RiskBased(0.01)
        qty1 = sizer.calculate(equity=10000, price=100, stop_distance=5)
        qty2 = sizer.calculate(equity=10000, price=100, stop_distance=10)
        assert qty2 == qty1 / 2  # 20 and 10

    def test_risk_based_invalid_stop_distance(self):
        """stop_distance must be positive."""
        sizer = RiskBased(0.01)
        with pytest.raises(ValueError, match="stop_distance must be positive"):
            sizer.calculate(equity=10000, price=100, stop_distance=-5)

        with pytest.raises(ValueError, match="stop_distance must be positive"):
            sizer.calculate(equity=10000, price=100, stop_distance=0)


class TestVolatilityScaled:
    """Test VolatilityScaled sizer."""

    def test_volatility_scaled_basic(self):
        """Quantity = (equity * risk_pct) / atr."""
        sizer = VolatilityScaled(0.02)
        qty = sizer.calculate(equity=10000, price=100, atr=2.5)
        # (10000 * 0.02) / 2.5 = 80
        assert qty == 80.0

    def test_volatility_scaled_requires_atr(self):
        """Atr is required."""
        sizer = VolatilityScaled(0.02)
        with pytest.raises(ValueError, match="atr"):
            sizer.calculate(equity=10000, price=100)

    def test_volatility_scaled_high_volatility(self):
        """Higher ATR should result in smaller position."""
        sizer = VolatilityScaled(0.02)
        qty1 = sizer.calculate(equity=10000, price=100, atr=2.5)
        qty2 = sizer.calculate(equity=10000, price=100, atr=5.0)
        assert qty2 == qty1 / 2  # 40 and 80

    def test_volatility_scaled_invalid_atr(self):
        """ATR must be positive."""
        sizer = VolatilityScaled(0.02)
        with pytest.raises(ValueError, match="atr must be positive"):
            sizer.calculate(equity=10000, price=100, atr=-1.0)

        with pytest.raises(ValueError, match="atr must be positive"):
            sizer.calculate(equity=10000, price=100, atr=0)


class TestKellyCriterion:
    """Test KellyCriterion sizer."""

    def test_kelly_criterion_basic(self):
        """Test basic Kelly formula."""
        # win_rate: 55%, avg_win: $100, avg_loss: $100
        sizer = KellyCriterion(
            win_rate=0.55,
            avg_win=100,
            avg_loss=100,
            fraction=1.0,
        )
        qty = sizer.calculate(equity=10000, price=100)
        # kelly_pct = 0.55 - ((1 - 0.55) / (100 / 100)) = 0.55 - 0.45 = 0.10
        # allocation = 10000 * 0.10 * 1.0 = 1000
        # quantity = 1000 / 100 = 10
        assert abs(qty - 10.0) < 0.0001  # Allow for floating point precision

    def test_kelly_criterion_fraction(self):
        """Fraction should scale down allocation."""
        sizer_full = KellyCriterion(
            win_rate=0.55,
            avg_win=100,
            avg_loss=100,
            fraction=1.0,
        )
        sizer_half = KellyCriterion(
            win_rate=0.55,
            avg_win=100,
            avg_loss=100,
            fraction=0.5,
        )
        qty_full = sizer_full.calculate(equity=10000, price=100)
        qty_half = sizer_half.calculate(equity=10000, price=100)
        assert qty_half == qty_full / 2

    def test_kelly_criterion_negative_edge(self):
        """Negative edge (losing strategy) should result in 0 allocation."""
        sizer = KellyCriterion(
            win_rate=0.40,
            avg_win=100,
            avg_loss=100,
            fraction=1.0,
        )
        qty = sizer.calculate(equity=10000, price=100)
        # kelly_pct would be negative, but we cap at 0
        assert qty == 0.0

    def test_kelly_criterion_invalid_inputs(self):
        """Invalid inputs should raise errors."""
        # Invalid win_rate
        sizer = KellyCriterion(
            win_rate=1.5,
            avg_win=100,
            avg_loss=100,
            fraction=1.0,
        )
        with pytest.raises(ValueError, match="win_rate must be 0-1"):
            sizer.calculate(equity=10000, price=100)

        # Invalid avg_win
        sizer = KellyCriterion(
            win_rate=0.55,
            avg_win=-100,
            avg_loss=100,
            fraction=1.0,
        )
        with pytest.raises(ValueError, match="avg_win and avg_loss must be positive"):
            sizer.calculate(equity=10000, price=100)


class TestEqualWeight:
    """Test EqualWeight sizer."""

    def test_equal_weight_basic(self):
        """Quantity = (equity / n_positions) / price."""
        sizer = EqualWeight(10)
        qty = sizer.calculate(equity=100000, price=100)
        # (100000 / 10) / 100 = 10000 / 100 = 100
        assert qty == 100.0

    def test_equal_weight_different_positions(self):
        """Fewer positions should result in larger positions."""
        sizer_5 = EqualWeight(5)
        sizer_10 = EqualWeight(10)
        qty_5 = sizer_5.calculate(equity=100000, price=100)
        qty_10 = sizer_10.calculate(equity=100000, price=100)
        assert qty_5 == 2 * qty_10  # 200 and 100

    def test_equal_weight_different_prices(self):
        """Higher prices should result in fewer units."""
        sizer = EqualWeight(10)
        qty1 = sizer.calculate(equity=100000, price=100)
        qty2 = sizer.calculate(equity=100000, price=200)
        assert qty1 == 100.0
        assert qty2 == 50.0

    def test_equal_weight_invalid_n_positions(self):
        """n_positions must be > 0."""
        sizer = EqualWeight(0)
        with pytest.raises(ValueError, match="n_positions must be > 0"):
            sizer.calculate(equity=100000, price=100)


class TestInputValidation:
    """Test input validation for all sizers."""

    def test_invalid_equity_fixed_fractional(self):
        """Equity must be positive for sizers that use it."""
        sizer = FixedFractional(0.02)
        with pytest.raises(ValueError, match="equity and price must be positive"):
            sizer.calculate(equity=-10000, price=100)

        with pytest.raises(ValueError, match="equity and price must be positive"):
            sizer.calculate(equity=0, price=100)

    def test_invalid_price_fixed_fractional(self):
        """Price must be positive for sizers that use it."""
        sizer = FixedFractional(0.02)
        with pytest.raises(ValueError, match="equity and price must be positive"):
            sizer.calculate(equity=10000, price=-100)

        with pytest.raises(ValueError, match="equity and price must be positive"):
            sizer.calculate(equity=10000, price=0)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
