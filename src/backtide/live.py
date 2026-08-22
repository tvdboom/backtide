"""Backtide.

Author: Mavs
Description: Live market data and deterministic simulated sessions.

"""

from collections import defaultdict, deque
from typing import Protocol, cast

from backtide.core.live import (
    LiveMarketFeed,
    MarketUpdate,
    Session,
    SessionConfig,
    SessionFill,
    SessionSnapshot,
    SessionUpdate,
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


class _LiveInstrument(Protocol):
    """Describe the catalog fields required for live currency routing."""

    symbol: str
    base: object | None
    quote: object


def _live_currency_plan(
    provider: str,
    symbols: list[str],
    base_currency: str,
) -> tuple[dict[str, str], dict[str, tuple[str, str]]]:
    """Resolve target quote currencies and the shortest live conversion legs."""
    catalog = cast(list[_LiveInstrument], list_live_instruments(provider, limit=10_000))
    instruments = {str(instrument.symbol).upper(): instrument for instrument in catalog}
    target_quotes: dict[str, str] = {}
    adjacency: dict[str, list[tuple[str, str, str, str]]] = defaultdict(list)

    for instrument in catalog:
        instrument_base = getattr(instrument, "base", None)
        if instrument_base is None:
            continue
        pair_base = str(instrument_base).upper()
        pair_quote = str(instrument.quote).upper()
        pair_symbol = str(instrument.symbol).upper()
        adjacency[pair_base].append((pair_quote, pair_symbol, pair_base, pair_quote))
        adjacency[pair_quote].append((pair_base, pair_symbol, pair_base, pair_quote))

    normalized_base = str(base_currency).upper()
    conversion_legs: dict[str, tuple[str, str]] = {}
    for symbol in symbols:
        normalized_symbol = symbol.upper()
        instrument = instruments.get(normalized_symbol)
        if instrument is None:
            raise ValueError(f"Live instrument {normalized_symbol!r} was not found on {provider}.")
        quote = str(instrument.quote).upper()
        target_quotes[normalized_symbol] = quote
        if quote == normalized_base:
            continue

        queue: deque[tuple[str, list[tuple[str, str, str]]]] = deque([(quote, [])])
        visited = {quote}
        path: list[tuple[str, str, str]] | None = None
        while queue:
            currency, current_path = queue.popleft()
            for neighbor, leg_symbol, leg_base, leg_quote in sorted(adjacency[currency]):
                if neighbor in visited:
                    continue
                next_path = [*current_path, (leg_symbol, leg_base, leg_quote)]
                if neighbor == normalized_base:
                    path = next_path
                    queue.clear()
                    break
                visited.add(neighbor)
                queue.append((neighbor, next_path))
        if path is None:
            raise ValueError(
                f"No live conversion path from {quote} to {normalized_base} exists on {provider}."
            )
        for leg_symbol, leg_base, leg_quote in path:
            conversion_legs[leg_symbol] = (leg_base, leg_quote)

    return target_quotes, conversion_legs


__all__ = [
    "LiveMarketFeed",
    "MarketUpdate",
    "Session",
    "SessionConfig",
    "SessionFill",
    "SessionSnapshot",
    "SessionUpdate",
    "collect_market_updates",
    "list_live_instruments",
]
