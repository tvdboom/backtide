"""Backtide.

Author: Mavs
Description: Live market data and deterministic paper trading.

"""

from backtide.core.live import (
    LiveMarketFeed,
    MarketUpdate,
    PaperFill,
    PaperTradingConfig,
    PaperTradingSession,
    PaperTradingSnapshot,
    PaperTradingUpdate,
    collect_market_updates,
    provider_live_support,
)

__all__ = [
    "LiveMarketFeed",
    "MarketUpdate",
    "PaperFill",
    "PaperTradingConfig",
    "PaperTradingSession",
    "PaperTradingSnapshot",
    "PaperTradingUpdate",
    "collect_market_updates",
    "provider_live_support",
]
