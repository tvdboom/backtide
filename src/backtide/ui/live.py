"""Backtide.

Author: Mavs
Description: Background WebSocket paper-trading session management for the UI.

"""

from __future__ import annotations

from collections import deque
from datetime import UTC, datetime
import importlib
import inspect
import json
from pathlib import Path
import re
import shutil
import threading
import time
from typing import Any
import uuid

from backtide.ui.services import APIError, _clean, dataframe_records, public_attributes


class LiveTradingManager:
    """Coordinate observable, persistent paper-trading and replay sessions."""

    def __init__(self, storage_root: Path | None = None) -> None:
        self._lock = threading.RLock()
        self._stop = threading.Event()
        self._paused = threading.Event()
        self._thread: threading.Thread | None = None
        self._feed: Any = None
        self._session: Any = None
        self._sessions: dict[str, Any] = {}
        self._config: dict[str, Any] = {}
        self._updates: deque[dict[str, Any]] = deque(maxlen=500)
        self._recent_order_outcomes: dict[str, deque[dict[str, Any]]] = {}
        self._error: str | None = None
        self._session_id: str | None = None
        self._started_at: str | None = None
        self._last_message_at: str | None = None
        self._received_events = 0
        self._warmup_loaded = 0
        self._flatten_requested = False
        self._cancel_requested = False
        self._target_quotes: dict[str, str] = {}
        self._conversion_legs: dict[str, tuple[str, str]] = {}
        self._exchange_rates: dict[str, dict[str, Any]] = {}
        self._replay_speed = 0.0
        self._replay_total_events = 0
        self._replay_processed_events = 0
        self._replay_source_duration = 0.0
        self._replay_warmup_source = "none"
        self._configured_storage_root = storage_root

    def start(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Validate configuration, warm strategies, and start a live paper session."""
        from backtide.live import LiveMarketFeed, PaperTradingConfig, PaperTradingSession

        provider = str(payload.get("provider", "kraken")).lower()
        interval = str(payload.get("interval", "1m"))
        symbols = self._symbols(payload.get("symbols", []))
        try:
            validation_feed = LiveMarketFeed(provider, symbols, interval)
            validation_feed.cancel()
        except (RuntimeError, TypeError, ValueError) as exc:
            raise APIError(str(exc)) from exc

        with self._lock:
            self._ensure_idle()
            config_values = self._supported_config_values(
                PaperTradingConfig,
                payload.get("config") or {},
            )
            base_currency = str(config_values.get("base_currency", "USD")).upper()
            inferred_quotes = {symbol: symbol.rsplit("-", 1)[-1].upper() for symbol in symbols}
            self._target_quotes = inferred_quotes
            self._conversion_legs = {}
            if any(quote != base_currency for quote in inferred_quotes.values()):
                live_module = importlib.import_module("backtide.live")
                planner = getattr(live_module, "_live_currency_plan", None)
                if planner is not None:
                    try:
                        self._target_quotes, self._conversion_legs = planner(
                            provider,
                            symbols,
                            base_currency,
                        )
                    except (RuntimeError, TypeError, ValueError) as exc:
                        raise APIError(str(exc)) from exc
            strategy_names = self._strategy_names(payload)
            strategy_objects = self._load_strategies(strategy_names)
            indicators = self._load_indicators(payload.get("indicators") or [])
            self._sessions = {}
            try:
                for name, strategy in strategy_objects:
                    config = PaperTradingConfig(**config_values)
                    self._sessions[name] = (
                        PaperTradingSession(config, strategy, indicators)
                        if indicators
                        else PaperTradingSession(config, strategy)
                    )
            except (RuntimeError, TypeError, ValueError) as exc:
                self._sessions.clear()
                raise APIError(str(exc)) from exc
            self._session = next(iter(self._sessions.values()))
            self._config = {
                "mode": "paper",
                "provider": provider,
                "interval": interval,
                "symbols": symbols,
                "strategy": strategy_names[0] if len(strategy_names) == 1 else None,
                "strategies": strategy_names,
                "indicators": [str(name) for name in payload.get("indicators") or []],
                "config": config_values,
                "target_quotes": self._target_quotes,
                "conversion_legs": {
                    symbol: {"base": base, "quote": quote}
                    for symbol, (base, quote) in self._conversion_legs.items()
                },
                "warmup_bars": max(0, int(payload.get("warmup_bars", 0))),
            }
            self._prepare_session()
            try:
                self._warmup_loaded = self._warm_up_sessions(persist=True)
                self._persist_manifest("running")
            except (OSError, RuntimeError, TypeError, ValueError) as exc:
                self._session = None
                self._sessions.clear()
                raise APIError(f"Could not prepare paper session: {exc}") from exc
            self._thread = threading.Thread(
                target=self._run,
                name="backtide-paper-trading",
                daemon=True,
            )
            self._thread.start()
        return self.status()

    def replay(
        self,
        session_id: str,
        playback_speed: float | str = "max",
    ) -> dict[str, Any]:
        """Replay recorded market events through a fresh paper engine."""
        from backtide.live import MarketUpdate, PaperTradingConfig, PaperTradingSession

        record = self.session(session_id)
        config = record.get("config") or {}
        events = record.get("updates") or []
        speed = self._playback_speed(playback_speed)
        with self._lock:
            self._ensure_idle()
            config_values = self._supported_config_values(
                PaperTradingConfig,
                config.get("config") or {},
            )
            strategy_names = [str(value) for value in config.get("strategies") or []]
            strategy_objects = self._load_strategies(strategy_names)
            indicators = self._load_indicators(config.get("indicators") or [])
            self._sessions = {}
            try:
                for name, strategy in strategy_objects:
                    paper = PaperTradingConfig(**config_values)
                    self._sessions[name] = (
                        PaperTradingSession(paper, strategy, indicators)
                        if indicators
                        else PaperTradingSession(paper, strategy)
                    )
            except (RuntimeError, TypeError, ValueError) as exc:
                self._sessions.clear()
                raise APIError(str(exc)) from exc
            self._session = next(iter(self._sessions.values()))
            self._config = {
                **config,
                "mode": "replay",
                "source_session_id": session_id,
                "playback_speed": speed,
            }
            self._target_quotes = {
                str(symbol): str(quote)
                for symbol, quote in (config.get("target_quotes") or {}).items()
            }
            self._conversion_legs = {
                str(symbol): (str(value["base"]), str(value["quote"]))
                for symbol, value in (config.get("conversion_legs") or {}).items()
            }
            self._prepare_session()
            self._replay_speed = speed
            self._replay_total_events = len(events)
            self._replay_source_duration = self._event_span(events)
            recorded_warmup = [MarketUpdate(**values) for values in record.get("warmup") or []]
            if recorded_warmup:
                self._replay_warmup_source = "recorded"
                self._warmup_loaded = self._warm_up_sessions(
                    markets=recorded_warmup,
                    persist=True,
                )
            elif int(config.get("warmup_bars") or 0) > 0:
                self._replay_warmup_source = "storage"
                self._warmup_loaded = self._warm_up_sessions(persist=True)
                if not self._warmup_loaded:
                    self._replay_warmup_source = "unavailable"

            def runner() -> None:
                try:
                    previous_timestamp = None
                    for index, event in enumerate(events):
                        if not self._wait_until_replay_resumed():
                            break
                        event_timestamp = self._event_timestamp(event)
                        if (
                            previous_timestamp is not None
                            and event_timestamp is not None
                            and speed > 0.0
                            and not self._wait_replay_delay(
                                max(0.0, event_timestamp - previous_timestamp) / speed
                            )
                        ):
                            break
                        if not self._wait_until_replay_resumed():
                            break
                        market_values = event.get("market") or {}
                        market = MarketUpdate(**market_values)
                        source_received_at = event.get("received_at")
                        if source_received_at is None:
                            source_received_at = market_values.get("received_ts")
                        for symbol, rate in (event.get("exchange_rates") or {}).items():
                            self._record_exchange_rate(
                                str(symbol),
                                str(rate["base"]),
                                str(rate["quote"]),
                                float(rate["rate"]),
                                int(rate["timestamp"]),
                            )
                        self._process_market(market, received_at=source_received_at)
                        self._replay_processed_events = index + 1
                        if event_timestamp is not None:
                            previous_timestamp = event_timestamp
                except Exception as exc:  # noqa: BLE001
                    with self._lock:
                        self._error = str(exc)
                finally:
                    self._stop.set()
                    self._persist_manifest("error" if self._error else "stopped")

            try:
                self._persist_manifest("running")
            except OSError as exc:
                self._session = None
                self._sessions.clear()
                raise APIError(f"Could not prepare replay session: {exc}") from exc
            self._thread = threading.Thread(
                target=runner,
                name="backtide-paper-replay",
                daemon=True,
            )
            self._thread.start()
        return self.status()

    def stop(self) -> dict[str, Any]:
        """Stop collection and persist the final account state."""
        self._stop.set()
        feed = self._feed
        if feed is not None:
            feed.cancel()
        thread = self._thread
        if thread and thread.is_alive():
            thread.join(timeout=6.0)
        self._persist_manifest("error" if self._error else "stopped")
        return self.status()

    def pause(self) -> dict[str, Any]:
        """Pause strategy evaluation while keeping the market connection alive."""
        self._paused.set()
        if self._session is not None:
            self._persist_manifest("paused")
        return self.status()

    def resume(self) -> dict[str, Any]:
        """Resume strategy evaluation after a pause."""
        self._paused.clear()
        if self._session is not None:
            self._persist_manifest("running")
        return self.status()

    def flatten(self) -> dict[str, Any]:
        """Request deterministic liquidation on the next market update."""
        self._flatten_requested = True
        return self.status()

    def cancel_all(self) -> dict[str, Any]:
        """Request cancellation of every resting order on the next market update."""
        self._cancel_requested = True
        return self.status()

    def status(self) -> dict[str, Any]:
        """Return account snapshots, recent events, and connection diagnostics."""
        with self._lock:
            running = bool(self._thread and self._thread.is_alive() and not self._stop.is_set())
            snapshots = {
                name: self._serialize_snapshot(session.snapshot())
                for name, session in self._sessions.items()
            }
            snapshot = self._aggregate_snapshots(snapshots)
            status = (
                "paused"
                if running and self._paused.is_set()
                else "running"
                if running
                else ("error" if self._error else "stopped")
            )
            if self._session is None:
                status = "idle"
            return {
                "id": self._session_id,
                "status": status,
                "config": dict(self._config),
                "snapshot": snapshot,
                "strategies": snapshots,
                "updates": list(self._updates),
                "recent_order_outcomes": {
                    name: list(reversed(outcomes))
                    for name, outcomes in self._recent_order_outcomes.items()
                },
                "health": {
                    "started_at": self._started_at,
                    "last_message_at": self._last_message_at,
                    "received_events": self._received_events,
                    "warmup_bars_loaded": self._warmup_loaded,
                    "paused": self._paused.is_set(),
                },
                "replay": self._replay_status() if self._config.get("mode") == "replay" else None,
                "error": self._error,
            }

    def sessions(self) -> list[dict[str, Any]]:
        """List newest-first persisted paper and replay sessions."""
        records = []
        root = self._storage_root()
        if not root.exists():
            return records
        with self._lock:
            active_session_id = (
                self._session_id
                if self._thread and self._thread.is_alive() and not self._stop.is_set()
                else None
            )
            for manifest in root.glob("*/manifest.json"):
                try:
                    record = json.loads(manifest.read_text(encoding="utf-8"))
                except (OSError, json.JSONDecodeError):
                    continue
                records.append(
                    self._reconcile_persisted_status(record, manifest, active_session_id)
                )
        return sorted(records, key=lambda item: str(item.get("started_at", "")), reverse=True)

    def session(self, session_id: str) -> dict[str, Any]:
        """Return one persisted session with its bounded event journal."""
        self._validate_session_id(session_id)
        folder = self._storage_root() / session_id
        manifest = folder / "manifest.json"
        if not manifest.is_file():
            raise APIError(f"Paper session {session_id!r} was not found.", 404)
        with self._lock:
            active_session_id = (
                self._session_id
                if self._thread and self._thread.is_alive() and not self._stop.is_set()
                else None
            )
            result = self._reconcile_persisted_status(
                json.loads(manifest.read_text(encoding="utf-8")),
                manifest,
                active_session_id,
            )
        updates = []
        journal = folder / "events.jsonl"
        if journal.is_file():
            for line in journal.read_text(encoding="utf-8").splitlines():
                try:
                    updates.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
        result["updates"] = updates
        result["warmup"] = self._read_json_lines(folder / "warmup.jsonl")
        return result

    def delete_session(self, session_id: str) -> dict[str, int]:
        """Delete one inactive persisted paper session and its event journal."""
        self._validate_session_id(session_id)
        folder = self._storage_root() / session_id
        with self._lock:
            active = bool(
                self._session_id == session_id
                and self._thread
                and self._thread.is_alive()
                and not self._stop.is_set()
            )
            if active:
                raise APIError("Stop the active paper session before deleting it.", 409)
            if not (folder / "manifest.json").is_file():
                raise APIError(f"Paper session {session_id!r} was not found.", 404)
            try:
                shutil.rmtree(folder)
            except OSError as exc:
                raise APIError(f"Could not delete paper session {session_id!r}.", 500) from exc
        return {"deleted": 1}

    @staticmethod
    def _validate_session_id(session_id: str) -> None:
        """Reject identifiers that could escape the paper-session storage root."""
        if not re.fullmatch(r"[0-9a-f]{16}", session_id):
            raise APIError("Paper session id is invalid.", 400)

    def _reconcile_persisted_status(
        self,
        record: dict[str, Any],
        manifest: Path,
        active_session_id: str | None,
    ) -> dict[str, Any]:
        """Close a manifest that claims activity without a matching worker."""
        if record.get("status") not in {"running", "paused"}:
            return record
        if record.get("id") == active_session_id:
            return record

        record = {**record, "status": "stopped"}
        record["finished_at"] = record.get("finished_at") or self._now()
        try:
            manifest.write_text(
                json.dumps(_clean(record), indent=2, sort_keys=True),
                encoding="utf-8",
            )
        except OSError:
            # The history response must still reflect reality if repair cannot be persisted.
            pass
        return record

    def _prepare_session(self) -> None:
        self._updates.clear()
        self._recent_order_outcomes.clear()
        self._error = None
        self._stop.clear()
        self._paused.clear()
        self._session_id = uuid.uuid4().hex[:16]
        self._started_at = self._now()
        self._last_message_at = None
        self._received_events = 0
        self._warmup_loaded = 0
        self._flatten_requested = False
        self._cancel_requested = False
        self._exchange_rates.clear()
        self._replay_speed = 0.0
        self._replay_total_events = 0
        self._replay_processed_events = 0
        self._replay_source_duration = 0.0
        self._replay_warmup_source = "none"

    def _run(self) -> None:
        from backtide.live import LiveMarketFeed

        try:
            self._feed = LiveMarketFeed(
                self._config["provider"],
                [*self._config["symbols"], *self._conversion_legs],
                self._config["interval"],
                include_partial=True,
            )
            while not self._stop.is_set():
                markets = self._feed.collect(max_events=10, timeout_seconds=5.0)
                for market in markets:
                    if self._stop.is_set():
                        break
                    self._process_market(market)
        except Exception as exc:  # noqa: BLE001
            with self._lock:
                self._error = str(exc)
            self._stop.set()
        finally:
            self._feed = None
            self._persist_manifest("error" if self._error else "stopped")

    def _process_market(self, market: Any, *, received_at: Any | None = None) -> None:
        """Process an update while preserving its precise replay receipt time."""
        processed_at = self._now()
        self._last_message_at = processed_at
        self._received_events += 1
        conversion_leg = self._conversion_legs.get(str(market.symbol))
        if conversion_leg is not None:
            self._record_exchange_rate(
                str(market.symbol),
                conversion_leg[0],
                conversion_leg[1],
                float(market.close),
                int(market.close_ts),
            )
            if str(market.symbol) not in self._config.get("symbols", []):
                return
        if set(self._conversion_legs) - set(self._exchange_rates):
            return
        if self._paused.is_set() and self._config.get("mode") != "replay":
            return
        flatten_requested = self._flatten_requested
        cancel_requested = self._cancel_requested
        results = {}
        for name, session in self._sessions.items():
            orders = self._control_orders(
                session,
                flatten_requested=flatten_requested,
                cancel_requested=cancel_requested,
            )
            results[name] = session.on_bar(market, orders or None)
        update = self._serialize_combined_update(market, results)
        update["exchange_rates"] = dict(self._exchange_rates)
        update["received_at"] = processed_at if received_at is None else received_at
        with self._lock:
            self._updates.append(update)
            for name, strategy_update in update.get("strategies", {}).items():
                outcomes = self._recent_order_outcomes.setdefault(name, deque(maxlen=12))
                outcomes.extend(strategy_update.get("fills") or [])
            self._append_event(update)
        controls_processed = bool(results) and all(result.processed for result in results.values())
        if controls_processed:
            if flatten_requested:
                self._flatten_requested = False
            if cancel_requested:
                self._cancel_requested = False

    def _record_exchange_rate(
        self,
        symbol: str,
        base: str,
        quote: str,
        rate: float,
        timestamp: int,
    ) -> None:
        """Record one provider conversion leg in every independent account."""
        for session in self._sessions.values():
            setter = getattr(session, "set_exchange_rate", None)
            if setter is not None:
                setter(base, quote, rate, timestamp)
        self._exchange_rates[symbol] = {
            "base": base,
            "quote": quote,
            "rate": rate,
            "timestamp": timestamp,
        }

    @staticmethod
    def _control_orders(
        session: Any,
        *,
        flatten_requested: bool,
        cancel_requested: bool,
    ) -> list[Any]:
        if not flatten_requested and not cancel_requested:
            return []
        from backtide.backtest import Order

        snapshot = session.snapshot()
        orders = []
        if cancel_requested:
            orders.extend(
                [
                    Order(
                        open_order.symbol,
                        0.0,
                        "Cancel",
                        id=str(open_order.id),
                    )
                    for open_order in snapshot.portfolio.orders
                ]
            )
        if flatten_requested:
            orders.extend(
                Order(symbol, -quantity, "Market")
                for symbol, quantity in snapshot.portfolio.positions.items()
            )
        return orders

    def _warm_up_sessions(
        self,
        *,
        markets: list[Any] | None = None,
        persist: bool = False,
    ) -> int:
        limit = int(self._config.get("warmup_bars") or 0)
        if (markets is None and limit <= 0) or not any(
            hasattr(session, "warm_up") for session in self._sessions.values()
        ):
            return 0
        from backtide.live import MarketUpdate

        if markets is None:
            from backtide.storage import query_bars

            requested_symbols = [*self._config["symbols"], *self._conversion_legs]
            rows = dataframe_records(
                query_bars(
                    requested_symbols,
                    self._config["interval"],
                    self._config["provider"],
                    limit=limit * len(requested_symbols),
                )
            )
            markets = [
                MarketUpdate(
                    symbol=str(row["symbol"]),
                    interval=str(row.get("interval") or self._config["interval"]),
                    open_ts=int(row["open_ts"]),
                    close_ts=int(row["close_ts"]),
                    open=float(row["open"]),
                    high=float(row["high"]),
                    low=float(row["low"]),
                    close=float(row.get("adj_close") or row["close"]),
                    volume=float(row.get("volume") or 0.0),
                    n_trades=row.get("n_trades"),
                    is_final=True,
                    provider=str(row.get("provider") or self._config["provider"]),
                    received_ts=int(row["close_ts"]),
                    quote_currency=self._target_quotes.get(str(row["symbol"])),
                )
                for row in sorted(rows, key=lambda value: int(value.get("open_ts") or 0))
            ]
        if persist:
            self._persist_warmup(markets)
        strategy_markets = []
        for market in markets:
            conversion_leg = self._conversion_legs.get(str(market.symbol))
            if conversion_leg is None:
                strategy_markets.append(market)
                continue
            self._record_exchange_rate(
                str(market.symbol),
                conversion_leg[0],
                conversion_leg[1],
                float(market.close),
                int(market.close_ts),
            )
        for session in self._sessions.values():
            if hasattr(session, "warm_up"):
                session.warm_up(strategy_markets)
        return len(strategy_markets)

    @staticmethod
    def _playback_speed(value: float | str) -> float:
        """Return a bounded replay speed where zero means maximum speed."""
        if isinstance(value, str) and value.strip().lower() in {"max", "maximum", "instant"}:
            return 0.0
        try:
            speed = float(value)
        except (TypeError, ValueError) as exc:
            raise APIError("Replay speed must be between 0.1x and 100x, or 'max'.", 400) from exc
        if not 0.1 <= speed <= 100.0:
            raise APIError("Replay speed must be between 0.1x and 100x, or 'max'.", 400)
        return speed

    @staticmethod
    def _event_timestamp(event: dict[str, Any]) -> float | None:
        """Return one recorded event timestamp in epoch seconds."""
        value = event.get("received_at")
        if value is None:
            market = event.get("market") or {}
            value = market.get("received_ts", market.get("close_ts"))
        if isinstance(value, (int, float)):
            timestamp = float(value)
            return timestamp / 1_000.0 if timestamp > 10_000_000_000 else timestamp
        if isinstance(value, str):
            try:
                return datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp()
            except ValueError:
                return None
        return None

    @classmethod
    def _event_span(cls, events: list[dict[str, Any]]) -> float:
        """Return elapsed recorded time between the first and last valid event."""
        timestamps = [
            value for event in events if (value := cls._event_timestamp(event)) is not None
        ]
        return max(0.0, timestamps[-1] - timestamps[0]) if len(timestamps) > 1 else 0.0

    def _wait_until_replay_resumed(self) -> bool:
        """Wait without consuming replay events while playback is paused."""
        while self._paused.is_set() and not self._stop.is_set():
            self._stop.wait(0.05)
        return not self._stop.is_set()

    def _wait_replay_delay(self, seconds: float) -> bool:
        """Wait for replay time while excluding time spent paused."""
        remaining = seconds
        previous = time.monotonic()
        while remaining > 0.0 and not self._stop.is_set():
            if self._paused.is_set():
                if not self._wait_until_replay_resumed():
                    return False
                previous = time.monotonic()
                continue
            self._stop.wait(min(remaining, 0.05))
            current = time.monotonic()
            remaining -= current - previous
            previous = current
        return not self._stop.is_set()

    def _replay_status(self) -> dict[str, Any]:
        """Return playback progress and reproducibility details."""
        total = self._replay_total_events
        processed = self._replay_processed_events
        return {
            "speed": self._replay_speed,
            "processed_events": processed,
            "total_events": total,
            "progress": processed / total if total else 1.0,
            "source_duration_seconds": self._replay_source_duration,
            "warmup_source": self._replay_warmup_source,
            "warmup_bars_loaded": self._warmup_loaded,
        }

    @staticmethod
    def _symbols(values: Any) -> list[str]:
        symbols = [str(symbol).strip().upper() for symbol in values]
        symbols = list(dict.fromkeys(symbol for symbol in symbols if symbol))
        if not symbols:
            raise APIError("Select at least one live symbol.")
        return symbols

    @staticmethod
    def _strategy_names(payload: dict[str, Any]) -> list[str]:
        values = payload.get("strategies")
        if values is None:
            values = [payload.get("strategy")] if payload.get("strategy") else []
        return list(dict.fromkeys(str(value) for value in values if value))

    @staticmethod
    def _supported_config_values(config_type: Any, values: Any) -> dict[str, Any]:
        """Keep only fields accepted by the loaded paper-engine configuration class."""
        if not isinstance(values, dict):
            raise APIError("Paper-trading configuration must be an object.")
        try:
            parameters = inspect.signature(config_type).parameters.values()
        except (TypeError, ValueError):
            return dict(values)
        if any(parameter.kind is inspect.Parameter.VAR_KEYWORD for parameter in parameters):
            return dict(values)
        accepted = {parameter.name for parameter in parameters if parameter.name != "self"}
        return {name: value for name, value in values.items() if name in accepted}

    @classmethod
    def _load_strategies(cls, names: list[str]) -> list[tuple[str, Any]]:
        if not names:
            return [("Monitor", None)]
        return [(name, cls._load_strategy(name)) for name in names]

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

    @staticmethod
    def _load_indicators(names: Any) -> list[Any]:
        if not names:
            return []
        from backtide.config import get_config
        from backtide.indicators.utils import _load_stored_indicators

        stored = _load_stored_indicators(get_config())
        missing = [str(name) for name in names if str(name) not in stored]
        if missing:
            raise APIError(f"Saved indicator {missing[0]!r} was not found.")
        return [stored[str(name)] for name in names]

    def _ensure_idle(self) -> None:
        if self._thread and self._thread.is_alive():
            raise APIError("A paper-trading session is already running.", 409)

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
                "gross_exposure",
                "net_exposure",
                "leverage",
                "buying_power",
                "drawdown",
                "peak_equity",
                "total_costs",
                "trading_halted",
                "halt_reason",
                "metrics",
            ),
        )
        portfolio = getattr(snapshot, "portfolio", None)
        if portfolio:
            output["portfolio"] = public_attributes(portfolio, ("cash", "positions"))
            output["portfolio"]["orders"] = [
                cls._serialize_order(order) for order in getattr(portfolio, "orders", [])
            ]
        else:
            output["portfolio"] = {}
        return _clean(output)

    @classmethod
    def _serialize_update(cls, update: Any) -> dict[str, Any]:
        market = cls._serialize_market(update.market)
        fills = cls._serialize_fills(update.fills)
        return _clean(
            {
                "market": market,
                "fills": fills,
                "orders_submitted": update.orders_submitted,
                "processed": update.processed,
                "snapshot": cls._serialize_snapshot(update.snapshot),
                "indicators": getattr(update, "indicators", {}),
            }
        )

    @classmethod
    def _serialize_combined_update(cls, market: Any, results: dict[str, Any]) -> dict[str, Any]:
        serialized = {name: cls._serialize_update(update) for name, update in results.items()}
        snapshots = {name: value["snapshot"] for name, value in serialized.items()}
        fills = []
        for name, value in serialized.items():
            fills.extend({**fill, "strategy": name} for fill in value["fills"])
        first = next(iter(serialized.values()))
        return {
            "market": cls._serialize_market(market),
            "fills": fills,
            "orders_submitted": sum(value["orders_submitted"] for value in serialized.values()),
            "processed": any(value["processed"] for value in serialized.values()),
            "snapshot": cls._aggregate_snapshots(snapshots),
            "strategies": serialized,
            "indicators": first.get("indicators", {}),
        }

    @staticmethod
    def _serialize_market(market: Any) -> dict[str, Any]:
        return _clean(
            public_attributes(
                market,
                (
                    "symbol",
                    "quote_currency",
                    "interval",
                    "open_ts",
                    "close_ts",
                    "open",
                    "high",
                    "low",
                    "close",
                    "volume",
                    "n_trades",
                    "is_final",
                    "provider",
                    "received_ts",
                ),
            )
        )

    @staticmethod
    def _serialize_fills(values: Any) -> list[dict[str, Any]]:
        fills = []
        for fill in values:
            row = public_attributes(
                fill,
                ("timestamp", "status", "fill_price", "commission", "realized_pnl", "reason"),
            )
            if row.get("status") is not None:
                row["status"] = str(row["status"])
            row["order"] = LiveTradingManager._serialize_order(fill.order)
            fills.append(_clean(row))
        return fills

    @staticmethod
    def _serialize_order(order: Any) -> dict[str, Any]:
        """Convert a native order and its enum type into journal-safe values."""
        row = public_attributes(
            order,
            ("id", "symbol", "order_type", "quantity", "price", "limit_price"),
        )
        if row.get("order_type") is not None:
            row["order_type"] = str(row["order_type"])
        return row

    @staticmethod
    def _aggregate_snapshots(snapshots: dict[str, dict[str, Any]]) -> dict[str, Any]:
        if not snapshots:
            return {}
        if len(snapshots) == 1:
            return dict(next(iter(snapshots.values())))
        values = list(snapshots.values())
        cash: dict[str, float] = {}
        positions: dict[str, float] = {}
        prices: dict[str, float] = {}
        for snapshot in values:
            prices.update(snapshot.get("latest_prices") or {})
            for currency, amount in (snapshot.get("portfolio", {}).get("cash") or {}).items():
                cash[currency] = cash.get(currency, 0.0) + float(amount)
            for symbol, amount in (snapshot.get("portfolio", {}).get("positions") or {}).items():
                positions[symbol] = positions.get(symbol, 0.0) + float(amount)
        equity = sum(float(value.get("equity") or 0.0) for value in values)
        gross = sum(float(value.get("gross_exposure") or 0.0) for value in values)
        return {
            "latest_prices": prices,
            "equity": equity,
            "realized_pnl": sum(float(value.get("realized_pnl") or 0.0) for value in values),
            "unrealized_pnl": sum(float(value.get("unrealized_pnl") or 0.0) for value in values),
            "processed_bars": max(int(value.get("processed_bars") or 0) for value in values),
            "gross_exposure": gross,
            "net_exposure": sum(float(value.get("net_exposure") or 0.0) for value in values),
            "leverage": gross / equity if equity > 0.0 else 0.0,
            "buying_power": sum(float(value.get("buying_power") or 0.0) for value in values),
            "drawdown": min(float(value.get("drawdown") or 0.0) for value in values),
            "peak_equity": sum(float(value.get("peak_equity") or 0.0) for value in values),
            "total_costs": sum(float(value.get("total_costs") or 0.0) for value in values),
            "trading_halted": any(bool(value.get("trading_halted")) for value in values),
            "halt_reason": "; ".join(
                str(value["halt_reason"]) for value in values if value.get("halt_reason")
            )
            or None,
            "metrics": {},
            "portfolio": {"cash": cash, "positions": positions, "orders": []},
        }

    def _storage_root(self) -> Path:
        if self._configured_storage_root is not None:
            return self._configured_storage_root
        from backtide.config import get_config

        return Path(get_config().data.storage_path) / "paper_sessions"

    def _persist_manifest(self, status: str) -> None:
        if not self._session_id:
            return
        folder = self._storage_root() / self._session_id
        folder.mkdir(parents=True, exist_ok=True)
        value = {
            "id": self._session_id,
            "status": status,
            "started_at": self._started_at,
            "finished_at": self._now() if status in {"stopped", "error"} else None,
            "config": self._config,
            "snapshot": self.status().get("snapshot") if self._session else {},
            "health": {
                "last_message_at": self._last_message_at,
                "received_events": self._received_events,
                "warmup_bars_loaded": self._warmup_loaded,
                "replay": self._replay_status() if self._config.get("mode") == "replay" else None,
            },
            "error": self._error,
        }
        (folder / "manifest.json").write_text(
            json.dumps(_clean(value), indent=2, sort_keys=True),
            encoding="utf-8",
        )

    def _append_event(self, update: dict[str, Any]) -> None:
        if not self._session_id:
            return
        folder = self._storage_root() / self._session_id
        folder.mkdir(parents=True, exist_ok=True)
        with (folder / "events.jsonl").open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(_clean(update), separators=(",", ":")) + "\n")

    def _persist_warmup(self, markets: list[Any]) -> None:
        """Persist the exact warm-up stream used to initialize a live session."""
        if not self._session_id:
            return
        folder = self._storage_root() / self._session_id
        folder.mkdir(parents=True, exist_ok=True)
        with (folder / "warmup.jsonl").open("w", encoding="utf-8") as stream:
            for market in markets:
                stream.write(
                    json.dumps(self._serialize_market(market), separators=(",", ":")) + "\n"
                )

    @staticmethod
    def _read_json_lines(path: Path) -> list[dict[str, Any]]:
        """Read valid JSON objects from a persisted line-delimited journal."""
        if not path.is_file():
            return []
        values = []
        for line in path.read_text(encoding="utf-8").splitlines():
            try:
                value = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(value, dict):
                values.append(value)
        return values

    @staticmethod
    def _now() -> str:
        return datetime.now(UTC).isoformat()
