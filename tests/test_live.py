"""Backtide.

Author: Mavs
Description: Tests for live market data and paper trading.

"""

import pytest

from backtide.backtest import Order, OrderStatus
from backtide.data import Currency
from backtide.live import (
    LiveMarketFeed,
    MarketUpdate,
    PaperTradingConfig,
    PaperTradingSession,
    provider_live_support,
)


def market(close: float, timestamp: int = 1_700_000_000, *, is_final: bool = True):
    """Return a deterministic one-minute market update."""
    return MarketUpdate(
        symbol="BTC-USD",
        interval="1m",
        open_ts=timestamp,
        close_ts=timestamp + 60,
        open=close,
        high=close,
        low=close,
        close=close,
        volume=1.0,
        n_trades=1,
        is_final=is_final,
    )


class TestProviderSupport:
    """Tests for provider capability reporting."""

    def test_yahoo_has_explicit_websocket_limitation(self):
        """Test that Yahoo is rejected with an actionable explanation."""
        supported, explanation = provider_live_support("yahoo", "1m")
        assert not supported
        assert "does not expose" in explanation

    def test_coinbase_requires_five_minute_candles(self):
        """Test the Coinbase public candle interval restriction."""
        assert provider_live_support("coinbase", "5m")[0]
        assert not provider_live_support("coinbase", "1m")[0]

    def test_feed_validates_before_connecting(self):
        """Test invalid providers without making a network request."""
        with pytest.raises(ValueError, match="does not expose"):
            LiveMarketFeed("yahoo", ["AAPL"])


class TestPaperTradingSession:
    """Tests for deterministic paper execution and accounting."""

    def test_market_order_updates_portfolio(self):
        """Test cash, positions, and equity after a market fill."""
        session = PaperTradingSession()
        result = session.on_bar(market(100.0), [Order("BTC-USD", 10.0)])

        assert result.fills[0].status == OrderStatus("Filled")
        assert result.snapshot.portfolio.positions == {"BTC-USD": 10.0}
        assert result.snapshot.portfolio.cash[Currency("USD")] == 99_000.0
        assert result.snapshot.equity == 100_000.0

    def test_round_trip_realizes_profit(self):
        """Test average-cost accounting over a complete trade."""
        session = PaperTradingSession()
        session.on_bar(market(100.0), [Order("BTC-USD", 10.0)])
        result = session.on_bar(market(110.0, 1_700_000_060), [Order("BTC-USD", -10.0)])

        assert result.snapshot.realized_pnl == 100.0
        assert result.snapshot.equity == 100_100.0

    def test_partial_candle_does_not_trade_by_default(self):
        """Test that an incomplete provider update only marks the account."""
        session = PaperTradingSession()
        result = session.on_bar(market(100.0, is_final=False), [Order("BTC-USD", 1.0)])

        assert not result.processed
        assert not result.fills
        assert result.snapshot.processed_bars == 0

    def test_duplicate_and_stale_final_candles_are_idempotent(self):
        """Test reconnect snapshots cannot repeat a strategy decision."""
        session = PaperTradingSession()
        session.on_bar(market(100.0), [Order("BTC-USD", 1.0)])

        duplicate = session.on_bar(market(101.0), [Order("BTC-USD", 1.0)])
        stale = session.on_bar(market(90.0, 1_699_999_940), [Order("BTC-USD", 1.0)])

        assert not duplicate.processed
        assert not duplicate.fills
        assert not stale.processed
        assert stale.snapshot.processed_bars == 1
        assert stale.snapshot.portfolio.positions == {"BTC-USD": 1.0}

    def test_risk_guards_reject_short_and_margin(self):
        """Test default short-selling and cash constraints."""
        session = PaperTradingSession(PaperTradingConfig(initial_cash=100.0))
        short = session.on_bar(market(10.0), [Order("BTC-USD", -1.0)])
        margin = session.on_bar(
            market(10.0, 1_700_000_060),
            [Order("BTC-USD", 20.0)],
        )

        assert short.fills[0].status == OrderStatus("Rejected")
        assert margin.fills[0].status == OrderStatus("Rejected")
