"""Backtide.

Author: Mavs
Description: Background WebSocket paper-trading session management for the UI.

"""

from __future__ import annotations

from collections import deque
import threading
from typing import Any

from backtide.ui.services import APIError, _clean, public_attributes


class LiveTradingManager:
    """Run bounded WebSocket collection batches around a paper-trading session."""

    def __init__(self) -> None:
        self._lock = threading.RLock()
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._feed: Any = None
        self._session: Any = None
        self._config: dict[str, Any] = {}
        self._updates: deque[dict[str, Any]] = deque(maxlen=500)
        self._error: str | None = None

    def start(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Validate configuration and start collecting provider updates."""
        from backtide.live import PaperTradingConfig, PaperTradingSession, provider_live_support

        provider = str(payload.get("provider", "kraken")).lower()
        interval = str(payload.get("interval", "1m"))
        symbols = [str(symbol).strip().upper() for symbol in payload.get("symbols", [])]
        symbols = [symbol for symbol in symbols if symbol]
        if not symbols:
            raise APIError("Select at least one live symbol.")
        supported, reason = provider_live_support(provider, interval)
        if not supported:
            raise APIError(reason or f"{provider.title()} does not support live data.")
        with self._lock:
            if self._thread and self._thread.is_alive():
                raise APIError("A paper-trading session is already running.", 409)
            config_values = payload.get("config") or {}
            config = PaperTradingConfig(**config_values)
            strategy = self._load_strategy(payload.get("strategy"))
            self._session = PaperTradingSession(config, strategy)
            self._config = {
                "provider": provider,
                "interval": interval,
                "symbols": symbols,
                "strategy": payload.get("strategy"),
                "config": config_values,
            }
            self._updates.clear()
            self._error = None
            self._stop.clear()
            self._thread = threading.Thread(
                target=self._run,
                name="backtide-paper-trading",
                daemon=True,
            )
            self._thread.start()
        return self.status()

    def stop(self) -> dict[str, Any]:
        """Prevent another bounded collection batch from starting."""
        self._stop.set()
        feed = self._feed
        if feed is not None:
            feed.cancel()
        thread = self._thread
        if thread and thread.is_alive():
            thread.join(timeout=6.0)
        return self.status()

    def status(self) -> dict[str, Any]:
        """Return a consistent snapshot and recent live events."""
        with self._lock:
            running = bool(self._thread and self._thread.is_alive() and not self._stop.is_set())
            snapshot = (
                self._serialize_snapshot(self._session.snapshot()) if self._session else None
            )
            status = "running" if running else "error" if self._error else "stopped"
            if self._session is None:
                status = "idle"
            return {
                "status": status,
                "config": dict(self._config),
                "snapshot": snapshot,
                "updates": list(self._updates),
                "error": self._error,
            }

    def _run(self) -> None:
        from backtide.live import LiveMarketFeed

        self._feed = LiveMarketFeed(
            self._config["provider"],
            self._config["symbols"],
            self._config["interval"],
            include_partial=True,
        )

        while not self._stop.is_set():
            try:
                markets = self._feed.collect(max_events=10, timeout_seconds=5.0)
                for market in markets:
                    if self._stop.is_set():
                        break
                    update = self._session.on_bar(market)
                    with self._lock:
                        self._updates.append(self._serialize_update(update))
            except Exception as exc:  # noqa: BLE001
                with self._lock:
                    self._error = str(exc)
                self._stop.set()
        self._feed = None

    @staticmethod
    def _load_strategy(name: Any) -> Any:
        if not name:
            return None
        from backtide.config import get_config
        from backtide.strategies.utils import _load_stored_strategies

        strategies = _load_stored_strategies(get_config())
        if name not in strategies:
            raise APIError(f"Saved strategy {name!r} was not found.")
        return strategies[name]

    @classmethod
    def _serialize_snapshot(cls, snapshot: Any) -> dict[str, Any]:
        if snapshot is None:
            return {}
        output = public_attributes(
            snapshot,
            (
                "latest_prices",
                "equity",
                "realized_pnl",
                "unrealized_pnl",
                "processed_bars",
            ),
        )
        portfolio = getattr(snapshot, "portfolio", None)
        output["portfolio"] = (
            public_attributes(portfolio, ("cash", "positions", "orders")) if portfolio else {}
        )
        return _clean(output)

    @classmethod
    def _serialize_update(cls, update: Any) -> dict[str, Any]:
        market = public_attributes(
            update.market,
            (
                "symbol",
                "interval",
                "open_ts",
                "close_ts",
                "open",
                "high",
                "low",
                "close",
                "volume",
                "is_final",
                "provider",
                "received_ts",
            ),
        )
        fills = []
        for fill in update.fills:
            row = public_attributes(
                fill,
                ("timestamp", "status", "fill_price", "commission", "realized_pnl", "reason"),
            )
            row["order"] = public_attributes(
                fill.order,
                ("id", "symbol", "order_type", "quantity", "price", "limit_price"),
            )
            fills.append(row)
        return _clean(
            {
                "market": market,
                "fills": fills,
                "orders_submitted": update.orders_submitted,
                "processed": update.processed,
                "snapshot": cls._serialize_snapshot(update.snapshot),
            }
        )
