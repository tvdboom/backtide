"""Backtide.

Author: Mavs
Description: Unit tests for the local web application and service layer.

"""

from __future__ import annotations

from datetime import UTC, datetime
from http.client import HTTPConnection
import json
from pathlib import Path
import sys
import threading
from types import SimpleNamespace
from typing import Any

import pytest

from backtide.live_history import (
    append_event,
    new_session_id,
    utc_now,
    write_manifest,
    write_warmup,
)
from backtide.ui.live import LiveTradingManager
from backtide.ui.server import create_server
from backtide.ui.services import APIError, BacktideServices, JobStore, dataframe_records


def _persist_live_replay_source(
    config: dict[str, Any],
    *,
    events: list[dict[str, Any]] | None = None,
    warmup: list[dict[str, Any]] | None = None,
) -> str:
    """Store one complete deterministic source session for replay tests."""
    session_id = new_session_id()
    now = utc_now()
    write_manifest(
        session_id,
        {
            "id": session_id,
            "status": "stopped",
            "started_at": now,
            "finished_at": now,
            "config": config,
            "snapshot": {},
            "health": {},
            "error": None,
        },
    )
    for event in events or []:
        append_event(session_id, event)
    write_warmup(session_id, warmup or [])
    return session_id


class StubServices(BacktideServices):
    """Provide deterministic route responses without storage or provider access."""

    def bootstrap(self):
        """Return a minimal boot payload."""
        return {"defaults": {}, "enums": {}, "strategies": {}, "indicators": {}}

    def dashboard(self):
        """Return deterministic dashboard content."""
        return {"metrics": {"experiments": 2}}

    def experiments(self, search=None, limit=100, offset=0):
        """Return the experiment paging arguments for route assertions."""
        return [{"search": search, "limit": limit, "offset": offset}]

    def start_study(self, payload):
        """Return a deterministic accepted study job."""
        return {"id": "study-job", "study": payload["study"]}

    def reuse_study_setup(self, payload):
        """Return a deterministic best-candidate experiment draft."""
        return {"general": {"name": payload["study_id"]}, "strategy": {}}

    def rerun_study(self, payload):
        """Return a deterministic complete study draft."""
        return {"general": {}, "_study": {"study_id": payload["study_id"]}}

    def live_instruments(self, provider, limit=10_000):
        """Return deterministic live-provider symbols for route assertions."""
        return [{"symbol": "ADA-USD", "provider": provider, "limit": limit}]

    def instrument_overview(self, symbol, instrument_type=None, provider=None):
        """Return deterministic instrument-preview route arguments."""
        return {
            "symbol": symbol,
            "instrument_type": instrument_type,
            "provider": provider,
        }

    def update_strategy(self, original_name, payload):
        """Return a deterministic saved strategy update."""
        return {"original": original_name, "saved": payload["name"]}

    def experiment_log(self, experiment_id):
        """Return deterministic complete log content."""
        return "Momentum-study-logs.txt", f"full log for {experiment_id}\n".encode()

    def experiment_orders(self, experiment_id, strategy_id, offset=0, limit=100):
        """Return deterministic paged order metadata."""
        return {
            "experiment_id": experiment_id,
            "strategy_id": strategy_id,
            "offset": offset,
            "limit": limit,
        }

    def live_sessions(self):
        """Return deterministic persisted live sessions."""
        return [{"id": "abc123", "status": "stopped"}]

    def live_session(self, session_id):
        """Return a deterministic persisted live session."""
        return {"id": session_id, "status": "stopped"}

    def delete_live_session(self, session_id):
        """Return a deterministic live-session deletion result."""
        return {"deleted": session_id}

    def session_config_from_experiment(self, experiment_id):
        """Return a deterministic live-simulation draft."""
        return {"experiment_id": experiment_id, "provider": "kraken"}

    def replay_live(self, payload):
        """Return the replay request for route assertions."""
        return {"replayed": payload["session_id"]}

    def sizer_catalog(self):
        """Return a deterministic position-sizer catalog."""
        return {"builtin": [{"type": "FixedFractional"}], "saved": []}


@pytest.fixture
def web_server():
    """Run a local ephemeral server for one test."""
    server = create_server("127.0.0.1", 0, StubServices())
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield server
    server.shutdown()
    server.server_close()
    thread.join(timeout=2)


def request(server, method: str, path: str, body: dict | None = None):
    """Issue one JSON request to the ephemeral test server."""
    connection = HTTPConnection("127.0.0.1", server.server_port, timeout=2)
    encoded = json.dumps(body).encode() if body is not None else None
    headers = {"Content-Type": "application/json"} if body is not None else {}
    connection.request(method, path, body=encoded, headers=headers)
    response = connection.getresponse()
    data = response.read()
    connection.close()
    return response, data


class TestStaticApplication:
    """Tests for the bundled SPA HTTP surface."""

    def test_index_is_served_for_client_route(self, web_server):
        """Client-side routes return the production application shell."""
        response, body = request(web_server, "GET", "/results")

        assert response.status == 200
        assert response.getheader("Content-Type").startswith("text/html")
        assert b'<div id="app"></div>' in body

    def test_logo_is_served_as_an_image(self, web_server, monkeypatch, tmp_path):
        """Root-level frontend images are served instead of the SPA fallback."""
        logo = tmp_path / "backtide-logo.png"
        logo.write_bytes(b"\x89PNG\r\n\x1a\n")
        monkeypatch.setattr("backtide.ui.server.STATIC_ROOT", tmp_path)

        response, body = request(web_server, "GET", "/backtide-logo.png")

        assert response.status == 200
        assert response.getheader("Content-Type") == "image/png"
        assert body == b"\x89PNG\r\n\x1a\n"

    def test_provider_logo_is_served_as_an_image(self, web_server, monkeypatch, tmp_path):
        """Bundled provider artwork is served instead of the SPA fallback."""
        providers = tmp_path / "providers"
        providers.mkdir()
        logo = providers / "yahoo.png"
        logo.write_bytes(b"\x89PNG\r\n\x1a\n")
        monkeypatch.setattr("backtide.ui.server.STATIC_ROOT", tmp_path)

        response, body = request(web_server, "GET", "/providers/yahoo.png")

        assert response.status == 200
        assert response.getheader("Content-Type") == "image/png"
        assert body == b"\x89PNG\r\n\x1a\n"

    def test_asset_traversal_is_rejected(self, web_server):
        """Static asset requests cannot escape the bundled directory."""
        response, body = request(web_server, "GET", "/assets/../../pyproject.toml")

        assert response.status == 404
        assert json.loads(body)["error"] == "Asset not found."


class TestJSONRoutes:
    """Tests for JSON response and routing behavior."""

    def test_health_response(self, web_server):
        """The health endpoint returns a no-store JSON response."""
        response, body = request(web_server, "GET", "/api/health")

        assert response.status == 200
        assert response.getheader("Cache-Control") == "no-store"
        assert json.loads(body) == {"status": "ok"}

    def test_live_instruments_route_forwards_provider_and_limit(self, web_server):
        """The live catalog route selects the requested exchange catalog."""
        response, body = request(
            web_server,
            "GET",
            "/api/live/instruments?provider=coinbase&limit=4321",
        )

        assert response.status == 200
        assert json.loads(body) == [{"symbol": "ADA-USD", "provider": "coinbase", "limit": 4321}]

    def test_instrument_overview_route_forwards_catalog_context(self, web_server):
        """The instrument preview route keeps the symbol, type, and provider context."""
        response, body = request(
            web_server,
            "GET",
            "/api/instrument-overview?symbol=BTC-USD&instrument_type=crypto&provider=kraken",
        )

        assert response.status == 200
        assert json.loads(body) == {
            "symbol": "BTC-USD",
            "instrument_type": "crypto",
            "provider": "kraken",
        }

    def test_unknown_command_returns_not_found(self, web_server):
        """An unknown API command returns a structured 404 error."""
        response, body = request(web_server, "POST", "/api/missing", {})

        assert response.status == 404
        assert json.loads(body) == {"error": "Endpoint not found."}

    def test_non_object_body_is_rejected(self, web_server):
        """Command bodies must be JSON objects."""
        connection = HTTPConnection("127.0.0.1", web_server.server_port, timeout=2)
        connection.request(
            "POST",
            "/api/downloads",
            body=b"[]",
            headers={"Content-Type": "application/json"},
        )
        response = connection.getresponse()
        body = response.read()

        assert response.status == 400
        assert json.loads(body)["error"] == "Request body must be a JSON object."

    def test_strategy_update_decodes_the_saved_name(self, web_server):
        """Saved strategy updates route the decoded original name and JSON body."""
        response, body = request(
            web_server,
            "PUT",
            "/api/strategies/Old%20name",
            {"name": "New name"},
        )

        assert response.status == 200
        assert json.loads(body) == {"original": "Old name", "saved": "New name"}

    def test_experiment_log_is_served_as_a_text_download(self, web_server):
        """The log route downloads complete text with the original filename pattern."""
        response, body = request(web_server, "GET", "/api/experiments/exp-1/logs")

        assert response.status == 200
        assert response.getheader("Content-Type") == "text/plain; charset=utf-8"
        assert response.getheader("Content-Disposition") == (
            'attachment; filename="Momentum-study-logs.txt"'
        )
        assert body == b"full log for exp-1\n"

    def test_experiment_orders_route_forwards_lazy_loading_parameters(self, web_server):
        """The order route forwards the strategy, offset, and bounded batch size."""
        response, body = request(
            web_server,
            "GET",
            "/api/experiments/exp-1/orders?strategy_id=run-2&offset=100&limit=100",
        )

        assert response.status == 200
        assert json.loads(body) == {
            "experiment_id": "exp-1",
            "strategy_id": "run-2",
            "offset": 100,
            "limit": 100,
        }

    def test_experiments_route_forwards_lazy_loading_parameters(self, web_server):
        """The experiment summary route forwards search, offset, and batch size."""
        response, body = request(
            web_server,
            "GET",
            "/api/experiments?search=momentum&offset=10&limit=10",
        )

        assert response.status == 200
        assert json.loads(body) == [{"search": "momentum", "limit": 10, "offset": 10}]

    def test_study_route_starts_an_accepted_background_job(self, web_server):
        """Studies have a dedicated asynchronous command endpoint."""
        response, body = request(
            web_server,
            "POST",
            "/api/studies",
            {
                "config": {"general": {"name": "SMA parameter study"}},
                "study": {"parameter_space": {"period": [10, 20]}},
            },
        )

        assert response.status == 202
        assert json.loads(body) == {
            "id": "study-job",
            "study": {"parameter_space": {"period": [10, 20]}},
        }

    def test_study_reuse_route_returns_a_normal_experiment_draft(self, web_server):
        """The best candidate can be promoted through its dedicated command endpoint."""
        response, body = request(
            web_server,
            "POST",
            "/api/studies/reuse",
            {"study_id": "experiment-1"},
        )

        assert response.status == 200
        assert json.loads(body) == {
            "general": {"name": "experiment-1"},
            "strategy": {},
        }

    def test_study_rerun_route_returns_the_complete_study_draft(self, web_server):
        """Saved study settings have a dedicated builder-draft endpoint."""
        response, body = request(
            web_server,
            "POST",
            "/api/studies/rerun",
            {"study_id": "experiment-1"},
        )

        assert response.status == 200
        assert json.loads(body) == {
            "general": {},
            "_study": {"study_id": "experiment-1"},
        }

    def test_live_history_and_replay_routes(self, web_server):
        """Live session history routes expose persisted sessions and replay control."""
        response, body = request(web_server, "GET", "/api/live/sessions")
        assert response.status == 200
        assert json.loads(body) == [{"id": "abc123", "status": "stopped"}]

        response, body = request(web_server, "GET", "/api/live/sessions/abc123")
        assert response.status == 200
        assert json.loads(body) == {"id": "abc123", "status": "stopped"}

        response, body = request(
            web_server,
            "POST",
            "/api/live/replay",
            {"session_id": "abc123"},
        )
        assert response.status == 200
        assert json.loads(body) == {"replayed": "abc123"}

        response, body = request(web_server, "DELETE", "/api/live/sessions/abc123")
        assert response.status == 200
        assert json.loads(body) == {"deleted": "abc123"}

    def test_experiment_session_config_route(self, web_server):
        """Experiment promotion returns a live wizard draft."""
        response, body = request(web_server, "GET", "/api/experiments/experiment-1/session-config")

        assert response.status == 200
        assert json.loads(body) == {"experiment_id": "experiment-1", "provider": "kraken"}

    def test_sizer_library_route(self, web_server):
        """Position sizers have a first-class Library collection endpoint."""
        response, body = request(web_server, "GET", "/api/sizers")

        assert response.status == 200
        assert json.loads(body) == {"builtin": [{"type": "FixedFractional"}], "saved": []}


class TestJobStore:
    """Tests for bounded background work snapshots."""

    def test_successful_job_exposes_result(self):
        """Completed work transitions to success and keeps its result."""
        jobs = JobStore()

        def work(progress):
            progress(4, 10)
            return {"value": 42}

        job = jobs.start(
            "test",
            work,
            name="Momentum research",
            progress_unit="items",
        )

        for _ in range(1000):
            snapshot = jobs.get(job["id"])
            if snapshot["status"] == "success":
                break

        assert snapshot["result"] == {"value": 42}
        assert snapshot["name"] == "Momentum research"
        assert snapshot["progress_completed"] == 4
        assert snapshot["progress_total"] == 10
        assert snapshot["progress_unit"] == "items"
        assert snapshot["progress_started_at"]

    def test_failed_job_exposes_safe_error(self):
        """A worker exception is captured without losing the job."""
        jobs = JobStore()

        def fail(_progress):
            raise ValueError("invalid work")

        job = jobs.start("test", fail)
        for _ in range(1000):
            snapshot = jobs.get(job["id"])
            if snapshot["status"] == "error":
                break

        assert snapshot["error"] == "invalid work"

    def test_unknown_job_raises_not_found(self):
        """Missing job identifiers produce an API 404."""
        with pytest.raises(APIError) as exc_info:
            JobStore().get("missing")

        assert exc_info.value.status == 404


class TestSerialization:
    """Tests for dataframe-independent response normalization."""

    def test_records_replace_non_finite_values(self):
        """NaN and infinite numbers become JSON null values."""
        rows = dataframe_records([{"nan": float("nan"), "inf": float("inf")}])

        assert rows == [{"nan": None, "inf": None}]

    def test_currency_options_use_rust_enum_metadata(self):
        """Currency choices include every Rust value with its name and country flag."""
        from backtide.data import Currency

        options = BacktideServices._currency_options(Currency)
        usd = next(option for option in options if option["code"] == "USD")

        assert {option["code"] for option in options} == {
            str(currency) for currency in Currency.variants()
        }
        assert usd == {
            "code": "USD",
            "name": "United States Dollar",
            "flag": "🇺🇸",
            "country_code": "us",
            "decimals": 2,
        }

    @pytest.mark.parametrize(
        ("value", "end", "expected"),
        [
            ("2024-01-01", False, 1_704_067_200),
            ("2024-01-01", True, 1_704_153_599),
            (1_704_067_200, False, 1_704_067_200),
            (None, False, None),
        ],
    )
    def test_download_date_boundaries_are_utc_seconds(self, value, end, expected):
        """Date controls convert to inclusive UTC Unix boundaries."""
        assert BacktideServices._date_boundary(value, end=end) == expected

    def test_constructor_parameters_include_defaults_and_kinds(self):
        """Built-in constructor controls receive serializable typed metadata."""

        class Example:
            def __init__(self, period: int = 14, name: str = "RSI", *, enabled: bool = True):
                pass

        result = BacktideServices._constructor_parameters(Example)

        assert result == [
            {
                "name": "period",
                "label": "Period",
                "kind": "number",
                "default": 14,
                "required": False,
            },
            {
                "name": "name",
                "label": "Name",
                "kind": "text",
                "default": "RSI",
                "required": False,
            },
            {
                "name": "enabled",
                "label": "Enabled",
                "kind": "boolean",
                "default": True,
                "required": False,
            },
        ]

    def test_required_numeric_constructor_parameters_keep_number_controls(self):
        """Stringified PyO3 annotations still produce numeric library inputs."""

        class Example:
            def __init__(self, amount: float):
                pass

        assert BacktideServices._constructor_parameters(Example) == [
            {
                "name": "amount",
                "label": "Amount",
                "kind": "number",
                "default": None,
                "required": True,
            }
        ]

    def test_constructor_values_return_the_saved_builtin_configuration(self):
        """Saved built-ins expose the values needed to prefill every editor control."""

        class Example:
            def __init__(self, period: int = 14, scale: float = 1.0):
                self.period = period
                self.scale = scale

            def __reduce__(self):
                return type(self), (self.period, self.scale)

        assert BacktideServices._constructor_values(Example(21, 2.5)) == {
            "period": 21,
            "scale": 2.5,
        }

    def test_constructor_values_match_all_builtin_defaults(self):
        """Every Rust built-in can prefill the editor through its persisted arguments."""
        from backtide.indicators import BUILTIN_INDICATORS
        from backtide.strategies import BUILTIN_STRATEGIES

        for cls in [*BUILTIN_STRATEGIES, *BUILTIN_INDICATORS]:
            expected = {
                parameter["name"]: parameter["default"]
                for parameter in BacktideServices._constructor_parameters(cls)
            }

            assert BacktideServices._constructor_values(cls()) == expected

    def test_catalog_description_falls_back_to_custom_class_docstring(self):
        """Custom library objects do not need a built-in description method."""

        class CustomStrategy:
            """Trade a deterministic custom signal."""

        assert (
            BacktideServices._catalog_description(CustomStrategy())
            == "Trade a deterministic custom signal."
        )

    def test_library_names_are_limited_to_twenty_characters(self):
        """Reusable asset names stay compact enough for every library editor."""
        name = "Twenty character nam"
        assert BacktideServices._safe_library_name(name) == name

        with pytest.raises(APIError, match="20 characters or fewer"):
            BacktideServices._safe_library_name("Twenty-one characters")

    @pytest.mark.parametrize(
        "class_name",
        ["MomentumStrategy", "MomentumIndicator", "MomentumSizer", "MomentumMetric"],
    )
    def test_empty_library_names_use_the_custom_python_class(self, class_name):
        """Custom Python assets fall back to the instantiated class name."""
        instance = type(class_name, (), {})()

        assert (
            BacktideServices._library_asset_name("", instance, ignored_class_prefix="My")
            == class_name
        )

    @pytest.mark.parametrize(
        "placeholder",
        ["MyStrategy", "MyMomentumIndicator", "MyPositionSizer", "myCustomMetric"],
    )
    def test_starter_class_names_are_not_used_as_library_names(self, placeholder):
        """Untouched starter class names do not silently become saved display names."""
        instance = type(placeholder, (), {})()

        with pytest.raises(APIError, match="rename the placeholder Python class"):
            BacktideServices._library_asset_name("", instance, ignored_class_prefix="My")


class TestServiceCommands:
    """Tests for command validation and backend dispatch."""

    def test_instrument_overview_fetches_a_non_persisted_daily_preview(self, monkeypatch):
        """Instrument previews return direct-provider closes and canonical exchange details."""
        import backtide.data

        class ExchangeValue(str):
            @property
            def mic(self) -> str:
                """Return the test MIC."""
                return "XNAS"

            @property
            def name(self) -> str:
                """Return the test exchange name."""
                return "NASDAQ Global Select Market"

            @property
            def country(self) -> SimpleNamespace:
                """Return the test exchange country."""
                return SimpleNamespace(alpha2="US")

        class CountryValue:
            alpha2 = "US"

        class CurrencyValue(str):
            country = CountryValue()

        captured = {}

        def fetch_bar_preview(symbol, instrument_type, provider, *, limit):
            captured.update(
                symbol=symbol,
                instrument_type=instrument_type,
                provider=provider,
                limit=limit,
            )
            instrument = SimpleNamespace(
                symbol="AAPL",
                name="Apple Inc.",
                base=None,
                quote=CurrencyValue("USD"),
                instrument_type="stocks",
                exchange=ExchangeValue("XNAS"),
                provider="yahoo",
            )
            bars = [SimpleNamespace(open_ts=index, adj_close=float(index)) for index in range(30)]
            return instrument, bars

        monkeypatch.setattr(
            backtide.data,
            "fetch_bar_preview",
            fetch_bar_preview,
            raising=False,
        )

        result = BacktideServices().instrument_overview(" AAPL ", "Stocks", "Yahoo")

        assert result == {
            "symbol": "AAPL",
            "name": "Apple Inc.",
            "base": None,
            "quote": "USD",
            "instrument_type": "stocks",
            "exchange": "XNAS",
            "provider": "yahoo",
            "exchange_mic": "XNAS",
            "exchange_name": "NASDAQ Global Select Market",
            "market_country_code": "us",
            "currency_country_code": "us",
            "interval": "1d",
            "sparkline": [float(index) for index in range(30)],
            "sparkline_ts": list(range(30)),
        }
        assert captured == {
            "symbol": "AAPL",
            "instrument_type": "Stocks",
            "provider": "Yahoo",
            "limit": 30,
        }

    def test_reuse_study_setup_promotes_only_the_best_candidate(
        self,
        monkeypatch,
        tmp_path: Path,
    ):
        """Best-candidate reuse saves its exact constructor and returns a normal draft."""
        from backtide import config as config_module
        from backtide import storage as storage_module
        from backtide.backtest.study import WalkForwardConfig
        from backtide.strategies import utils as strategy_utils

        class Strategy:
            def __init__(self, fast=10, slow=100):
                self.fast = fast
                self.slow = slow

        experiment_root = tmp_path / "experiments" / "experiment-1"
        experiment_root.mkdir(parents=True)
        (experiment_root / "config.toml").write_text(
            '[general]\nname = "Study"\n\n[strategy]\nstrategies = ["Original"]\n',
            encoding="utf-8",
        )
        cfg = SimpleNamespace(data=SimpleNamespace(storage_path=str(tmp_path)))
        best = SimpleNamespace(
            strategy_name="C002 · fast=20, slow=200",
            parameters={"fast": 20, "slow": 200},
        )
        study = SimpleNamespace(
            name="Study",
            strategy_name="Original",
            best_candidate=best,
            parameter_space={"fast": [10, 20], "slow": [100, 200]},
            min_trades=12,
            max_drawdown=0.25,
            walk_forward=WalkForwardConfig(
                training_days=730,
                test_days=180,
                step_days=90,
                anchored=True,
            ),
            folds=[],
        )
        saved = {}
        monkeypatch.setattr(config_module, "get_config", lambda: cfg)
        monkeypatch.setattr(
            storage_module,
            "query_study",
            lambda _experiment_id: study,
        )
        monkeypatch.setattr(
            strategy_utils,
            "_load_stored_strategies",
            lambda _cfg: {"Original": Strategy()},
        )
        monkeypatch.setattr(
            strategy_utils,
            "_save_strategy",
            lambda value, name, _cfg: saved.update(value=value, name=name),
        )

        result = BacktideServices().reuse_study_setup({"study_id": "experiment-1"})

        assert result["general"]["name"] == "Study · C002"
        assert result["strategy"]["strategies"] == ["Original · C002"]
        assert saved["name"] == "Original · C002"
        assert (saved["value"].fast, saved["value"].slow) == (20, 200)

        rerun = BacktideServices().rerun_study({"study_id": "experiment-1"})

        assert rerun["strategy"]["strategies"] == ["Original"]
        assert rerun["_study"] == {
            "parameter_space": {"fast": [10, 20], "slow": [100, 200]},
            "min_trades": 12,
            "max_drawdown": 0.25,
            "walk_forward": {
                "training_days": 730,
                "test_days": 180,
                "step_days": 90,
                "anchored": True,
            },
        }

    @pytest.mark.parametrize(
        (
            "method_name",
            "domain_module",
            "utils_module",
            "builtins_name",
            "build_name",
            "check_name",
            "save_name",
            "class_name",
        ),
        [
            (
                "save_strategy",
                "backtide.strategies",
                "backtide.strategies.utils",
                "BUILTIN_STRATEGIES",
                "_build_custom_strategy",
                "_check_strategy_code",
                "_save_strategy",
                "MomentumStrategy",
            ),
            (
                "save_indicator",
                "backtide.indicators",
                "backtide.indicators.utils",
                "BUILTIN_INDICATORS",
                "_build_custom_indicator",
                "_check_indicator_code",
                "_save_indicator",
                "MomentumIndicator",
            ),
            (
                "save_sizer",
                "backtide.sizers",
                "backtide.sizers.utils",
                "BUILTIN_SIZERS",
                "_build_custom_sizer",
                "_check_sizer_code",
                "_save_sizer",
                "MomentumSizer",
            ),
            (
                "save_metric",
                "backtide.metrics",
                "backtide.metrics.utils",
                "BUILTIN_METRICS",
                "_build_custom_metric",
                "_check_metric_code",
                "_save_metric",
                "MomentumMetric",
            ),
        ],
    )
    def test_custom_assets_save_under_the_python_class_when_name_is_empty(
        self,
        monkeypatch,
        method_name,
        domain_module,
        utils_module,
        builtins_name,
        build_name,
        check_name,
        save_name,
        class_name,
    ):
        """Custom library assets share class-name fallback behavior."""
        cfg = SimpleNamespace(data=SimpleNamespace(storage_path="unused"))
        instance = type(class_name, (), {})()
        captured = {}

        def save(value, name, received_cfg):
            captured.update(value=value, name=name, cfg=received_cfg)

        config_module = __import__("backtide.config", fromlist=["get_config"])
        domain = __import__(domain_module, fromlist=[builtins_name])
        utilities = __import__(utils_module, fromlist=[build_name, check_name, save_name])
        monkeypatch.setattr(config_module, "get_config", lambda: cfg)
        monkeypatch.setattr(domain, builtins_name, [])
        monkeypatch.setattr(utilities, build_name, lambda _code: instance)
        monkeypatch.setattr(utilities, check_name, lambda *_args: None)
        monkeypatch.setattr(utilities, save_name, save)

        result = getattr(BacktideServices(), method_name)(
            {"kind": "custom", "name": "", "code": "custom source"}
        )

        assert result == {"saved": class_name}
        assert captured == {"value": instance, "name": class_name, "cfg": cfg}

    def test_live_instruments_uses_provider_specific_catalog(self, monkeypatch):
        """Live session receives canonical symbols from the selected provider."""
        import backtide.live

        captured = {}

        def list_live_instruments(provider, limit):
            captured.update(provider=provider, limit=limit)
            return [
                SimpleNamespace(
                    symbol="ADA-USD",
                    name="Cardano",
                    base="ADA",
                    quote="USD",
                    instrument_type="Crypto",
                    exchange=None,
                    provider="Kraken",
                )
            ]

        monkeypatch.setattr(backtide.live, "list_live_instruments", list_live_instruments)

        result = BacktideServices().live_instruments("kraken", limit=50_000)

        assert captured == {"provider": "kraken", "limit": 10_000}
        assert result[0]["symbol"] == "ADA-USD"
        assert result[0]["name"] == "Cardano"

    def test_bootstrap_uses_configured_base_currency_for_experiments(self, monkeypatch):
        """New experiment defaults inherit the application's configured currency."""
        import backtide.config

        cfg = SimpleNamespace(
            general=SimpleNamespace(base_currency="EUR"),
            data=SimpleNamespace(dataframe_library=SimpleNamespace(class_name="DataFrame")),
            display=SimpleNamespace(
                currency_prefix=False,
                logokit_api_key=None,
                timezone="UTC",
                date_format="YYYY-MM-DD",
                datetime_format=lambda: "YYYY-MM-DD HH:mm",
            ),
        )
        monkeypatch.setattr(backtide.config, "get_config", lambda: cfg)
        services = BacktideServices()
        monkeypatch.setattr(services, "strategy_catalog", lambda: {"saved": []})
        monkeypatch.setattr(services, "indicator_catalog", lambda: {"saved": []})
        monkeypatch.setattr(services, "metric_catalog", lambda: {"builtin": [], "saved": []})
        monkeypatch.setattr(services, "sizer_catalog", lambda: {"builtin": [], "saved": []})
        monkeypatch.setattr(services, "live_capabilities", lambda: {"providers": {}})

        result = services.bootstrap()

        assert result["defaults"]["portfolio"]["base_currency"] == "EUR"
        assert result["display"]["currency_prefix"] is False

    def test_sizer_catalog_describes_required_numeric_parameters(self, monkeypatch, tmp_path):
        """Built-in sizers are cataloged without constructing missing required arguments."""
        import backtide.config

        monkeypatch.setattr(
            backtide.config,
            "get_config",
            lambda: SimpleNamespace(data=SimpleNamespace(storage_path=tmp_path)),
        )

        catalog = BacktideServices().sizer_catalog()
        fixed = next(item for item in catalog["builtin"] if item["type"] == "FixedNotional")

        assert len(catalog["builtin"]) == 7
        assert fixed["name"] == "Fixed Notional"
        assert fixed["parameters"] == [
            {
                "name": "amount",
                "label": "Amount",
                "kind": "number",
                "default": 1_000.0,
                "required": False,
            }
        ]

        fractional = next(item for item in catalog["builtin"] if item["type"] == "FixedFractional")
        assert fractional["name"] == "Fixed Fractional"
        assert fractional["parameters"][0]["default"] == 0.1

    def test_save_sizer_persists_and_reloads_a_builtin(self, monkeypatch, tmp_path):
        """A built-in sizer preset survives the complete library persistence path."""
        import cloudpickle

        import backtide.config

        monkeypatch.setattr(
            backtide.config,
            "get_config",
            lambda: SimpleNamespace(data=SimpleNamespace(storage_path=tmp_path)),
        )
        services = BacktideServices()

        result = services.save_sizer(
            {
                "kind": "builtin",
                "name": "Ten percent",
                "type": "FixedFractional",
                "params": {"fraction": 0.1},
            }
        )
        saved = services.sizer_catalog()["saved"]
        with (tmp_path / "sizers" / "Ten percent.pkl").open("rb") as stream:
            stored = cloudpickle.load(stream)

        assert result == {"saved": "Ten percent"}
        assert stored == {
            "format": "backtide.builtin-sizer.v1",
            "type": "FixedFractional",
            "parameters": {"fraction": 0.1},
        }
        assert saved == [
            {
                "name": "Ten percent",
                "type": "FixedFractional",
                "builtin": True,
                "description": "Allocate a fixed percentage of total current equity.",
                "source": None,
                "params": {"fraction": 0.1},
            }
        ]

    def test_sizer_loader_removes_empty_failed_saves(self, tmp_path):
        """An empty artifact from an interrupted save is cleaned without deserialization."""
        from backtide.config import Config, DataConfig
        from backtide.sizers.utils import _load_stored_sizers

        folder = tmp_path / "sizers"
        folder.mkdir()
        failed = folder / "Equal weights.pkl"
        failed.touch()
        cfg = Config(data=DataConfig(storage_path=str(tmp_path)))

        assert _load_stored_sizers(cfg) == {}
        assert not failed.exists()

    def test_sizer_save_failure_preserves_the_previous_file(self, monkeypatch, tmp_path):
        """Atomic sizer writes never truncate an existing reusable preset."""
        from backtide.config import Config, DataConfig
        from backtide.sizers import FixedFractional, utils

        folder = tmp_path / "sizers"
        folder.mkdir()
        target = folder / "Allocation.pkl"
        target.write_bytes(b"previous")
        cfg = Config(data=DataConfig(storage_path=str(tmp_path)))

        def fail_dump(_value, _stream):
            raise TypeError("simulated pickle failure")

        monkeypatch.setattr("backtide.utils.library.cloudpickle.dump", fail_dump)

        with pytest.raises(TypeError, match="simulated pickle failure"):
            utils._save_sizer(FixedFractional(0.1), "Allocation", cfg)

        assert target.read_bytes() == b"previous"
        assert list(folder.glob("*.tmp")) == []

    def test_experiment_configuration_prefills_live_trading(self, monkeypatch, tmp_path):
        """Shared research settings and the first non-benchmark strategy are promoted."""
        from backtide.backtest import ExperimentConfig
        import backtide.config

        experiment = ExperimentConfig.from_dict(
            {
                "data": {"symbols": ["BTC-USD"], "interval": "FiveMinutes"},
                "portfolio": {"initial_cash": 25_000, "base_currency": "EUR"},
                "strategy": {
                    "benchmark": "Benchmark strategy",
                    "strategies": ["Benchmark strategy", "Momentum", "Mean reversion"],
                },
                "indicators": {"indicators": ["Fast SMA"]},
                "metrics": ["total_return", "sharpe", "alpha"],
                "engine": {"warmup_period": 120, "risk_free_rate": 2.5},
                "exchange": {
                    "commission_type": "PercentagePlusFixed",
                    "commission_pct": 0.25,
                    "commission_fixed": 1.5,
                    "slippage": 0.15,
                    "allowed_order_types": ["Market", "Limit"],
                    "partial_fills": True,
                    "allow_margin": True,
                    "max_leverage": 3.0,
                    "initial_margin": 40.0,
                    "maintenance_margin": 20.0,
                    "margin_interest": 4.0,
                    "allow_short_selling": True,
                    "borrow_rate": 2.0,
                    "max_position_size": 35,
                },
            }
        )
        folder = tmp_path / "experiments" / "experiment-1"
        folder.mkdir(parents=True)
        (folder / "config.toml").write_text(experiment.to_toml(), encoding="utf-8")
        monkeypatch.setattr(
            backtide.config,
            "get_config",
            lambda: SimpleNamespace(data=SimpleNamespace(storage_path=tmp_path)),
        )

        result = BacktideServices().session_config_from_experiment("experiment-1")

        assert result["provider"] == "kraken"
        assert result["interval"] == "5m"
        assert result["symbols"] == ["BTC-USD"]
        assert result["strategies"] == ["Momentum"]
        assert result["indicators"] == ["Fast SMA"]
        assert result["warmup_bars"] == 120
        assert result["config"] == {
            "initial_cash": 25_000,
            "base_currency": "EUR",
            "commission_pct": 0.25,
            "commission_fixed": 1.5,
            "slippage": 0.15,
            "allow_short": True,
            "allow_margin": True,
            "max_leverage": 3.0,
            "initial_margin": 40.0,
            "maintenance_margin": 20.0,
            "margin_interest": 4.0,
            "borrow_rate": 2.0,
            "max_position_size": 35,
            "allowed_order_types": ["Market", "Limit"],
            "partial_fills": True,
            "metrics": ["total_return", "sharpe"],
            "risk_free_rate": 2.5,
        }

    def test_fixed_experiment_commission_disables_live_percentage_fee(self, monkeypatch, tmp_path):
        """Fixed-only experiments do not acquire an extra percentage fee in live trading."""
        from backtide.backtest import ExperimentConfig
        import backtide.config

        experiment = ExperimentConfig.from_dict(
            {
                "metrics": ["sharpe"],
                "exchange": {
                    "commission_type": "Fixed",
                    "commission_pct": 0.25,
                    "commission_fixed": 1.5,
                },
            }
        )
        folder = tmp_path / "experiments" / "experiment-1"
        folder.mkdir(parents=True)
        (folder / "config.toml").write_text(experiment.to_toml(), encoding="utf-8")
        monkeypatch.setattr(
            backtide.config,
            "get_config",
            lambda: SimpleNamespace(data=SimpleNamespace(storage_path=tmp_path)),
        )

        result = BacktideServices().session_config_from_experiment("experiment-1")

        assert result["config"]["commission_pct"] == 0.0
        assert result["config"]["commission_fixed"] == 1.5

    def test_download_converts_browser_dates_before_dispatch(self, monkeypatch):
        """Download jobs pass inclusive UTC seconds to the data API."""
        from backtide import data

        captured = {}

        def resolve_profiles(*_args, **_kwargs):
            return ["profile"]

        monkeypatch.setattr(data, "resolve_profiles", resolve_profiles)

        def download_bars(profiles, start, end, *, verbose):
            captured.update(profiles=profiles, start=start, end=end, verbose=verbose)
            return SimpleNamespace(n_succeeded=1, n_failed=0, warnings=[])

        monkeypatch.setattr(data, "download_bars", download_bars)
        services = BacktideServices()

        job = services.start_download(
            {
                "symbols": ["AAPL"],
                "instrument_type": "stocks",
                "intervals": ["1d"],
                "start": "2024-01-01",
                "end": "2024-01-01",
            }
        )
        for _ in range(1000):
            if services.jobs.get(job["id"])["status"] == "success":
                break

        assert captured == {
            "profiles": ["profile"],
            "start": 1_704_067_200,
            "end": 1_704_153_599,
            "verbose": False,
        }

    def test_download_plan_reports_ranges_and_estimates(self, monkeypatch, sample_instrument):
        """Download planning exposes provider dates and legacy row estimates."""
        from backtide import data
        from backtide.data import InstrumentProfile, Interval

        interval = Interval("1d")
        profile = InstrumentProfile(
            instrument=sample_instrument,
            earliest_ts={interval: int(datetime(2024, 1, 1, 12, tzinfo=UTC).timestamp())},
            latest_ts={interval: int(datetime(2024, 1, 31, 12, tzinfo=UTC).timestamp())},
            legs=[],
        )
        monkeypatch.setattr(data, "resolve_profiles", lambda *_args, **_kwargs: [profile])

        plan = BacktideServices().download_plan(
            {
                "symbols": ["AAPL"],
                "instrument_type": "stocks",
                "intervals": ["1d"],
                "full_history": True,
            }
        )

        assert plan["available_start"] == "2024-01-01"
        assert plan["available_end"] == "2024-01-31"
        assert plan["summary"] == {
            "estimated_bars": 21,
            "estimated_seconds": 21 / 40_000,
            "estimated_bytes": 2_520,
            "series": 1,
        }
        assert plan["profiles"][0]["market_country_code"] == "us"
        assert plan["profiles"][0]["currency_country_code"] == "us"
        assert plan["profiles"][0]["intervals"][0] == {
            "interval": "1d",
            "available_start": "2024-01-01",
            "available_end": "2024-01-31",
            "download_start": "2024-01-01",
            "download_end": "2024-01-31",
            "days": 31,
            "estimated_bars": 21,
        }

    def test_download_plan_hides_provider_runtime_details(self, monkeypatch):
        """Provider resolution failures return a safe planning error."""
        from backtide import data

        def fail(*_args, **_kwargs):
            raise RuntimeError(r"provider failed at C:\private\database.duckdb")

        monkeypatch.setattr(data, "resolve_profiles", fail)

        with pytest.raises(
            APIError,
            match=r"Provider availability could not be resolved for this selection\.",
        ) as exc_info:
            BacktideServices().download_plan(
                {
                    "symbols": ["AAPL"],
                    "instrument_type": "stocks",
                    "intervals": ["1d"],
                }
            )

        assert exc_info.value.status == 422

    @pytest.mark.parametrize(
        ("suffix", "text", "parsed"),
        [
            (".json", '{"general":{"name":"json"}}', {"general": {"name": "json"}}),
            (".yaml", "general:\n  name: yaml", {"general": {"name": "yaml"}}),
        ],
    )
    def test_uploaded_config_formats_are_normalized(self, monkeypatch, suffix, text, parsed):
        """JSON and YAML uploads pass through ExperimentConfig normalization."""
        captured = {}

        class Config:
            @staticmethod
            def from_dict(value):
                captured.update(value)
                return SimpleNamespace(to_dict=lambda: {"normalized": True})

        monkeypatch.setitem(
            sys.modules, "backtide.backtest", SimpleNamespace(ExperimentConfig=Config)
        )

        result = BacktideServices().parse_experiment_config({"suffix": suffix, "text": text})

        assert captured == parsed
        assert result == {"normalized": True}

    def test_unknown_uploaded_config_suffix_is_rejected(self):
        """Only the documented configuration formats are accepted."""
        with pytest.raises(APIError, match=r"Use a \.toml"):
            BacktideServices().parse_experiment_config({"suffix": ".py", "text": "pass"})

    def test_experiment_detail_skips_unused_equity_history(self, monkeypatch, tmp_path: Path):
        """Result details avoid loading and returning per-bar equity history."""
        experiment = tmp_path / "experiments" / "exp-1"
        experiment.mkdir(parents=True)
        (experiment / "config.toml").write_text(
            """[general]
name = "test"

[data]
symbols = ["AAPL", "MSFT"]
instrument_type = "stocks"
interval = "OneDay"
full_history = false
start_date = "2024-01-01"
end_date = "2024-03-01"
""",
            encoding="utf-8",
        )
        captured = {}
        run = SimpleNamespace(
            strategy_id="run-1",
            strategy_name="Momentum",
            base_currency="USD",
            is_benchmark=False,
            metrics={"return": 0.1},
            error=None,
            trades=[],
            orders=[],
        )
        benchmark = SimpleNamespace(
            strategy_id="benchmark-1",
            strategy_name="Benchmark",
            base_currency="USD",
            is_benchmark=True,
            metrics={"return": 0.08},
            error=None,
            trades=[],
            orders=[],
        )

        def query_strategy_runs(
            experiment_id,
            *,
            include_equity_curve=True,
            include_trades=True,
            include_orders=True,
        ):
            captured.update(
                experiment_id=experiment_id,
                include_equity_curve=include_equity_curve,
                include_trades=include_trades,
                include_orders=include_orders,
            )
            return [run, benchmark]

        monkeypatch.setitem(
            sys.modules,
            "backtide.config",
            SimpleNamespace(
                get_config=lambda: SimpleNamespace(
                    data=SimpleNamespace(storage_path=str(tmp_path))
                )
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_experiments=lambda _experiment_id: [
                    {"id": "exp-1", "name": "Test", "status": "Success"}
                ],
                query_strategy_runs=query_strategy_runs,
                query_study=lambda _experiment_id: None,
            ),
        )

        services = BacktideServices()
        monkeypatch.setattr(services, "metric_catalog", lambda: {"builtin": [], "saved": []})

        result = services.experiment("exp-1")

        assert captured == {
            "experiment_id": "exp-1",
            "include_equity_curve": False,
            "include_trades": False,
            "include_orders": False,
        }
        assert [item["strategy_name"] for item in result["runs"]] == [
            "Benchmark",
            "Momentum",
        ]
        assert "equity_curve" not in result["runs"][1]
        assert result["runs"][1]["metrics"] == {"return": 0.1}
        assert "orders" not in result["runs"][1]
        assert result["runs"][1]["order_count"] == 0
        assert result["config_metadata"] == {
            "symbols": 2,
            "symbol_values": ["AAPL", "MSFT"],
            "instrument_type": "stocks",
            "interval": "OneDay",
            "full_history": False,
            "start_date": "2024-01-01",
            "end_date": "2024-03-01",
        }

    def test_experiment_orders_are_returned_newest_first_in_bounded_pages(self, monkeypatch):
        """Order details load in stable newest-first batches instead of the main payload."""
        orders = [
            SimpleNamespace(
                timestamp=index,
                status="Filled",
                fill_price=100.0,
                commission=0.0,
                pnl=float(index),
                reason=None,
                order=SimpleNamespace(
                    id=f"order-{index}",
                    symbol="AAPL",
                    order_type="Market",
                    quantity=1.0,
                    price=None,
                    limit_price=None,
                ),
            )
            for index in range(205)
        ]
        run = SimpleNamespace(strategy_id="run-1", orders=orders)
        services = BacktideServices()
        monkeypatch.setattr(
            services,
            "_query_result_runs",
            lambda _experiment_id, **_kwargs: [run],
        )

        first = services.experiment_orders("exp-1", "run-1")
        last = services.experiment_orders("exp-1", "run-1", offset=200, limit=100)

        assert first["total"] == 205
        assert first["has_more"] is True
        assert len(first["orders"]) == 100
        assert first["orders"][0]["timestamp"] == 204
        assert first["orders"][-1]["timestamp"] == 105
        assert len(last["orders"]) == 5
        assert last["orders"][0]["timestamp"] == 4
        assert last["orders"][-1]["timestamp"] == 0
        assert last["has_more"] is False

    def test_experiment_detail_distinguishes_empty_and_missing_logs(
        self, monkeypatch, tmp_path: Path
    ):
        """An empty saved log remains a real artifact while a missing log is null."""
        experiment = tmp_path / "experiments" / "exp-1"
        experiment.mkdir(parents=True)
        (experiment / "logs.txt").write_text("", encoding="utf-8")

        monkeypatch.setitem(
            sys.modules,
            "backtide.config",
            SimpleNamespace(
                get_config=lambda: SimpleNamespace(
                    data=SimpleNamespace(storage_path=str(tmp_path))
                )
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_experiments=lambda _experiment_id: [
                    {"id": "exp-1", "name": "Test", "status": "Success"}
                ],
                query_strategy_runs=lambda _experiment_id, **_kwargs: [],
                query_study=lambda _experiment_id: None,
            ),
        )

        services = BacktideServices()
        monkeypatch.setattr(services, "metric_catalog", lambda: {"builtin": [], "saved": []})
        detail = services.experiment("exp-1")
        assert detail["logs"] == ""
        assert detail["logs_truncated"] is False

        (experiment / "logs.txt").unlink()
        detail = services.experiment("exp-1")
        assert detail["logs"] is None
        assert detail["logs_truncated"] is False

    def test_experiment_detail_bounds_large_logs(self, monkeypatch, tmp_path: Path):
        """Experiment details return only the bounded tail of a large log file."""
        experiment = tmp_path / "experiments" / "exp-1"
        experiment.mkdir(parents=True)
        (experiment / "logs.txt").write_text(
            "\n".join(f"log line {index}" for index in range(2_000)),
            encoding="utf-8",
        )

        monkeypatch.setitem(
            sys.modules,
            "backtide.config",
            SimpleNamespace(
                get_config=lambda: SimpleNamespace(
                    data=SimpleNamespace(storage_path=str(tmp_path))
                )
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_experiments=lambda _experiment_id: [
                    {"id": "exp-1", "name": "Test", "status": "Success"}
                ],
                query_strategy_runs=lambda _experiment_id, **_kwargs: [],
                query_study=lambda _experiment_id: None,
            ),
        )

        services = BacktideServices()
        monkeypatch.setattr(services, "metric_catalog", lambda: {"builtin": [], "saved": []})

        detail = services.experiment("exp-1")

        assert detail["logs_truncated"] is True
        assert len(detail["logs"].splitlines()) == 1_000
        assert detail["logs"].startswith("log line 1000")
        assert detail["logs"].endswith("log line 1999")

    def test_experiment_log_download_returns_the_complete_file(self, monkeypatch, tmp_path: Path):
        """Full-log downloads are not limited by the experiment-detail preview."""
        experiment = tmp_path / "experiments" / "exp-1"
        experiment.mkdir(parents=True)
        full_log = "\n".join(f"log line {index}" for index in range(2_000))
        (experiment / "logs.txt").write_text(full_log, encoding="utf-8")

        monkeypatch.setitem(
            sys.modules,
            "backtide.config",
            SimpleNamespace(
                get_config=lambda: SimpleNamespace(
                    data=SimpleNamespace(storage_path=str(tmp_path))
                )
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_experiments=lambda _experiment_id: [
                    {"id": "exp-1", "name": "Momentum / study"}
                ]
            ),
        )

        filename, body = BacktideServices().experiment_log("exp-1")

        assert filename == "Momentum _ study-logs.txt"
        assert body.decode() == full_log

    def test_dashboard_uses_enriched_recent_experiment_metrics(self, monkeypatch):
        """Dashboard activity includes recent experiments and persisted live sessions."""
        recent = [{"id": "exp-1", "primary_metric_name": "CAGR", "primary_metric_value": 0.2}]
        sessions = [{"id": f"session-{index}"} for index in range(8)]
        captured = {}
        services = BacktideServices()

        def experiments(search=None, limit=100):
            captured.update(search=search, limit=limit)
            return recent

        monkeypatch.setattr(services, "experiments", experiments)
        monkeypatch.setattr(services, "live_sessions", lambda: sessions)
        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_bars_summary=list,
                query_experiments=lambda: [{"id": "exp-1"}],
            ),
        )

        result = services.dashboard()

        assert captured == {"search": None, "limit": 6}
        assert result["experiments"] == recent
        assert result["sessions"] == sessions[:6]
        assert result["metrics"]["sessions"] == 8

    def test_experiment_summaries_include_lightweight_strategy_metrics(self, monkeypatch):
        """Result cards include per-strategy metrics without loading equity curves."""
        captured = []
        run = SimpleNamespace(
            strategy_id="run-1",
            strategy_name="Momentum",
            base_currency="USD",
            is_benchmark=False,
            metrics={"sharpe_ratio": 1.4, "total_return": 0.12},
            error=None,
        )

        def query_strategy_runs(experiment_id, **kwargs):
            captured.append((experiment_id, kwargs))
            return [run]

        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_experiments=lambda **_kwargs: [
                    {"id": "exp-1", "name": "Momentum study", "icon": "🎯"}
                ],
                query_strategy_runs=query_strategy_runs,
                query_study=lambda _experiment_id: None,
            ),
        )

        services = BacktideServices()
        monkeypatch.setattr(
            services,
            "_read_text",
            lambda *_args, **_kwargs: '[data]\nsymbols = ["AAPL", "MSFT"]',
        )

        result = services.experiments()

        assert captured == [
            (
                "exp-1",
                {
                    "include_equity_curve": False,
                    "include_trades": False,
                    "include_orders": False,
                },
            )
        ]
        assert result[0]["n_symbols"] == 2
        assert result[0]["runs"] == [
            {
                "strategy_id": "run-1",
                "strategy_name": "Momentum",
                "base_currency": "USD",
                "is_benchmark": False,
                "metrics": {"sharpe_ratio": 1.4, "total_return": 0.12},
                "error": None,
            }
        ]

    def test_primary_metric_summary_preserves_configured_metric_order(self):
        """Result summaries expose selected metrics for ordered breakdown rendering."""
        services = BacktideServices()
        result = services._primary_metric_summary(
            """
metrics = ["sharpe", "total_return", "pnl", "win_rate"]
""",
            [{"is_benchmark": False, "metrics": {"sharpe": 1.25}}],
            {
                "builtin": [
                    {
                        "key": "sharpe",
                        "name": "Sharpe ratio",
                        "percentage": False,
                        "greater_is_better": True,
                    }
                ],
                "saved": [],
            },
        )

        assert result["primary_metric"] == "sharpe"
        assert result["selected_metrics"] == [
            "sharpe",
            "total_return",
            "pnl",
            "win_rate",
        ]
        assert result["primary_metric_value"] == 1.25

    def test_primary_metric_summary_reads_the_metrics_section(self):
        """The canonical metrics section retains configured ordering in result summaries."""
        result = BacktideServices()._primary_metric_summary(
            '[metrics]\nselected = ["total_return", "sharpe"]\n',
            [{"is_benchmark": False, "metrics": {"total_return": 0.2}}],
            {
                "builtin": [
                    {
                        "key": "total_return",
                        "name": "Total return",
                        "percentage": True,
                        "greater_is_better": True,
                    }
                ],
                "saved": [],
            },
        )

        assert result["primary_metric"] == "total_return"
        assert result["selected_metrics"] == ["total_return", "sharpe"]
        assert result["primary_metric_value"] == 0.2

    def test_study_runs_use_compact_names_and_parameter_metadata(self):
        """Saved candidate parameters stay out of labels and remain available to the UI."""
        runs = [{"strategy_id": "run-2", "strategy_name": "C002 · fast=20 · slow=100"}]
        study = SimpleNamespace(
            candidates=[
                SimpleNamespace(
                    strategy_id="run-2",
                    strategy_name="C002 · fast=20 · slow=100",
                    parameters={"fast": 20, "slow": 100},
                )
            ]
        )

        BacktideServices._apply_study_run_metadata(runs, study)

        assert runs == [
            {
                "strategy_id": "run-2",
                "strategy_name": "C002",
                "parameters": {"fast": 20, "slow": 100},
            }
        ]

    def test_plot_payloads_replace_legacy_parameter_heavy_candidate_names(self):
        """Plot legends and hover labels use compact candidate names for saved studies."""
        payload = {
            "data": [
                {
                    "name": "C002 · fast=20 · slow=100",
                    "legendgroup": "C002 · fast=20 · slow=100",
                    "hovertemplate": "C002 · fast=20 · slow=100: %{y}",
                }
            ]
        }

        result = BacktideServices._replace_figure_candidate_names(
            payload,
            {"C002 · fast=20 · slow=100": "C002"},
        )

        assert result["data"] == [
            {
                "name": "C002",
                "legendgroup": "C002",
                "hovertemplate": "C002: %{y}",
            }
        ]

    def test_study_runs_put_benchmark_before_top_three_in_rank_order(self):
        """Study summaries and charts put the benchmark before the three best candidates."""
        benchmark = SimpleNamespace(strategy_id="benchmark", is_benchmark=True)
        candidates = [
            SimpleNamespace(
                strategy_id=f"run-{rank}",
                strategy_name=f"C{rank:03d}",
                rank=rank,
            )
            for rank in range(1, 6)
        ]
        runs = [
            *(
                SimpleNamespace(strategy_id=f"run-{rank}", is_benchmark=False)
                for rank in range(1, 6)
            ),
            benchmark,
        ]

        selected = BacktideServices._study_detail_runs(
            runs,
            SimpleNamespace(candidates=candidates),
        )

        assert [run.strategy_id for run in selected] == [
            "benchmark",
            "run-1",
            "run-2",
            "run-3",
        ]

    def test_study_summary_uses_the_best_eligible_candidate(self):
        """A constrained study headline cannot be taken from an excluded candidate."""
        study = SimpleNamespace(
            objective="sharpe",
            best_candidate=SimpleNamespace(metrics={"sharpe": 1.1}),
        )

        result = BacktideServices._study_metric_summary(
            study,
            {
                "builtin": [{"key": "sharpe", "name": "Sharpe ratio", "percentage": False}],
                "saved": [],
            },
        )

        assert result == {
            "primary_metric": "sharpe",
            "primary_metric_name": "Sharpe ratio",
            "primary_metric_value": 1.1,
            "primary_metric_percentage": False,
        }

    def test_experiment_summaries_only_enrich_the_requested_page(self, monkeypatch):
        """Experiment paging fetches and enriches only the requested ten-item slice."""
        captured = {}
        rows = [{"id": f"exp-{index}", "name": f"Study {index}"} for index in range(15)]
        enriched = []

        def query_experiments(**kwargs):
            captured.update(kwargs)
            return rows[: kwargs["limit"]]

        def query_strategy_runs(experiment_id, **kwargs):
            enriched.append((experiment_id, kwargs))
            return []

        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_experiments=query_experiments,
                query_strategy_runs=query_strategy_runs,
                query_study=lambda _experiment_id: None,
            ),
        )
        services = BacktideServices()
        monkeypatch.setattr(services, "metric_catalog", dict)
        monkeypatch.setattr(services, "_primary_metric_summary", lambda *_args: {})

        result = services.experiments(search="study", limit=10, offset=10)

        assert captured == {"search": "study", "limit": 20}
        assert [item["id"] for item in result] == [f"exp-{index}" for index in range(10, 15)]
        assert enriched == [
            (
                f"exp-{index}",
                {
                    "include_equity_curve": False,
                    "include_trades": False,
                    "include_orders": False,
                },
            )
            for index in range(10, 15)
        ]

    def test_required_indicator_catalog_exposes_auto_injected_metadata(self, monkeypatch):
        """Saved strategies describe indicators that the engine injects automatically."""

        class Indicator:
            @staticmethod
            def description():
                return "Smooths prices over a fixed lookback."

        monkeypatch.setitem(
            sys.modules,
            "backtide.strategies.utils",
            SimpleNamespace(
                _resolve_auto_indicators=lambda _strategies: [("SMA 20", Indicator(), "built-in")]
            ),
        )

        result = BacktideServices._required_indicator_catalog(SimpleNamespace())

        assert result == [
            {
                "name": "SMA 20",
                "type": "Indicator",
                "description": "Smooths prices over a fixed lookback.",
            }
        ]

    def test_result_plot_dispatches_to_existing_analysis(self, monkeypatch, tmp_path: Path):
        """Result plot requests use the public Plotly analysis function."""
        import backtide

        experiment = tmp_path / "experiments" / "exp-1"
        experiment.mkdir(parents=True)
        (experiment / "config.toml").write_text("[general]\nname='test'", encoding="utf-8")
        run = SimpleNamespace(strategy_id="run-1")
        captured: dict[str, Any] = {"queries": [], "plots": []}

        class Figure:
            def to_json(self):
                return '{"data":[],"layout":{"title":"PnL"}}'

        def plot_pnl(runs, *, normalize, drawdown, display):
            captured["plots"].append(
                {
                    "runs": runs,
                    "normalize": normalize,
                    "drawdown": drawdown,
                    "display": display,
                }
            )
            return Figure()

        def query_strategy_runs(_experiment_id, **kwargs):
            captured["queries"].append(kwargs)
            return [run]

        analysis = SimpleNamespace(plot_pnl=plot_pnl)
        monkeypatch.setitem(sys.modules, "backtide.analysis", analysis)
        monkeypatch.setattr(backtide, "analysis", analysis, raising=False)
        monkeypatch.setitem(
            sys.modules,
            "backtide.backtest",
            SimpleNamespace(
                ExperimentConfig=SimpleNamespace(from_toml=lambda _text: SimpleNamespace())
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.config",
            SimpleNamespace(
                get_config=lambda: SimpleNamespace(
                    data=SimpleNamespace(storage_path=str(tmp_path))
                )
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_bars=lambda *_args, **_kwargs: [],
                query_strategy_runs=query_strategy_runs,
                query_study=lambda _experiment_id: None,
            ),
        )

        services = BacktideServices()
        result = services.result_plot(
            {
                "experiment_id": "exp-1",
                "strategy_id": "run-1",
                "plot": "pnl",
                "options": {"normalize": True, "drawdown": False},
            }
        )
        services.result_plot(
            {
                "experiment_id": "exp-1",
                "strategy_id": "run-1",
                "plot": "pnl",
            }
        )

        assert result["layout"]["title"] == "PnL"
        assert captured == {
            "queries": [
                {
                    "include_equity_curve": True,
                    "include_trades": False,
                    "include_orders": False,
                }
            ],
            "plots": [
                {
                    "runs": [run],
                    "normalize": True,
                    "drawdown": False,
                    "display": None,
                },
                {
                    "runs": [run],
                    "normalize": False,
                    "drawdown": True,
                    "display": None,
                },
            ],
        }

    def test_analysis_metrics_returns_per_symbol_statistics(self, monkeypatch):
        """The first analysis tab returns tabular statistics without building a figure."""
        import backtide

        bars = [{"symbol": "AAPL", "close": 100.0}]
        captured: dict[str, Any] = {}

        class Statistics:
            @staticmethod
            def to_dict(*, orient):
                assert orient == "records"
                return [
                    {
                        "symbol": "AAPL",
                        "sharpe": 1.24,
                        "cagr": 0.137,
                        "max_dd": -0.082,
                        "win_rate": 0.54,
                    }
                ]

        def compute_statistics(data, *, price_col):
            captured.update(data=data, price_col=price_col)
            return Statistics()

        def query_bars(symbols, interval, provider, *, limit):
            captured.update(
                symbols=symbols,
                interval=interval,
                provider=provider,
                limit=limit,
            )
            return bars

        analysis = SimpleNamespace(compute_statistics=compute_statistics)
        monkeypatch.setitem(sys.modules, "backtide.analysis", analysis)
        monkeypatch.setattr(backtide, "analysis", analysis, raising=False)
        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(query_bars=query_bars, query_dividends=lambda *_args: []),
        )

        result = BacktideServices().analysis_plot(
            "metrics",
            {
                "symbols": ["AAPL"],
                "interval": "1d",
                "provider": "yahoo",
                "price_col": "close",
            },
        )

        assert result == {
            "rows": [
                {
                    "symbol": "AAPL",
                    "sharpe": 1.24,
                    "cagr": 0.137,
                    "max_dd": -0.082,
                    "win_rate": 0.54,
                }
            ]
        }
        assert captured == {
            "data": bars,
            "price_col": "close",
            "symbols": ["AAPL"],
            "interval": "1d",
            "provider": "yahoo",
            "limit": 50_000,
        }

    def test_result_dividends_plot_uses_the_experiment_symbols(self, monkeypatch, tmp_path: Path):
        """The result dividends tab queries payouts for the configured universe."""
        import backtide

        experiment = tmp_path / "experiments" / "exp-1"
        experiment.mkdir(parents=True)
        (experiment / "config.toml").write_text("config", encoding="utf-8")
        run = SimpleNamespace(strategy_id="run-1")
        captured: dict[str, Any] = {}

        class Figure:
            def to_json(self):
                return '{"data":[],"layout":{"title":"Dividends"}}'

        def query_dividends(symbols):
            captured["symbols"] = symbols
            return [{"symbol": "AAPL", "amount": 1.0}]

        def plot_dividends(data, *, display):
            captured.update(data=data, display=display)
            return Figure()

        analysis = SimpleNamespace(plot_dividends=plot_dividends)
        monkeypatch.setitem(sys.modules, "backtide.analysis", analysis)
        monkeypatch.setattr(backtide, "analysis", analysis, raising=False)
        monkeypatch.setitem(
            sys.modules,
            "backtide.backtest",
            SimpleNamespace(
                ExperimentConfig=SimpleNamespace(
                    from_toml=lambda _text: SimpleNamespace(
                        data=SimpleNamespace(symbols=["AAPL", "MSFT"])
                    )
                )
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.config",
            SimpleNamespace(
                get_config=lambda: SimpleNamespace(
                    data=SimpleNamespace(storage_path=str(tmp_path))
                )
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_bars=lambda *_args, **_kwargs: [],
                query_dividends=query_dividends,
                query_strategy_runs=lambda _experiment_id, **_kwargs: [run],
                query_study=lambda _experiment_id: None,
            ),
        )

        result = BacktideServices().result_plot(
            {
                "experiment_id": "exp-1",
                "strategy_id": "run-1",
                "plot": "dividends",
            }
        )

        assert result["layout"]["title"] == "Dividends"
        assert captured == {
            "symbols": ["AAPL", "MSFT"],
            "data": [{"symbol": "AAPL", "amount": 1.0}],
            "display": None,
        }

    def test_benchmark_runs_use_the_configured_symbol_as_their_display_name(self):
        """Benchmark result labels identify the compared market instrument."""
        runs = [
            {"strategy_name": "Benchmark", "is_benchmark": True},
            {"strategy_name": "Momentum", "is_benchmark": False},
        ]

        BacktideServices._apply_benchmark_display_name('[strategy]\nbenchmark = "SPY"', runs)

        assert [run["strategy_name"] for run in runs] == ["SPY", "Momentum"]

    def test_unknown_result_plot_is_rejected(self, monkeypatch, tmp_path: Path):
        """Result plot names are restricted to the explicit dispatch table."""
        import backtide

        experiment = tmp_path / "experiments" / "exp-1"
        experiment.mkdir(parents=True)
        (experiment / "config.toml").write_text("config", encoding="utf-8")
        analysis = SimpleNamespace()
        monkeypatch.setitem(sys.modules, "backtide.analysis", analysis)
        monkeypatch.setattr(backtide, "analysis", analysis, raising=False)
        monkeypatch.setitem(
            sys.modules,
            "backtide.backtest",
            SimpleNamespace(
                ExperimentConfig=SimpleNamespace(from_toml=lambda _text: SimpleNamespace())
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.config",
            SimpleNamespace(
                get_config=lambda: SimpleNamespace(
                    data=SimpleNamespace(storage_path=str(tmp_path))
                )
            ),
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.storage",
            SimpleNamespace(
                query_bars=lambda *_args, **_kwargs: [],
                query_strategy_runs=lambda _experiment_id, **_kwargs: [
                    SimpleNamespace(strategy_id="run-1")
                ],
                query_study=lambda _experiment_id: None,
            ),
        )

        with pytest.raises(APIError, match="Unknown result plot"):
            BacktideServices().result_plot({"experiment_id": "exp-1", "plot": "not-a-plot"})

    @pytest.mark.parametrize(
        ("folder", "method_name", "utils_module"),
        [
            ("strategies", "update_strategy", "backtide.strategies.utils"),
            ("indicators", "update_indicator", "backtide.indicators.utils"),
        ],
    )
    def test_custom_library_update_replaces_source_and_renames_file(
        self,
        monkeypatch,
        tmp_path: Path,
        folder,
        method_name,
        utils_module,
    ):
        """Custom strategy and indicator updates persist code under the edited name."""
        directory = tmp_path / folder
        directory.mkdir()
        original_path = directory / "Original.pkl"
        original_path.write_bytes(b"original")
        cfg = SimpleNamespace(data=SimpleNamespace(storage_path=str(tmp_path)))
        existing = SimpleNamespace(version="original")
        updated = SimpleNamespace(version="updated")
        captured = {}

        def save(value, name, _cfg):
            captured.update(value=value, name=name)
            (directory / f"{name}.pkl").write_bytes(b"updated")

        monkeypatch.setitem(
            sys.modules,
            "backtide.config",
            SimpleNamespace(get_config=lambda: cfg),
        )
        if folder == "strategies":
            utilities = SimpleNamespace(
                _build_custom_strategy=lambda _code: updated,
                _check_strategy_code=lambda code: captured.setdefault("code", code) and None,
                _is_builtin_strategy=lambda _value: False,
                _load_stored_strategies=lambda _cfg: {"Original": existing},
                _save_strategy=save,
            )
        else:
            utilities = SimpleNamespace(
                _build_custom_indicator=lambda _code: updated,
                _check_indicator_code=lambda code, _cfg: (
                    captured.setdefault("code", code) and None
                ),
                _is_builtin_indicator=lambda _value: False,
                _load_stored_indicators=lambda _cfg: {"Original": existing},
                _save_indicator=save,
            )
        monkeypatch.setitem(sys.modules, utils_module, utilities)

        result = getattr(BacktideServices(), method_name)(
            "Original", {"name": "Renamed", "code": "updated source"}
        )

        assert result == {"saved": "Renamed"}
        assert captured == {"code": "updated source", "value": updated, "name": "Renamed"}
        assert not original_path.exists()
        assert (directory / "Renamed.pkl").read_bytes() == b"updated"

    def test_builtin_library_update_only_changes_the_saved_name(self, tmp_path: Path):
        """Renaming a built-in asset retains the existing configured instance."""
        directory = tmp_path / "strategies"
        directory.mkdir()
        original_path = directory / "Original.pkl"
        original_path.write_bytes(b"original")
        existing = SimpleNamespace(version="configured")
        captured = {}

        def save(value, name):
            captured.update(value=value, name=name)
            (directory / f"{name}.pkl").write_bytes(b"renamed")

        result = BacktideServices()._update_saved_asset(
            folder="strategies",
            label="strategy",
            original_name="Original",
            payload={"name": "Renamed"},
            stored={"Original": existing},
            storage_path=tmp_path,
            is_builtin=lambda _value: True,
            validate=lambda _code: pytest.fail("Built-in source must not be validated"),
            build=lambda _code: pytest.fail("Built-in source must not be rebuilt"),
            save=save,
        )

        assert result == {"saved": "Renamed"}
        assert captured == {"value": existing, "name": "Renamed"}
        assert not original_path.exists()

    def test_custom_library_update_uses_the_rebuilt_class_when_name_is_empty(self, tmp_path: Path):
        """Editing custom code can rename the saved asset from its Python class."""
        directory = tmp_path / "strategies"
        directory.mkdir()
        original_path = directory / "Original.pkl"
        original_path.write_bytes(b"original")
        replacement = type("MomentumStrategy", (), {})()
        captured = {}

        result = BacktideServices()._update_saved_asset(
            folder="strategies",
            label="strategy",
            original_name="Original",
            payload={"kind": "custom", "name": "", "code": "updated source"},
            stored={"Original": object()},
            storage_path=tmp_path,
            is_builtin=lambda _value: False,
            validate=lambda _code: pytest.fail("Full updates use the replacement builder"),
            build=lambda _code: pytest.fail("Full updates use the replacement builder"),
            rebuild=lambda _payload: replacement,
            save=lambda value, name: captured.update(value=value, name=name),
            ignored_class_prefix="My",
        )

        assert result == {"saved": "MomentumStrategy"}
        assert captured == {"value": replacement, "name": "MomentumStrategy"}
        assert not original_path.exists()

    def test_library_rename_does_not_overwrite_an_existing_file(self, tmp_path: Path):
        """Renaming to an existing saved name returns a conflict without writing."""
        directory = tmp_path / "strategies"
        directory.mkdir()
        (directory / "Original.pkl").write_bytes(b"original")
        (directory / "Existing.pkl").write_bytes(b"existing")

        with pytest.raises(APIError, match="already exists") as exc_info:
            BacktideServices()._update_saved_asset(
                folder="strategies",
                label="strategy",
                original_name="Original",
                payload={"name": "Existing"},
                stored={"Original": object()},
                storage_path=tmp_path,
                is_builtin=lambda _value: True,
                validate=lambda _code: None,
                build=lambda _code: object(),
                save=lambda _value, _name: pytest.fail("Conflicting rename must not write"),
            )

        assert exc_info.value.status == 409
        assert (directory / "Original.pkl").read_bytes() == b"original"
        assert (directory / "Existing.pkl").read_bytes() == b"existing"

    def test_library_update_rebuilds_the_complete_configuration(self, tmp_path: Path):
        """A full editor payload replaces the stored kind, type, and parameter values."""
        directory = tmp_path / "strategies"
        directory.mkdir()
        original_path = directory / "Original.pkl"
        original_path.write_bytes(b"original")
        replacement = SimpleNamespace(version="replacement")
        captured = {}
        payload = {
            "kind": "builtin",
            "name": "Original",
            "type": "Momentum",
            "params": {"period": 30},
            "code": "",
        }

        result = BacktideServices()._update_saved_asset(
            folder="strategies",
            label="strategy",
            original_name="Original",
            payload=payload,
            stored={"Original": SimpleNamespace(version="existing")},
            storage_path=tmp_path,
            is_builtin=lambda _value: True,
            validate=lambda _code: pytest.fail("Full updates use the replacement builder"),
            build=lambda _code: pytest.fail("Full updates use the replacement builder"),
            rebuild=lambda value: captured.setdefault("payload", value) and replacement,
            save=lambda value, name: captured.update(value=value, name=name),
        )

        assert result == {"saved": "Original"}
        assert captured == {"payload": payload, "value": replacement, "name": "Original"}


class TestLiveCapabilities:
    """Tests for provider and interval capability discovery."""

    def test_reports_coinbase_support_per_interval(self, monkeypatch):
        """Coinbase remains selectable when its supported interval is chosen."""

        class Feed:
            def __init__(self, provider, _symbols, interval):
                if provider == "yahoo" or (provider == "coinbase" and interval != "5m"):
                    raise ValueError("Unsupported interval.")

            def cancel(self):
                return None

        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(LiveMarketFeed=Feed),
        )

        capabilities = BacktideServices().live_capabilities()

        coinbase = capabilities["providers"]["coinbase"]
        assert coinbase["supported"] is True
        assert coinbase["intervals"]["1m"]["supported"] is False
        assert coinbase["intervals"]["5m"]["supported"] is True
        assert capabilities["providers"]["yahoo"]["supported"] is False


class TestLiveTradingManager:
    """Tests for live-session validation and idempotent stop behavior."""

    def test_start_rejects_unsupported_provider(self, monkeypatch):
        """A provider capability failure is returned before a thread starts."""

        class UnsupportedFeed:
            def __init__(self, _provider, _symbols, _interval):
                raise ValueError("No WebSocket feed.")

        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(
                LiveMarketFeed=UnsupportedFeed,
                SessionConfig=object,
                Session=object,
            ),
        )

        with pytest.raises(APIError, match="No WebSocket feed"):
            LiveTradingManager().start(
                {"provider": "yahoo", "interval": "1m", "symbols": ["AAPL"]}
            )

    def test_stop_without_session_is_idempotent(self):
        """Stopping before start leaves the manager in an idle state."""
        assert LiveTradingManager().stop()["status"] == "idle"

    @pytest.mark.parametrize(
        ("value", "expected"),
        [("max", 0.0), (1, 1.0), ("2", 2.0), (10.0, 10.0)],
    )
    def test_replay_speed_accepts_real_time_multipliers(self, value, expected):
        """Replay speed accepts bounded multipliers and an unlimited mode."""
        assert LiveTradingManager._playback_speed(value) == expected

    @pytest.mark.parametrize("value", [0, -1, 101, "slow"])
    def test_replay_speed_rejects_invalid_values(self, value):
        """Replay speed rejects values that cannot produce safe bounded waits."""
        with pytest.raises(APIError, match=r"between 0\.1x and 100x"):
            LiveTradingManager._playback_speed(value)

    def test_replay_pause_blocks_without_consuming_the_next_event(
        self,
        monkeypatch,
    ):
        """Paused playback waits instead of dropping recorded market events."""
        delay_started = threading.Event()
        release_delay = threading.Event()
        second_processed = threading.Event()
        replay_delays = []

        class Config:
            def __init__(self, initial_cash=10_000.0):
                self.initial_cash = initial_cash

        class Market:
            def __init__(self, **values):
                self.__dict__.update(values)

        class Session:
            calls = 0

            def __init__(self, _config, _strategy):
                pass

            @classmethod
            def on_bar(cls, market, _orders):
                cls.calls += 1
                if cls.calls == 2:
                    second_processed.set()
                return SimpleNamespace(
                    market=market,
                    fills=[],
                    orders_submitted=0,
                    processed=True,
                    snapshot=None,
                    indicators={},
                )

            @staticmethod
            def snapshot():
                return None

        markets = [
            {
                "symbol": "BTC-USD",
                "interval": "1m",
                "open_ts": timestamp,
                "close_ts": timestamp + 1,
                "open": 100.0,
                "high": 100.0,
                "low": 100.0,
                "close": 100.0,
                "volume": 1.0,
                "n_trades": 1,
                "is_final": True,
                "provider": "mock",
                "received_ts": timestamp + 1,
            }
            for timestamp in (1_700_000_000, 1_700_000_001)
        ]
        session_id = _persist_live_replay_source(
            {
                "mode": "live",
                "provider": "mock",
                "interval": "1m",
                "symbols": ["BTC-USD"],
                "strategies": [],
                "warmup_bars": 0,
                "config": {"initial_cash": 10_000.0},
            },
            events=[
                {"market": market, "received_at": market["received_ts"]} for market in markets
            ],
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(
                MarketUpdate=Market,
                SessionConfig=Config,
                Session=Session,
            ),
        )
        manager = LiveTradingManager()

        def wait_for_delay(seconds):
            replay_delays.append(seconds)
            delay_started.set()
            return release_delay.wait(timeout=1.0)

        monkeypatch.setattr(manager, "_wait_replay_delay", wait_for_delay)

        manager.replay(session_id, 1)
        assert delay_started.wait(timeout=1.0)
        assert manager._replay_processed_events == 1

        manager.pause()
        release_delay.set()
        assert not second_processed.wait(timeout=0.1)

        manager.resume()
        assert second_processed.wait(timeout=1.0)
        assert manager._thread is not None
        manager._thread.join(timeout=1.0)
        assert manager._replay_processed_events == 2
        assert replay_delays == [1.0]
        replay_id = manager._session_id
        assert replay_id is not None
        manager.delete_session(replay_id)
        manager.delete_session(session_id)

    def test_replay_restores_recorded_warmup_without_querying_storage(
        self,
        monkeypatch,
    ):
        """Replay initializes strategies from the exact persisted warm-up stream."""
        warmed = []

        class Config:
            def __init__(self, initial_cash=10_000.0):
                self.initial_cash = initial_cash

        class Market:
            def __init__(self, **values):
                self.__dict__.update(values)

        class Session:
            def __init__(self, _config, _strategy):
                pass

            @staticmethod
            def warm_up(markets):
                warmed.extend(markets)

            @staticmethod
            def snapshot():
                return None

        warmup = {
            "symbol": "BTC-USD",
            "interval": "1m",
            "open_ts": 1_700_000_000,
            "close_ts": 1_700_000_060,
            "open": 100.0,
            "high": 101.0,
            "low": 99.0,
            "close": 100.5,
            "volume": 10.0,
            "n_trades": 2,
            "is_final": True,
            "provider": "mock",
            "received_ts": 1_700_000_060,
        }
        session_id = _persist_live_replay_source(
            {
                "mode": "live",
                "provider": "mock",
                "interval": "1m",
                "symbols": ["BTC-USD"],
                "strategies": [],
                "warmup_bars": 1,
                "config": {"initial_cash": 10_000.0},
            },
            warmup=[warmup],
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(
                MarketUpdate=Market,
                SessionConfig=Config,
                Session=Session,
            ),
        )
        manager = LiveTradingManager()

        manager.replay(session_id)
        assert manager._thread is not None
        manager._thread.join(timeout=1.0)

        assert [market.symbol for market in warmed] == ["BTC-USD"]
        assert manager._warmup_loaded == 1
        assert manager.status()["replay"]["warmup_source"] == "recorded"
        replay_id = manager._session_id
        assert replay_id is not None
        manager.delete_session(replay_id)
        manager.delete_session(session_id)

    def test_replay_rejects_unknown_config_fields(
        self,
        monkeypatch,
    ):
        """Replay does not silently filter unsupported session configuration."""

        class Config:
            def __init__(self, initial_cash=100_000.0):
                self.initial_cash = initial_cash

        session_id = _persist_live_replay_source(
            {
                "mode": "live",
                "strategies": [],
                "config": {
                    "initial_cash": 25_000.0,
                    "allowed_order_types": ["Market"],
                },
            }
        )
        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(
                MarketUpdate=SimpleNamespace,
                SessionConfig=Config,
                Session=SimpleNamespace,
            ),
        )
        manager = LiveTradingManager()

        with pytest.raises(APIError, match="allowed_order_types"):
            manager.replay(session_id)

        manager.delete_session(session_id)

    def test_mock_feed_updates_session_and_stops_cleanly(self, monkeypatch):
        """A mocked WebSocket batch is processed, serialized, and canceled on stop."""
        processed = threading.Event()
        instances = []
        market = SimpleNamespace(
            symbol="BTC-USD",
            interval="1m",
            open_ts=1_700_000_000,
            close_ts=1_700_000_060,
            open=100.0,
            high=102.0,
            low=99.0,
            close=101.0,
            volume=10.0,
            is_final=True,
            provider="mock",
            received_ts=1_700_000_061,
        )
        snapshot = SimpleNamespace(
            latest_prices={"BTC-USD": 101.0},
            equity=1_001.0,
            realized_pnl=1.0,
            unrealized_pnl=0.0,
            processed_bars=1,
            portfolio=SimpleNamespace(cash={"USD": 900.0}, positions={"BTC-USD": 1.0}, orders=[]),
        )

        class Feed:
            def __init__(self, provider, symbols, interval, *, include_partial=True):
                self.arguments = (provider, symbols, interval, include_partial)
                self.canceled = threading.Event()
                self.emitted = False
                instances.append(self)

            def cancel(self):
                self.canceled.set()

            def collect(self, max_events, timeout_seconds):
                assert (max_events, timeout_seconds) == (10, 5.0)
                if not self.emitted:
                    self.emitted = True
                    return [market]
                self.canceled.wait(timeout=1.0)
                return []

        class Session:
            def __init__(self, config, strategy):
                assert config.values == {"initial_cash": 1_000.0}
                assert strategy is None

            def on_bar(self, value, orders=None):
                assert value is market
                assert orders is None
                processed.set()
                return SimpleNamespace(
                    market=market,
                    fills=[],
                    orders_submitted=0,
                    processed=True,
                    snapshot=snapshot,
                )

            @staticmethod
            def snapshot():
                return snapshot

        class Config:
            def __init__(self, **values):
                self.values = values

        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(
                LiveMarketFeed=Feed,
                SessionConfig=Config,
                Session=Session,
            ),
        )
        manager = LiveTradingManager()

        manager.start(
            {
                "provider": "KRAKEN",
                "interval": "1m",
                "symbols": [" btc-usd ", ""],
                "config": {"initial_cash": 1_000.0},
            }
        )
        assert processed.wait(timeout=1.0)
        status = manager.stop()

        assert status["status"] == "stopped"
        assert status["config"]["provider"] == "kraken"
        assert status["config"]["symbols"] == ["BTC-USD"]
        assert status["updates"][0]["market"]["close"] == 101.0
        assert status["updates"][0]["snapshot"]["equity"] == 1_001.0
        assert len(instances) == 2
        assert instances[0].canceled.is_set()
        assert instances[1].canceled.is_set()

    def test_conversion_updates_seed_accounts_before_target_processing(self):
        """Conversion legs are observed before a foreign-quoted target reaches a strategy."""
        processed = []
        rates = []

        class Session:
            @staticmethod
            def set_exchange_rate(base, quote, rate, timestamp):
                rates.append((base, quote, rate, timestamp))

            @staticmethod
            def on_bar(value, _orders):
                processed.append(value.symbol)
                return SimpleNamespace(
                    market=value,
                    fills=[],
                    orders_submitted=0,
                    processed=True,
                    snapshot=None,
                    indicators={},
                )

            @staticmethod
            def snapshot():
                return None

        def update(symbol, close):
            return SimpleNamespace(
                symbol=symbol,
                quote_currency=symbol.rsplit("-", 1)[-1],
                interval="1m",
                open_ts=1_700_000_000,
                close_ts=1_700_000_060,
                open=close,
                high=close,
                low=close,
                close=close,
                volume=1.0,
                n_trades=1,
                is_final=True,
                provider="mock",
                received_ts=1_700_000_061,
            )

        manager = LiveTradingManager()
        manager._sessions = {"Monitor": Session()}
        manager._session = manager._sessions["Monitor"]
        manager._config = {"symbols": ["AAVE-ETH"]}
        manager._conversion_legs = {"ETH-EUR": ("ETH", "EUR")}
        manager._prepare_session()

        manager._process_market(update("AAVE-ETH", 0.04653))
        manager._process_market(update("ETH-EUR", 4_000.0))
        manager._process_market(update("AAVE-ETH", 0.04653))

        assert rates == [("ETH", "EUR", 4_000.0, 1_700_000_060)]
        assert processed == ["AAVE-ETH"]
        assert manager.status()["updates"][0]["exchange_rates"]["ETH-EUR"]["rate"] == 4_000.0

    def test_mock_feed_failure_sets_terminal_error(self, monkeypatch):
        """A mocked WebSocket failure stops the worker and exposes its message."""
        failed = threading.Event()

        class Feed:
            def __init__(self, _provider, _symbols, _interval, *, include_partial=True):
                self.include_partial = include_partial

            def cancel(self):
                return None

            def collect(self, max_events, timeout_seconds):
                assert (max_events, timeout_seconds, self.include_partial) == (10, 5.0, True)
                failed.set()
                raise RuntimeError("mock socket disconnected")

        class Session:
            def __init__(self, _config, _strategy):
                pass

            @staticmethod
            def snapshot():
                return SimpleNamespace(
                    latest_prices={},
                    equity=1_000.0,
                    realized_pnl=0.0,
                    unrealized_pnl=0.0,
                    processed_bars=0,
                    portfolio=SimpleNamespace(cash={}, positions={}, orders=[]),
                )

        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(
                LiveMarketFeed=Feed,
                SessionConfig=lambda **_values: object(),
                Session=Session,
            ),
        )
        manager = LiveTradingManager()

        manager.start({"provider": "binance", "symbols": ["BTC-USDT"]})
        assert failed.wait(timeout=1.0)
        thread = manager._thread
        assert thread is not None
        thread.join(timeout=1.0)
        status = manager.status()

        assert status["status"] == "error"
        assert status["error"] == "mock socket disconnected"

    def test_worker_initialization_failure_updates_the_running_manifest(
        self,
        monkeypatch,
    ):
        """A feed that fails in the worker cannot leave a running history entry."""
        failed = threading.Event()

        class Feed:
            creations = 0

            def __init__(self, _provider, _symbols, _interval, *, include_partial=True):
                self.include_partial = include_partial
                Feed.creations += 1
                if Feed.creations == 2:
                    failed.set()
                    raise RuntimeError("mock connection failed")

            def cancel(self):
                return None

        class Session:
            def __init__(self, _config, _strategy):
                pass

            @staticmethod
            def snapshot():
                return None

        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(
                LiveMarketFeed=Feed,
                SessionConfig=lambda **_values: object(),
                Session=Session,
            ),
        )
        manager = LiveTradingManager()

        manager.start({"provider": "binance", "symbols": ["BTC-USDT"]})
        assert failed.wait(timeout=1.0)
        thread = manager._thread
        assert thread is not None
        thread.join(timeout=1.0)

        status = manager.status()
        assert status["status"] == "error"
        assert status["error"] == "mock connection failed"
        assert manager._session_id is not None
        manifest = manager.session(manager._session_id)
        assert manifest["status"] == "error"
        assert manifest["finished_at"] is not None
        manager.delete_session(manager._session_id)

    def test_combines_independent_strategy_accounts_with_fill_attribution(
        self,
        monkeypatch,
    ):
        """Multiple strategies keep separate accounts and expose attributed fills."""
        market = SimpleNamespace(
            symbol="BTC-USD",
            interval="1m",
            open_ts=1_700_000_000,
            close_ts=1_700_000_060,
            open=100.0,
            high=100.0,
            low=100.0,
            close=100.0,
            volume=10.0,
            n_trades=1,
            is_final=True,
            provider="mock",
            received_ts=1_700_000_061,
        )

        def snapshot(equity):
            return SimpleNamespace(
                latest_prices={"BTC-USD": 100.0},
                equity=equity,
                realized_pnl=equity - 1_000.0,
                unrealized_pnl=0.0,
                processed_bars=1,
                gross_exposure=100.0,
                net_exposure=100.0,
                leverage=0.1,
                buying_power=900.0,
                drawdown=0.0,
                peak_equity=equity,
                total_costs=0.0,
                trading_halted=False,
                halt_reason=None,
                metrics={"pnl": equity - 1_000.0},
                portfolio=SimpleNamespace(
                    cash={"USD": equity - 100.0},
                    positions={"BTC-USD": 1.0},
                    orders=[],
                ),
            )

        class Session:
            def __init__(self, name, equity):
                self.name = name
                self.value = snapshot(equity)

            def on_bar(self, _market, _orders):
                order = SimpleNamespace(
                    id=f"{self.name}-order",
                    symbol="BTC-USD",
                    order_type="Market",
                    quantity=1.0,
                    price=None,
                    limit_price=None,
                )
                fill = SimpleNamespace(
                    order=order,
                    timestamp=1_700_000_060,
                    status="Filled",
                    fill_price=100.0,
                    commission=0.0,
                    realized_pnl=0.0,
                    reason="test fill",
                )
                return SimpleNamespace(
                    market=market,
                    fills=[fill],
                    orders_submitted=1,
                    processed=True,
                    snapshot=self.value,
                    indicators={},
                )

            def snapshot(self):
                return self.value

        manager = LiveTradingManager()
        manager._sessions = {
            "Momentum": Session("momentum", 1_010.0),
            "Mean reversion": Session("mean", 990.0),
        }
        manager._session = manager._sessions["Momentum"]
        monkeypatch.setattr(
            manager,
            "_now",
            lambda: "2026-08-13T10:12:58.123456+00:00",
        )
        manager._prepare_session()

        manager._process_market(market)
        status = manager.status()

        assert status["snapshot"]["equity"] == 2_000.0
        assert set(status["strategies"]) == {"Momentum", "Mean reversion"}
        assert {fill["strategy"] for fill in status["updates"][0]["fills"]} == {
            "Momentum",
            "Mean reversion",
        }
        assert set(status["recent_order_outcomes"]) == {"Momentum", "Mean reversion"}
        assert status["recent_order_outcomes"]["Momentum"][0]["order"]["id"] == ("momentum-order")
        assert status["updates"][0]["received_at"] == "2026-08-13T10:12:58.123456+00:00"

    def test_recent_order_outcomes_survive_market_update_eviction(self):
        """Completed orders remain visible independently of buffered market updates."""
        market = SimpleNamespace(
            symbol="BTC-USD",
            interval="1m",
            open_ts=1_700_000_000,
            close_ts=1_700_000_060,
            open=100.0,
            high=100.0,
            low=100.0,
            close=100.0,
            volume=10.0,
            n_trades=1,
            is_final=True,
            provider="mock",
            received_ts=1_700_000_061,
        )
        snapshot = SimpleNamespace(
            latest_prices={"BTC-USD": 100.0},
            equity=1_000.0,
            realized_pnl=0.0,
            unrealized_pnl=0.0,
            processed_bars=1,
            portfolio=SimpleNamespace(cash={"USD": 900.0}, positions={"BTC-USD": 1.0}, orders=[]),
        )

        class Session:
            calls = 0

            @classmethod
            def on_bar(cls, _market, _orders):
                cls.calls += 1
                fills = []
                if cls.calls == 1:
                    fills = [
                        SimpleNamespace(
                            order=SimpleNamespace(
                                id="buy-order",
                                symbol="BTC-USD",
                                order_type="Market",
                                quantity=1.0,
                                price=None,
                                limit_price=None,
                            ),
                            timestamp=market.close_ts,
                            status="Filled",
                            fill_price=100.0,
                            commission=0.0,
                            realized_pnl=0.0,
                            reason="test fill",
                        )
                    ]
                return SimpleNamespace(
                    market=market,
                    fills=fills,
                    orders_submitted=len(fills),
                    processed=True,
                    snapshot=snapshot,
                    indicators={},
                )

            @staticmethod
            def snapshot():
                return snapshot

        manager = LiveTradingManager()
        manager._sessions = {"Buy & Hold": Session()}
        manager._session = manager._sessions["Buy & Hold"]
        manager._prepare_session()

        for _ in range(501):
            manager._process_market(market)

        status = manager.status()

        assert len(status["updates"]) == 500
        assert all(not update["fills"] for update in status["updates"])
        outcomes = status["recent_order_outcomes"]["Buy & Hold"]
        assert len(outcomes) == 1
        assert outcomes[0]["order"]["id"] == "buy-order"

    def test_native_order_values_are_json_safe_in_session_journal(self):
        """Native order enums and resting orders are normalized before persistence."""
        from backtide.backtest import Order, OrderStatus, OrderType

        market = SimpleNamespace(
            symbol="BTC-USD",
            interval="1m",
            open_ts=1_700_000_000,
            close_ts=1_700_000_060,
            open=100.0,
            high=102.0,
            low=99.0,
            close=101.0,
            volume=10.0,
            n_trades=1,
            is_final=True,
            provider="mock",
            received_ts=1_700_000_061,
        )
        order = Order("BTC-USD", 1.0, OrderType.Limit, price=100.0)
        snapshot = SimpleNamespace(
            latest_prices={"BTC-USD": 101.0},
            equity=1_001.0,
            realized_pnl=1.0,
            unrealized_pnl=0.0,
            processed_bars=1,
            portfolio=SimpleNamespace(
                cash={"USD": 900.0},
                positions={"BTC-USD": 1.0},
                orders=[order],
            ),
        )
        update = SimpleNamespace(
            market=market,
            fills=[
                SimpleNamespace(
                    order=order,
                    timestamp=market.close_ts,
                    status=OrderStatus.Filled,
                    fill_price=100.0,
                    commission=0.1,
                    realized_pnl=0.0,
                    reason="test fill",
                )
            ],
            orders_submitted=1,
            processed=True,
            snapshot=snapshot,
            indicators={},
        )
        manager = LiveTradingManager()
        manager._prepare_session()
        manager._config = {"mode": "live"}
        manager._session = SimpleNamespace(snapshot=lambda: snapshot)

        serialized = manager._serialize_update(update)
        manager._append_event(serialized)
        manager._persist_manifest("stopped")

        assert manager._session_id is not None
        persisted = manager.session(manager._session_id)["updates"][0]
        assert persisted["fills"][0]["status"] == str(OrderStatus.Filled)
        assert persisted["fills"][0]["order"]["order_type"] == str(OrderType.Limit)
        assert persisted["snapshot"]["portfolio"]["orders"][0]["order_type"] == str(
            OrderType.Limit
        )
        manager.delete_session(manager._session_id)

    def test_persists_and_reads_a_replayable_session_journal(self):
        """Session manifests and exact market events survive manager recreation."""
        manager = LiveTradingManager()
        manager._prepare_session()
        manager._config = {"mode": "live", "provider": "mock", "symbols": ["BTC-USD"]}
        manager._session = SimpleNamespace(snapshot=lambda: None)
        update = {
            "market": {"symbol": "BTC-USD", "close": 101.0},
            "fills": [],
            "snapshot": {
                "equity": 1_001.0,
                "metrics": {"custom_score": 4.25},
            },
        }

        manager._append_event(update)
        manager._persist_warmup(
            [
                SimpleNamespace(
                    symbol="BTC-USD",
                    close=99.0,
                    close_ts=1_700_000_000,
                )
            ]
        )
        manager._persist_manifest("stopped")

        sessions = LiveTradingManager().sessions()
        assert manager._session_id is not None
        restored = LiveTradingManager().session(manager._session_id)
        assert any(value["id"] == manager._session_id for value in sessions)
        assert restored["status"] == "stopped"
        assert restored["updates"] == [update]
        assert restored["updates"][0]["snapshot"]["metrics"]["custom_score"] == 4.25
        assert restored["warmup"][0]["symbol"] == "BTC-USD"
        manager.delete_session(manager._session_id)

    def test_deletes_an_inactive_persisted_session(self):
        """Deleting a stopped session removes its manifest and recorded events."""
        manager = LiveTradingManager()
        manager._prepare_session()
        manager._config = {"mode": "live"}
        manager._session = SimpleNamespace(snapshot=lambda: None)
        manager._persist_manifest("stopped")
        assert manager._session_id is not None

        result = manager.delete_session(manager._session_id)

        assert result == {"deleted": 1}
        with pytest.raises(APIError, match="was not found"):
            manager.session(manager._session_id)

    def test_rejects_deleting_the_active_session(self):
        """An active worker keeps ownership of its persisted session rows."""
        manager = LiveTradingManager()
        manager._prepare_session()
        manager._config = {"mode": "live"}
        manager._session = SimpleNamespace(snapshot=lambda: None)
        manager._persist_manifest("running")
        assert manager._session_id is not None
        manager._thread = threading.current_thread()

        with pytest.raises(APIError, match="Stop the active live session") as error:
            manager.delete_session(manager._session_id)

        assert error.value.status == 409
        assert manager.session(manager._session_id)["status"] == "running"
        manager._thread = None
        manager.delete_session(manager._session_id)

    def test_rejects_an_invalid_session_id_before_deletion(self):
        """Session deletion rejects malformed database identifiers."""
        manager = LiveTradingManager()

        with pytest.raises(APIError, match="Live session id is invalid") as error:
            manager.delete_session("../outside")

        assert error.value.status == 400

    @pytest.mark.parametrize("status", ["running", "paused"])
    def test_session_history_closes_orphaned_active_manifests(
        self,
        monkeypatch,
        status,
    ):
        """History repairs active states that have no live worker in this process."""
        manager = LiveTradingManager()
        manager._prepare_session()
        manager._started_at = "2026-08-13T10:00:00+00:00"
        manager._config = {"mode": "live"}
        manager._session = SimpleNamespace(snapshot=lambda: None)
        manager._persist_manifest(status)
        assert manager._session_id is not None
        monkeypatch.setattr(manager, "_now", lambda: "2026-08-13T10:30:00+00:00")

        sessions = manager.sessions()
        stored = next(value for value in sessions if value["id"] == manager._session_id)

        assert stored["status"] == "stopped"
        assert stored["finished_at"] == "2026-08-13T10:30:00+00:00"
        persisted = manager.session(manager._session_id)
        assert persisted["status"] == "stopped"
        assert persisted["finished_at"] == "2026-08-13T10:30:00+00:00"
        manager.delete_session(manager._session_id)

    def test_session_history_preserves_the_current_active_manifest(self):
        """The worker-owned manifest remains active and offers an Open action."""
        manager = LiveTradingManager()
        manager._prepare_session()
        manager._started_at = "2026-08-13T10:00:00+00:00"
        manager._config = {"mode": "live"}
        manager._session = SimpleNamespace(snapshot=lambda: None)
        manager._persist_manifest("running")
        assert manager._session_id is not None
        manager._thread = threading.current_thread()

        sessions = manager.sessions()
        stored = next(value for value in sessions if value["id"] == manager._session_id)

        assert stored["status"] == "running"
        assert stored["finished_at"] is None
        persisted = manager.session(manager._session_id)
        assert persisted["status"] == "running"
        manager._thread = None
        manager.delete_session(manager._session_id)

    def test_pause_and_resume_update_observable_state(self):
        """Pause control is reflected in health without discarding the session."""
        manager = LiveTradingManager()
        manager._session = SimpleNamespace(snapshot=lambda: None)

        manager.pause()
        assert manager.status()["health"]["paused"] is True
        manager.resume()
        assert manager.status()["health"]["paused"] is False

    def test_flatten_remains_pending_until_a_market_update_is_processed(
        self,
        monkeypatch,
    ):
        """A partial candle cannot consume a flatten request that it does not process."""

        class Order:
            def __init__(self, symbol, quantity, order_type):
                self.symbol = symbol
                self.quantity = quantity
                self.order_type = order_type

        market = SimpleNamespace(
            symbol="BTC-USD",
            interval="1m",
            open_ts=1_700_000_000,
            close_ts=1_700_000_060,
            open=100.0,
            high=102.0,
            low=99.0,
            close=101.0,
            volume=10.0,
            n_trades=1,
            is_final=False,
            provider="mock",
            received_ts=1_700_000_030,
        )
        snapshot = SimpleNamespace(
            latest_prices={"BTC-USD": 101.0},
            equity=1_001.0,
            realized_pnl=0.0,
            unrealized_pnl=1.0,
            processed_bars=1,
            portfolio=SimpleNamespace(
                cash={"USD": 900.0},
                positions={"BTC-USD": 1.0},
                orders=[],
            ),
        )

        class Session:
            def __init__(self):
                self.submissions = []

            def snapshot(self):
                return snapshot

            def on_bar(self, update, orders):
                self.submissions.append(orders)
                return SimpleNamespace(
                    market=update,
                    fills=[],
                    orders_submitted=len(orders or []),
                    processed=update.is_final,
                    snapshot=snapshot,
                    indicators={},
                )

        monkeypatch.setitem(sys.modules, "backtide.backtest", SimpleNamespace(Order=Order))
        session = Session()
        manager = LiveTradingManager()
        manager._sessions = {"Buy & Hold": session}
        manager._session = session
        manager._prepare_session()
        manager.flatten()

        manager._process_market(market)

        assert manager._flatten_requested is True
        assert session.submissions[0][0].quantity == -1.0

        market.is_final = True
        market.received_ts = market.close_ts
        manager._process_market(market)

        assert manager._flatten_requested is False
        assert session.submissions[1][0].quantity == -1.0
