"""Backtide.

Author: Mavs
Description: Dependency-free local HTTP server for the Backtide web app.

"""

from __future__ import annotations

from collections.abc import Callable
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import logging
import mimetypes
from pathlib import Path
import threading
from typing import Any
from urllib.parse import parse_qs, unquote, urlsplit
import webbrowser

from backtide.ui.services import APIError, BacktideServices, json_bytes

STATIC_ROOT = Path(__file__).resolve().parent / "static"
logger = logging.getLogger(__name__)


class BacktideRequestHandler(BaseHTTPRequestHandler):
    """Serve the SPA and its same-origin JSON API."""

    server_version = "Backtide/1"

    @property
    def services(self) -> BacktideServices:
        """Return the service facade attached to the HTTP server."""
        if not isinstance(self.server, BacktideHTTPServer):
            raise RuntimeError("Backtide request handler is attached to an invalid server")
        return self.server.services

    def do_GET(self) -> None:
        """Handle JSON reads and static assets."""
        split = urlsplit(self.path)
        path = split.path.rstrip("/") or "/"
        query = parse_qs(split.query)
        try:
            if path == "/api/health":
                self._json({"status": "ok"})
            elif path == "/api/bootstrap":
                self._json(self.services.bootstrap())
            elif path == "/api/dashboard":
                self._json(self.services.dashboard())
            elif path == "/api/instruments":
                self._json(
                    self.services.instruments(
                        self._first(query, "instrument_type"),
                        self._first(query, "provider"),
                        self._first(query, "search"),
                        int(self._first(query, "limit") or 2_000),
                        catalog=self._first(query, "source") == "catalog",
                    )
                )
            elif path == "/api/live/instruments":
                self._json(
                    self.services.live_instruments(
                        self._first(query, "provider") or "kraken",
                        int(self._first(query, "limit") or 10_000),
                    )
                )
            elif path == "/api/live/sessions":
                self._json(self.services.live_sessions())
            elif path.startswith("/api/live/sessions/"):
                self._json(self.services.live_session(unquote(path.rsplit("/", 1)[1])))
            elif path == "/api/bars":
                self._json(
                    self.services.bars(
                        query.get("symbol", []),
                        self._first(query, "interval"),
                        self._first(query, "provider"),
                        int(self._first(query, "limit") or 50_000),
                    )
                )
            elif path == "/api/storage":
                self._json(self.services.storage())
            elif path == "/api/experiments":
                self._json(
                    self.services.experiments(
                        self._first(query, "search"),
                        int(self._first(query, "limit") or 100),
                        int(self._first(query, "offset") or 0),
                    )
                )
            elif path.startswith("/api/experiments/") and path.endswith("/logs"):
                experiment_id = unquote(path[len("/api/experiments/") : -len("/logs")])
                filename, body = self.services.experiment_log(experiment_id)
                self._download(filename, body)
            elif path.startswith("/api/experiments/") and path.endswith("/orders"):
                experiment_id = unquote(path[len("/api/experiments/") : -len("/orders")])
                self._json(
                    self.services.experiment_orders(
                        experiment_id,
                        self._first(query, "strategy_id"),
                        int(self._first(query, "offset") or 0),
                        int(self._first(query, "limit") or 100),
                    )
                )
            elif path.startswith("/api/experiments/") and path.endswith("/paper-config"):
                experiment_id = unquote(path[len("/api/experiments/") : -len("/paper-config")])
                self._json(self.services.paper_config_from_experiment(experiment_id))
            elif path.startswith("/api/experiments/"):
                self._json(self.services.experiment(unquote(path.rsplit("/", 1)[1])))
            elif path == "/api/jobs":
                self._json(self.services.jobs.list_jobs())
            elif path.startswith("/api/jobs/"):
                self._json(self.services.jobs.get(unquote(path.rsplit("/", 1)[1])))
            elif path == "/api/strategies":
                self._json(self.services.strategy_catalog())
            elif path == "/api/indicators":
                self._json(self.services.indicator_catalog())
            elif path == "/api/metrics":
                self._json(self.services.metric_catalog())
            elif path == "/api/sizers":
                self._json(self.services.sizer_catalog())
            elif path == "/api/live":
                self._json(self.services.live_status())
            else:
                self._static(split.path)
        except APIError as exc:
            self._error(exc.status, str(exc))
        except (TypeError, ValueError) as exc:
            self._error(HTTPStatus.BAD_REQUEST, str(exc))
        except Exception as exc:
            logger.exception("Unhandled Backtide API read error: %s", exc)
            self._error(HTTPStatus.INTERNAL_SERVER_ERROR, "Internal server error.")

    def do_POST(self) -> None:
        """Handle commands that create work or persisted entities."""
        path = urlsplit(self.path).path.rstrip("/")
        try:
            body = self._body()
            routes: dict[str, Callable[[dict[str, Any]], Any]] = {
                "/api/downloads": self.services.start_download,
                "/api/downloads/plan": self.services.download_plan,
                "/api/experiments": self.services.start_experiment,
                "/api/analysis": lambda value: self.services.analysis_plot(
                    str(value.pop("plot", "")), value
                ),
                "/api/results/plot": self.services.result_plot,
                "/api/config/parse": self.services.parse_experiment_config,
                "/api/strategies": self.services.save_strategy,
                "/api/indicators": self.services.save_indicator,
                "/api/metrics": self.services.save_metric,
                "/api/sizers": self.services.save_sizer,
                "/api/live": self.services.start_live,
                "/api/live/replay": self.services.replay_live,
            }
            if path == "/api/experiments/abort":
                result = self.services.abort_experiment()
            elif path == "/api/live/stop":
                result = self.services.stop_live()
            elif path == "/api/live/pause":
                result = self.services.pause_live()
            elif path == "/api/live/resume":
                result = self.services.resume_live()
            elif path == "/api/live/flatten":
                result = self.services.flatten_live()
            elif path == "/api/live/cancel-all":
                result = self.services.cancel_live_orders()
            elif path not in routes:
                raise APIError("Endpoint not found.", 404)
            else:
                result = routes[path](body)
            accepted = path in {"/api/downloads", "/api/experiments"}
            self._json(result, HTTPStatus.ACCEPTED if accepted else HTTPStatus.OK)
        except APIError as exc:
            self._error(exc.status, str(exc))
        except (TypeError, ValueError) as exc:
            self._error(HTTPStatus.BAD_REQUEST, str(exc))
        except Exception as exc:
            logger.exception("Unhandled Backtide API command error: %s", exc)
            self._error(HTTPStatus.INTERNAL_SERVER_ERROR, "Internal server error.")

    def do_DELETE(self) -> None:
        """Handle deletion of persisted records, stored data, and user code."""
        path = urlsplit(self.path).path.rstrip("/")
        try:
            if path == "/api/storage":
                result = self.services.delete_storage(self._body())
            elif path.startswith("/api/live/sessions/"):
                result = self.services.delete_live_session(unquote(path.rsplit("/", 1)[1]))
            elif path.startswith("/api/experiments/"):
                result = self.services.delete_experiment(unquote(path.rsplit("/", 1)[1]))
            elif path.startswith("/api/strategies/"):
                result = self.services.delete_strategy(unquote(path.rsplit("/", 1)[1]))
            elif path.startswith("/api/indicators/"):
                result = self.services.delete_indicator(unquote(path.rsplit("/", 1)[1]))
            elif path.startswith("/api/metrics/"):
                result = self.services.delete_metric(unquote(path.rsplit("/", 1)[1]))
            elif path.startswith("/api/sizers/"):
                result = self.services.delete_sizer(unquote(path.rsplit("/", 1)[1]))
            else:
                raise APIError("Endpoint not found.", 404)
            self._json(result)
        except APIError as exc:
            self._error(exc.status, str(exc))
        except Exception as exc:
            logger.exception("Unhandled Backtide API deletion error: %s", exc)
            self._error(HTTPStatus.INTERNAL_SERVER_ERROR, "Internal server error.")

    def do_PUT(self) -> None:
        """Handle updates to persisted user strategies and indicators."""
        path = urlsplit(self.path).path.rstrip("/")
        try:
            body = self._body()
            if path.startswith("/api/strategies/"):
                result = self.services.update_strategy(unquote(path.rsplit("/", 1)[1]), body)
            elif path.startswith("/api/indicators/"):
                result = self.services.update_indicator(unquote(path.rsplit("/", 1)[1]), body)
            elif path.startswith("/api/metrics/"):
                result = self.services.update_metric(unquote(path.rsplit("/", 1)[1]), body)
            elif path.startswith("/api/sizers/"):
                result = self.services.update_sizer(unquote(path.rsplit("/", 1)[1]), body)
            else:
                raise APIError("Endpoint not found.", 404)
            self._json(result)
        except APIError as exc:
            self._error(exc.status, str(exc))
        except (TypeError, ValueError) as exc:
            self._error(HTTPStatus.BAD_REQUEST, str(exc))
        except Exception as exc:
            logger.exception("Unhandled Backtide API update error: %s", exc)
            self._error(HTTPStatus.INTERNAL_SERVER_ERROR, "Internal server error.")

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A002
        """Silence routine browser polling while preserving server errors."""
        if args and str(args[1]).startswith("5"):
            super().log_message(format, *args)

    def _body(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        if length > 2_000_000:
            raise APIError("Request body is too large.", 413)
        if not length:
            return {}
        import json

        value = json.loads(self.rfile.read(length))
        if not isinstance(value, dict):
            raise APIError("Request body must be a JSON object.")
        return value

    def _static(self, raw_path: str) -> None:
        relative = unquote(raw_path).lstrip("/") or "index.html"
        if relative.startswith(("assets/", "providers/")) or relative in {
            "backtide-logo.png",
            "favicon.svg",
            "index.html",
        }:
            path = (STATIC_ROOT / relative).resolve()
            if STATIC_ROOT.resolve() not in path.parents and path != STATIC_ROOT.resolve():
                raise APIError("Asset not found.", 404)
            if not path.is_file():
                raise APIError("Asset not found.", 404)
        else:
            path = STATIC_ROOT / "index.html"
        content_type, _ = mimetypes.guess_type(path.name)
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", content_type or "application/octet-stream")
        self.send_header("Content-Length", str(path.stat().st_size))
        self.send_header(
            "Cache-Control",
            "no-cache" if path.name == "index.html" else "public, max-age=31536000, immutable",
        )
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "same-origin")
        self.end_headers()
        self.wfile.write(path.read_bytes())

    def _json(self, value: Any, status: int = HTTPStatus.OK) -> None:
        body = json_bytes(value)
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(body)

    def _download(self, filename: str, body: bytes) -> None:
        """Return a complete plain-text artifact as a browser download."""
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Content-Disposition", f'attachment; filename="{filename}"')
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(body)

    def _error(self, status: int, message: str) -> None:
        self._json({"error": message}, status)

    @staticmethod
    def _first(query: dict[str, list[str]], name: str) -> str | None:
        values = query.get(name)
        return values[0] if values else None


class BacktideHTTPServer(ThreadingHTTPServer):
    """HTTP server carrying a shared Backtide service facade."""

    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, address: tuple[str, int], services: BacktideServices | None = None):
        self.services = services or BacktideServices()
        super().__init__(address, BacktideRequestHandler)


def create_server(
    address: str = "localhost",
    port: int = 8501,
    services: BacktideServices | None = None,
) -> BacktideHTTPServer:
    """Create a configured local Backtide HTTP server."""
    return BacktideHTTPServer((address, port), services)


def launch(
    address: str = "localhost",
    port: int = 8501,
    *,
    open_browser: bool = True,
) -> None:
    """Serve Backtide until interrupted and optionally open the browser."""
    server = create_server(address, port)
    display_host = "localhost" if address in {"0.0.0.0", "::"} else address
    url = f"http://{display_host}:{server.server_port}"
    if open_browser:
        threading.Timer(0.35, lambda: webbrowser.open(url)).start()
    try:
        server.serve_forever(poll_interval=0.25)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
