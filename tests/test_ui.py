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

import pytest

from backtide.ui.live import LiveTradingManager
from backtide.ui.server import create_server
from backtide.ui.services import APIError, BacktideServices, JobStore, dataframe_records


class StubServices(BacktideServices):
    """Provide deterministic route responses without storage or provider access."""

    def bootstrap(self):
        """Return a minimal boot payload."""
        return {"defaults": {}, "enums": {}, "strategies": {}, "indicators": {}}

    def dashboard(self):
        """Return deterministic dashboard content."""
        return {"metrics": {"experiments": 2}}

    def update_strategy(self, original_name, payload):
        """Return a deterministic saved strategy update."""
        return {"original": original_name, "saved": payload["name"]}


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


class TestJobStore:
    """Tests for bounded background work snapshots."""

    def test_successful_job_exposes_result(self):
        """Completed work transitions to success and keeps its result."""
        jobs = JobStore()
        job = jobs.start("test", lambda: {"value": 42})

        for _ in range(1000):
            snapshot = jobs.get(job["id"])
            if snapshot["status"] == "success":
                break

        assert snapshot["result"] == {"value": 42}

    def test_failed_job_exposes_safe_error(self):
        """A worker exception is captured without losing the job."""
        jobs = JobStore()

        def fail():
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


class TestServiceCommands:
    """Tests for command validation and backend dispatch."""

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
        (experiment / "config.toml").write_text("[general]\nname='test'", encoding="utf-8")
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

        def query_strategy_runs(experiment_id, *, include_equity_curve=True):
            captured.update(
                experiment_id=experiment_id,
                include_equity_curve=include_equity_curve,
            )
            return [run]

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
            ),
        )

        result = BacktideServices().experiment("exp-1")

        assert captured == {"experiment_id": "exp-1", "include_equity_curve": False}
        assert "equity_curve" not in result["runs"][0]
        assert result["runs"][0]["metrics"] == {"return": 0.1}

    def test_result_plot_dispatches_to_existing_analysis(self, monkeypatch, tmp_path: Path):
        """Result plot requests use the public Plotly analysis function."""
        import backtide

        experiment = tmp_path / "experiments" / "exp-1"
        experiment.mkdir(parents=True)
        (experiment / "config.toml").write_text("[general]\nname='test'", encoding="utf-8")
        run = SimpleNamespace(strategy_id="run-1")
        captured = {"queries": 0, "plots": []}

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

        def query_strategy_runs(_experiment_id):
            captured["queries"] += 1
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
            "queries": 1,
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
                query_strategy_runs=lambda _experiment_id: [SimpleNamespace(strategy_id="run-1")],
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

        def provider_support(provider, interval):
            supported = provider != "yahoo" and (provider != "coinbase" or interval == "5m")
            return supported, "Supported." if supported else "Unsupported interval."

        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(provider_live_support=provider_support),
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
        monkeypatch.setitem(
            sys.modules,
            "backtide.live",
            SimpleNamespace(
                PaperTradingConfig=object,
                PaperTradingSession=object,
                provider_live_support=lambda _provider, _interval: (
                    False,
                    "No WebSocket feed.",
                ),
            ),
        )

        with pytest.raises(APIError, match="No WebSocket feed"):
            LiveTradingManager().start(
                {"provider": "yahoo", "interval": "1m", "symbols": ["AAPL"]}
            )

    def test_stop_without_session_is_idempotent(self):
        """Stopping before start leaves the manager in an idle state."""
        assert LiveTradingManager().stop()["status"] == "idle"
