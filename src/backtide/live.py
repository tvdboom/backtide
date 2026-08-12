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
)

try:
    from backtide.core.live import list_live_instruments
except ImportError:

    def list_live_instruments(provider: str, limit: int = 10_000) -> list[object]:
        """Report that the installed Rust extension predates instrument discovery."""
        del provider, limit
        raise RuntimeError(
            "The installed Backtide extension does not support live instrument discovery. "
            "Run `just build` and restart Backtide."
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
    "list_live_instruments",
]
