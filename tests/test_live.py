"""Backtide.

Author: Mavs
Description: Tests for live market data and live simulation.

"""

import inspect
from types import SimpleNamespace

import pytest

from backtide.backtest import Order, OrderStatus
from backtide.data import Currency
import backtide.live as live
from backtide.live import (
    LiveMarketFeed,
    MarketUpdate,
    Session,
    SessionConfig,
    SessionFill,
    SessionSnapshot,
    SessionUpdate,
    collect_market_updates,
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


class TestLiveModels:
    """Tests for public live data-model metadata."""

    @pytest.mark.parametrize(
        "model",
        [
            MarketUpdate,
            SessionFill,
            SessionConfig,
            SessionSnapshot,
            SessionUpdate,
        ],
    )
    def test_models_expose_rust_dataclass_marker(self, model):
        """Test AutoDocs renders every live data model as a dataclass."""
        assert model.__RUST_DATACLASS__ is True


class TestProviderSupport:
    """Tests for provider capability reporting."""

    def test_capability_helper_is_not_public(self):
        """Test that provider capability discovery stays an application detail."""
        assert not hasattr(live, "provider_live_support")

    def test_collector_signature_has_concrete_interval_default(self):
        """Test the public collector signature never exposes an opaque ellipsis."""
        signature = inspect.signature(collect_market_updates)

        assert signature.parameters["interval"].default == "1m"

    def test_collector_docstring_contains_api_reference_sections(self):
        """Test the collector exposes parameters and an example to AutoDocs."""
        docstring = inspect.getdoc(collect_market_updates)

        assert docstring is not None
        assert "Parameters\n----------" in docstring
        assert "provider : str | [Provider]" in docstring
        assert "include_partial : bool, default=True" in docstring
        assert "Examples\n--------" in docstring
        assert "updates = collect_market_updates(" in docstring

    def test_feed_validates_before_connecting(self):
        """Test invalid providers without making a network request."""
        with pytest.raises(ValueError, match="does not expose"):
            LiveMarketFeed("yahoo", ["AAPL"])

    def test_coinbase_accepts_only_five_minute_candles(self):
        """Test the Coinbase candle restriction through the public feed API."""
        with pytest.raises(ValueError, match="five-minute candles only"):
            LiveMarketFeed("coinbase", ["BTC-USD"], "1m")

        feed = LiveMarketFeed("coinbase", ["BTC-USD"], "5m")

        assert not feed.is_cancelled()

    @pytest.mark.parametrize("symbols", [[], [""], [" "]])
    def test_feed_rejects_empty_symbols(self, symbols):
        """Test symbol validation completes before opening a WebSocket."""
        with pytest.raises(ValueError, match="non-empty symbol"):
            LiveMarketFeed("binance", symbols)

    @pytest.mark.parametrize(
        ("options", "message"),
        [
            ({"reconnect_attempts": 0}, "reconnect_attempts must be positive"),
            ({"backoff_seconds": 0.0}, "backoff_seconds must be finite and positive"),
            ({"backoff_seconds": float("inf")}, "backoff_seconds must be finite and positive"),
            ({"backoff_seconds": float("nan")}, "backoff_seconds must be finite and positive"),
        ],
    )
    def test_feed_rejects_invalid_retry_options(self, options, message):
        """Test retry validation completes before opening a WebSocket."""
        with pytest.raises(ValueError, match=message):
            LiveMarketFeed("binance", ["BTC-USDT"], **options)

    def test_feed_cancel_reset_and_canceled_collection(self):
        """Test cancellation is idempotent and avoids network access during collection."""
        feed = LiveMarketFeed("binance", ["BTC-USDT"])

        feed.cancel()
        feed.cancel()

        assert feed.is_cancelled()
        assert feed.collect(max_events=1, timeout_seconds=0.01) == []
        feed.reset()
        assert not feed.is_cancelled()

    @pytest.mark.parametrize("timeout", [0.0, -1.0, float("inf"), float("nan")])
    def test_feed_rejects_invalid_collection_timeout(self, timeout):
        """Test collection timeout bounds without making a network request."""
        feed = LiveMarketFeed("binance", ["BTC-USDT"])
        feed.cancel()

        with pytest.raises(ValueError, match="timeout_seconds must be finite and positive"):
            feed.collect(timeout_seconds=timeout)

    def test_feed_rejects_zero_collection_limit(self):
        """Test a zero event limit is rejected before collection starts."""
        feed = LiveMarketFeed("binance", ["BTC-USDT"])
        feed.cancel()

        with pytest.raises(ValueError, match="max_events must be positive"):
            feed.collect(max_events=0)

    def test_currency_plan_resolves_crypto_quote_through_available_pairs(self, monkeypatch):
        """Test a live quote reaches the account currency through provider instruments."""
        instruments = [
            SimpleNamespace(symbol="AAVE-ETH", base="AAVE", quote="ETH"),
            SimpleNamespace(symbol="ETH-USDT", base="ETH", quote="USDT"),
            SimpleNamespace(symbol="EUR-USDT", base="EUR", quote="USDT"),
        ]

        def list_instruments(_provider, limit):
            del limit
            return instruments

        monkeypatch.setattr(live, "list_live_instruments", list_instruments)

        quotes, legs = live._live_currency_plan("binance", ["AAVE-ETH"], "EUR")

        assert quotes == {"AAVE-ETH": "ETH"}
        assert legs == {
            "ETH-USDT": ("ETH", "USDT"),
            "EUR-USDT": ("EUR", "USDT"),
        }


class TestSession:
    """Tests for deterministic simulated execution and accounting."""

    def test_market_order_updates_portfolio(self):
        """Test cash, positions, and equity after a market fill."""
        session = Session()
        result = session.on_bar(market(100.0), [Order("BTC-USD", 10.0)])

        assert result.fills[0].status == OrderStatus("Filled")
        assert result.snapshot.portfolio.positions == {"BTC-USD": 10.0}
        assert result.snapshot.portfolio.cash[Currency("USD")] == 99_000.0
        assert result.snapshot.equity == 100_000.0

    def test_round_trip_realizes_profit(self):
        """Test average-cost accounting over a complete trade."""
        session = Session()
        session.on_bar(market(100.0), [Order("BTC-USD", 10.0)])
        result = session.on_bar(market(110.0, 1_700_000_060), [Order("BTC-USD", -10.0)])

        assert result.snapshot.realized_pnl == 100.0
        assert result.snapshot.equity == 100_100.0

    def test_partial_candle_does_not_trade_by_default(self):
        """Test that an incomplete provider update only marks the account."""
        session = Session()
        result = session.on_bar(market(100.0, is_final=False), [Order("BTC-USD", 1.0)])

        assert not result.processed
        assert not result.fills
        assert result.snapshot.processed_bars == 0

    def test_duplicate_and_stale_final_candles_are_idempotent(self):
        """Test reconnect snapshots cannot repeat a strategy decision."""
        session = Session()
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
        session = Session(SessionConfig(initial_cash=100.0))
        short = session.on_bar(market(10.0), [Order("BTC-USD", -1.0)])
        margin = session.on_bar(
            market(10.0, 1_700_000_060),
            [Order("BTC-USD", 20.0)],
        )

        assert short.fills[0].status == OrderStatus("Rejected")
        assert margin.fills[0].status == OrderStatus("Rejected")
