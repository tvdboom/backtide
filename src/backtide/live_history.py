"""Backtide.

Author: Mavs
Description: Shared serialization and DuckDB persistence for live-session history.

"""

from __future__ import annotations

from datetime import UTC, datetime
import json
import math
from typing import Any
import uuid

from backtide.core.storage import (
    _append_live_session_event,
    _delete_live_session,
    _query_live_session,
    _query_live_session_events,
    _query_live_session_warmup,
    _query_live_sessions,
    _write_live_session,
    _write_live_session_warmup,
)


def new_session_id() -> str:
    """Return a stable live-session identifier."""
    return uuid.uuid4().hex[:16]


def utc_now() -> str:
    """Return the current UTC timestamp in the persisted ISO format."""
    return datetime.now(UTC).isoformat()


def clean(value: Any) -> Any:
    """Recursively replace non-finite numeric values before JSON encoding."""
    if isinstance(value, float) and not math.isfinite(value):
        return None
    if isinstance(value, dict):
        return {str(key): clean(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [clean(item) for item in value]
    return value


def public_attributes(value: Any, names: tuple[str, ...]) -> dict[str, Any]:
    """Select named public attributes from a Rust-backed Python object."""
    return {name: clean(getattr(value, name, None)) for name in names if hasattr(value, name)}


def serialize_order(order: Any) -> dict[str, Any]:
    """Convert a native order and its enum type into journal-safe values."""
    row = public_attributes(
        order,
        ("id", "symbol", "order_type", "quantity", "price", "limit_price"),
    )
    if row.get("order_type") is not None:
        row["order_type"] = str(row["order_type"])
    return row


def serialize_fills(values: Any) -> list[dict[str, Any]]:
    """Convert native paper fills into journal-safe mappings."""
    fills = []
    for fill in values:
        row = public_attributes(
            fill,
            ("timestamp", "status", "fill_price", "commission", "realized_pnl", "reason"),
        )
        if row.get("status") is not None:
            row["status"] = str(row["status"])
        row["order"] = serialize_order(fill.order)
        fills.append(clean(row))
    return fills


def serialize_market(market: Any) -> dict[str, Any]:
    """Convert a native market update into a replayable mapping."""
    return clean(
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


def serialize_snapshot(snapshot: Any) -> dict[str, Any]:
    """Convert a native paper account snapshot into a journal-safe mapping."""
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
            serialize_order(order) for order in getattr(portfolio, "orders", [])
        ]
    else:
        output["portfolio"] = {}
    return clean(output)


def serialize_update(update: Any) -> dict[str, Any]:
    """Convert a native paper update into the persisted event shape."""
    return clean(
        {
            "market": serialize_market(update.market),
            "fills": serialize_fills(update.fills),
            "orders_submitted": update.orders_submitted,
            "processed": update.processed,
            "snapshot": serialize_snapshot(update.snapshot),
            "indicators": getattr(update, "indicators", {}),
        }
    )


def aggregate_snapshots(snapshots: dict[str, dict[str, Any]]) -> dict[str, Any]:
    """Aggregate independent strategy account snapshots for the session summary."""
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


def serialize_combined_update(market: Any, results: dict[str, Any]) -> dict[str, Any]:
    """Serialize one market event and every independent strategy result."""
    serialized = {name: serialize_update(update) for name, update in results.items()}
    snapshots = {name: value["snapshot"] for name, value in serialized.items()}
    fills = []
    for name, value in serialized.items():
        fills.extend({**fill, "strategy": name} for fill in value["fills"])
    first = next(iter(serialized.values()))
    return {
        "market": serialize_market(market),
        "fills": fills,
        "orders_submitted": sum(value["orders_submitted"] for value in serialized.values()),
        "processed": any(value["processed"] for value in serialized.values()),
        "snapshot": aggregate_snapshots(snapshots),
        "strategies": serialized,
        "indicators": first.get("indicators", {}),
    }


def write_manifest(session_id: str, value: dict[str, Any]) -> None:
    """Insert or replace one live-session manifest in DuckDB."""
    cleaned = clean(value)
    _write_live_session(
        session_id,
        str(cleaned["status"]),
        str(cleaned["started_at"]),
        cleaned.get("finished_at"),
        json.dumps(cleaned.get("config") or {}, separators=(",", ":")),
        json.dumps(cleaned.get("snapshot") or {}, separators=(",", ":")),
        json.dumps(cleaned.get("health") or {}, separators=(",", ":")),
        cleaned.get("error"),
    )


def append_event(session_id: str, update: dict[str, Any]) -> None:
    """Append one replayable live-session event in DuckDB."""
    _append_live_session_event(
        session_id,
        json.dumps(clean(update), separators=(",", ":")),
    )


def write_warmup(session_id: str, markets: list[dict[str, Any]]) -> None:
    """Replace the exact warm-up market stream stored for a live session."""
    _write_live_session_warmup(
        session_id,
        [json.dumps(clean(market), separators=(",", ":")) for market in markets],
    )


def _decode_manifest(row: tuple[Any, ...]) -> dict[str, Any]:
    """Decode one native storage row into the application manifest shape."""
    return {
        "id": row[0],
        "status": row[1],
        "started_at": row[2],
        "finished_at": row[3],
        "config": json.loads(row[4]),
        "snapshot": json.loads(row[5]),
        "health": json.loads(row[6]),
        "error": row[7],
    }


def query_manifests() -> list[dict[str, Any]]:
    """Return every persisted live-session manifest newest first."""
    return [_decode_manifest(row) for row in _query_live_sessions()]


def query_session(session_id: str) -> dict[str, Any] | None:
    """Return one manifest with its replay event and warm-up streams."""
    row = _query_live_session(session_id)
    if row is None:
        return None
    value = _decode_manifest(row)
    value["updates"] = [json.loads(event) for event in _query_live_session_events(session_id)]
    value["warmup"] = [json.loads(market) for market in _query_live_session_warmup(session_id)]
    return value


def delete_session(session_id: str) -> int:
    """Delete one live session and all related SQL rows."""
    return int(_delete_live_session(session_id))
