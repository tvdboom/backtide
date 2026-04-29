"""Backtide.

Author: Mavs
Description: Built-in single-symbol buy-and-hold strategy used as benchmark."""

from __future__ import annotations

from backtide.utils.types import DataFrameLike
from backtide.strategies.base import BaseStrategy
from backtide.core.backtest import Order, OrderType
from backtide.core.backtest import Portfolio, State


class Benchmark(BaseStrategy):
    """Buy-and-hold a single benchmark symbol.

    Parameters
    ----------
    symbol : str
        The symbol to buy and hold. Must be present in the experiment's
        `data.symbols` so the engine actually fetches its bars.

    """

    def __init__(self, symbol: str):
        self.symbol = symbol

    def evaluate(
        self,
        data: DataFrameLike,
        portfolio: Portfolio,
        state: State,
        indicators: DataFrameLike,
    ) -> list[Order]:
        """Place a single market buy for `self.symbol` on the first bar."""
        # Local import keeps the module import-light and avoids loading the
        # Rust extension at strategy-definition time.

        # Already long or have a pending buy: nothing to do.
        if int(portfolio.positions.get(self.symbol, 0)) > 0:
            return []
        if any(o.symbol == self.symbol and o.quantity > 0 for o in portfolio.orders):
            return []

        # Locate the configured symbol in the per-symbol data view.
        df = data.get(self.symbol) if hasattr(data, "get") else None
        if df is None:
            return []

        try:
            last_close = float(df["close"].iloc[-1])
        except Exception:  # noqa: BLE001
            return []
        if not (last_close > 0):
            return []

        # Spend all available cash (sum of every currency balance).
        cash = float(sum(portfolio.cash.values()))
        qty = int(cash // last_close)
        if qty <= 0:
            return []

        return [
            Order(
                symbol=self.symbol,
                order_type=OrderType.Market,
                quantity=qty,
            )
        ]
