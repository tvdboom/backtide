"""Backtide.

Author: Mavs
Description: Application services exposed by the local web interface.

"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from dataclasses import asdict, is_dataclass
from datetime import UTC, date, datetime
import inspect
import json
import math
from pathlib import Path
import re
import threading
import tomllib
from typing import Any
import uuid


class APIError(Exception):
    """Error that can be returned safely to an API client."""

    def __init__(self, message: str, status: int = 400):
        super().__init__(message)
        self.status = status


def json_default(value: Any) -> Any:
    """Convert Backtide, dataframe and scalar values to JSON-compatible data."""
    if is_dataclass(value):
        return asdict(value)
    if isinstance(value, (date, datetime)):
        return value.isoformat()
    if isinstance(value, Path):
        return str(value)
    if hasattr(value, "item"):
        return value.item()
    if hasattr(value, "to_dict"):
        return value.to_dict()
    if hasattr(value, "name") and not hasattr(value, "__dict__"):
        return str(value)
    if hasattr(value, "__dict__"):
        return {key: item for key, item in vars(value).items() if not key.startswith("_")}
    return str(value)


def json_bytes(value: Any) -> bytes:
    """Serialize an API response to UTF-8 JSON."""
    return json.dumps(
        value,
        default=json_default,
        allow_nan=False,
        separators=(",", ":"),
    ).encode()


def _clean(value: Any) -> Any:
    """Recursively replace non-finite numeric values before JSON encoding."""
    if isinstance(value, float) and not math.isfinite(value):
        return None
    if isinstance(value, dict):
        return {str(key): _clean(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_clean(item) for item in value]
    return value


def dataframe_records(data: Any) -> list[dict[str, Any]]:
    """Return records from either a pandas or Polars dataframe."""
    if data is None:
        return []
    if hasattr(data, "to_dicts"):
        rows = data.to_dicts()
    elif hasattr(data, "to_dict"):
        rows = data.to_dict(orient="records")
    else:
        rows = list(data)
    return [_clean(dict(row)) for row in rows]


def public_attributes(value: Any, names: tuple[str, ...]) -> dict[str, Any]:
    """Select public attributes from a Rust-backed Python object."""
    return {name: _clean(getattr(value, name, None)) for name in names if hasattr(value, name)}


class JobStore:
    """Manage bounded background jobs for downloads and backtests."""

    def __init__(self, max_completed: int = 40):
        self._jobs: dict[str, dict[str, Any]] = {}
        self._max_completed = max_completed
        self._lock = threading.RLock()

    def start(self, kind: str, work: Callable[[], Any]) -> dict[str, Any]:
        """Start work in a daemon thread and return its initial snapshot."""
        job_id = uuid.uuid4().hex[:16]
        job = {
            "id": job_id,
            "kind": kind,
            "status": "queued",
            "created_at": datetime.now().astimezone().isoformat(),
            "result": None,
            "error": None,
        }
        with self._lock:
            self._jobs[job_id] = job

        def runner() -> None:
            with self._lock:
                job["status"] = "running"
                job["started_at"] = datetime.now().astimezone().isoformat()
            try:
                result = work()
            except Exception as exc:  # noqa: BLE001
                with self._lock:
                    job["status"] = "error"
                    job["error"] = str(exc)
            else:
                with self._lock:
                    job["status"] = "success"
                    job["result"] = _clean(result)
            finally:
                with self._lock:
                    job["finished_at"] = datetime.now().astimezone().isoformat()
                    self._trim()

        threading.Thread(target=runner, name=f"backtide-{kind}-{job_id}", daemon=True).start()
        return dict(job)

    def get(self, job_id: str) -> dict[str, Any]:
        """Return a copy of a job snapshot."""
        with self._lock:
            if job_id not in self._jobs:
                raise APIError("Job not found.", 404)
            return dict(self._jobs[job_id])

    def list_jobs(self) -> list[dict[str, Any]]:
        """Return jobs in newest-first order."""
        with self._lock:
            return [dict(job) for job in reversed(self._jobs.values())]

    def _trim(self) -> None:
        completed = [
            key for key, job in self._jobs.items() if job["status"] in {"success", "error"}
        ]
        for key in completed[: -self._max_completed]:
            self._jobs.pop(key, None)


class BacktideServices:
    """Facade over Backtide's data, strategy and simulation APIs."""

    instrument_fields = (
        "symbol",
        "name",
        "base",
        "quote",
        "instrument_type",
        "exchange",
        "provider",
    )
    run_fields = (
        "strategy_id",
        "strategy_name",
        "base_currency",
        "is_benchmark",
        "metrics",
        "error",
    )

    def __init__(self, jobs: JobStore | None = None):
        self.jobs = jobs or JobStore()
        self._result_runs_cache: tuple[str, list[Any]] | None = None
        self._result_runs_lock = threading.Lock()

    def bootstrap(self) -> dict[str, Any]:
        """Return configuration and catalog data needed to initialize the SPA."""
        from backtide.backtest import (
            CommissionType,
            ConversionPeriod,
            CurrencyConversionMode,
            EmptyBarPolicy,
            ExperimentConfig,
            OrderType,
        )
        from backtide.config import get_config
        from backtide.data import Currency, InstrumentType, Interval, Provider

        cfg = get_config()
        defaults = ExperimentConfig().to_dict()
        defaults["portfolio"]["base_currency"] = str(cfg.general.base_currency)
        return {
            "defaults": defaults,
            "enums": {
                "instrument_types": self._variants(InstrumentType),
                "intervals": self._variants(Interval),
                "providers": self._variants(Provider),
                "commission_types": self._variants(CommissionType),
                "conversion_modes": self._variants(CurrencyConversionMode),
                "conversion_periods": self._variants(ConversionPeriod),
                "empty_bar_policies": self._variants(EmptyBarPolicy),
                "order_types": self._variants(OrderType),
                "currencies": self._currency_options(Currency),
            },
            "display": {
                "currency_prefix": bool(cfg.display.currency_prefix),
                "dataframe_class": cfg.data.dataframe_library.class_name,
                "logokit_api_key": getattr(cfg.display, "logokit_api_key", None),
                "timezone": str(cfg.display.timezone),
                "date_format": str(cfg.display.date_format),
                "datetime_format": str(cfg.display.datetime_format()),
            },
            "strategies": self.strategy_catalog(),
            "indicators": self.indicator_catalog(),
            "metrics": self.metric_catalog(),
            "sizers": self.sizer_catalog(),
            "live": self.live_capabilities(),
        }

    def dashboard(self) -> dict[str, Any]:
        """Return recent workspace activity and local storage statistics."""
        from backtide.storage import query_bars_summary, query_experiments

        experiments = self.experiments(limit=6)
        sessions = self.live_sessions()
        storage = dataframe_records(query_bars_summary())
        symbols = {row.get("symbol") for row in storage if row.get("symbol")}
        bars = sum(int(row.get("n_rows") or row.get("rows") or 0) for row in storage)
        return {
            "experiments": experiments,
            "sessions": sessions[:6],
            "storage": storage[:8],
            "metrics": {
                "experiments": len(dataframe_records(query_experiments())),
                "sessions": len(sessions),
                "symbols": len(symbols),
                "bars": bars,
                "series": len(storage),
            },
        }

    def instruments(
        self,
        instrument_type: str | None = None,
        provider: str | None = None,
        search: str | None = None,
        limit: int = 2_000,
        *,
        catalog: bool = False,
    ) -> list[dict[str, Any]]:
        """Return searchable stored or provider-discovered instrument metadata."""
        if catalog:
            from backtide.data import list_instruments

            values = list_instruments(
                instrument_type or "stocks",
                limit=limit,
                verbose=False,
            )
        else:
            from backtide.storage import query_instruments

            values = query_instruments(instrument_type, provider, limit=limit)
        rows = [public_attributes(value, self.instrument_fields) for value in values]
        if search:
            needle = search.casefold()
            rows = [
                row
                for row in rows
                if needle in f"{row.get('symbol', '')} {row.get('name', '')}".casefold()
            ]
        return rows

    def live_instruments(
        self,
        provider: str,
        limit: int = 10_000,
    ) -> list[dict[str, Any]]:
        """Return the complete canonical spot catalog for one live provider."""
        from backtide.live import list_live_instruments

        values = list_live_instruments(provider, limit=min(max(limit, 1), 10_000))
        return [public_attributes(value, self.instrument_fields) for value in values]

    def bars(
        self,
        symbols: list[str],
        interval: str | None,
        provider: str | None,
        limit: int,
    ) -> list[dict[str, Any]]:
        """Return stored OHLCV data for browser-side analysis."""
        from backtide.storage import query_bars

        if not symbols:
            raise APIError("Select at least one symbol.")
        limit = min(max(limit, 1), 100_000)
        return dataframe_records(query_bars(symbols, interval, provider, limit=limit))

    def storage(self) -> list[dict[str, Any]]:
        """Return one summary record per stored market-data series."""
        from backtide.storage import query_bars_summary

        return dataframe_records(query_bars_summary())

    def delete_storage(self, payload: dict[str, Any]) -> dict[str, int]:
        """Delete selected stored market-data series."""
        from backtide.storage import delete_symbols

        series = payload.get("series")
        if series:
            cleaned = [tuple(item) for item in series]
            return {"deleted": delete_symbols(series=cleaned)}
        symbol = payload.get("symbol")
        if not symbol:
            raise APIError("A symbol or list of series is required.")
        return {
            "deleted": delete_symbols(
                symbol,
                payload.get("interval"),
                payload.get("provider"),
            )
        }

    def start_download(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Start a background market-data download."""
        symbols = [str(symbol).strip().upper() for symbol in payload.get("symbols", [])]
        symbols = [symbol for symbol in symbols if symbol]
        if not symbols:
            raise APIError("Select at least one symbol.")
        start = self._date_boundary(payload.get("start"), end=False)
        end = self._date_boundary(payload.get("end"), end=True)

        def work() -> dict[str, Any]:
            from backtide.data import download_bars, resolve_profiles

            profiles = resolve_profiles(
                symbols,
                payload.get("instrument_type", "stocks"),
                payload.get("intervals") or ["1d"],
                verbose=False,
            )
            result = download_bars(
                profiles,
                start=start,
                end=end,
                verbose=False,
            )
            return public_attributes(
                result,
                ("n_succeeded", "n_failed", "warnings"),
            )

        return self.jobs.start("download", work)

    def download_plan(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Resolve provider ranges and estimate the requested market-data download."""
        from backtide.config import get_config
        from backtide.data import resolve_profiles
        from backtide.utils.utils import _get_timezone

        symbols = [str(symbol).strip().upper() for symbol in payload.get("symbols", [])]
        symbols = [symbol for symbol in symbols if symbol]
        intervals = [str(interval) for interval in payload.get("intervals", []) if interval]
        if not symbols or not intervals:
            raise APIError("Select at least one symbol and interval.")

        try:
            profiles = resolve_profiles(
                symbols,
                payload.get("instrument_type", "stocks"),
                intervals,
                verbose=False,
            )
        except RuntimeError as exc:
            raise APIError(
                "Provider availability could not be resolved for this selection.", 422
            ) from exc
        direct_profiles = profiles[: len(symbols)]
        available = [
            (str(interval), int(start), int(profile.latest_ts[interval]))
            for profile in direct_profiles
            for interval, start in profile.earliest_ts.items()
            if str(interval) in intervals and interval in profile.latest_ts
        ]
        if not available:
            raise APIError("No provider ranges are available for this selection.")

        cfg = get_config()
        timezone = _get_timezone(cfg.display.timezone)
        today = datetime.now(tz=timezone).date()
        available_start = datetime.fromtimestamp(
            min(item[1] for item in available), timezone
        ).date()
        available_end = min(
            datetime.fromtimestamp(max(item[2] for item in available), timezone).date(),
            today,
        )
        full_history = bool(payload.get("full_history", True))
        requested_start = self._download_date(
            payload.get("start"), timezone, fallback=available_start
        )
        requested_end = self._download_date(payload.get("end"), timezone, fallback=available_end)

        details = []
        estimated_bars = 0
        series_count = 0
        for index, profile in enumerate(profiles):
            earliest_by_interval = {
                str(key): int(value) for key, value in profile.earliest_ts.items()
            }
            latest_by_interval = {str(key): int(value) for key, value in profile.latest_ts.items()}
            profile_intervals = []
            for interval in intervals:
                if interval not in earliest_by_interval or interval not in latest_by_interval:
                    continue
                available_interval_start = datetime.fromtimestamp(
                    earliest_by_interval[interval], timezone
                ).date()
                available_interval_end = min(
                    datetime.fromtimestamp(latest_by_interval[interval], timezone).date(),
                    today,
                )
                download_start = (
                    available_interval_start
                    if full_history
                    else max(available_interval_start, requested_start)
                )
                download_end = (
                    available_interval_end
                    if full_history
                    else min(available_interval_end, requested_end)
                )
                interval_model = next(key for key in profile.earliest_ts if str(key) == interval)
                bars = self._estimate_download_bars(
                    profile,
                    interval_model,
                    download_start,
                    download_end,
                )
                estimated_bars += bars
                profile_intervals.append(
                    {
                        "interval": interval,
                        "available_start": available_interval_start.isoformat(),
                        "available_end": available_interval_end.isoformat(),
                        "download_start": download_start.isoformat(),
                        "download_end": download_end.isoformat(),
                        "days": max((download_end - download_start).days + 1, 0),
                        "estimated_bars": bars,
                    }
                )

            series_count += len(profile_intervals)

            details.append(
                {
                    "symbol": str(profile.symbol),
                    "name": str(profile.name),
                    "instrument_type": str(profile.instrument_type),
                    "provider": str(profile.provider),
                    "exchange": str(profile.exchange),
                    "quote": str(profile.quote),
                    "legs": [str(leg) for leg in profile.legs],
                    "direct": index < len(symbols),
                    "intervals": profile_intervals,
                }
            )

        return {
            "available_start": available_start.isoformat(),
            "available_end": available_end.isoformat(),
            "profiles": details,
            "summary": {
                "estimated_bars": estimated_bars,
                "estimated_seconds": estimated_bars / 40_000,
                "estimated_bytes": estimated_bars * 120,
                "series": series_count,
            },
        }

    def experiments(
        self,
        search: str | None = None,
        limit: int = 100,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        """Return one page of persisted experiment summaries with lightweight run metrics."""
        from backtide.storage import query_experiments, query_strategy_runs

        bounded_limit = max(1, min(int(limit), 100))
        bounded_offset = max(0, int(offset))
        experiments = dataframe_records(
            query_experiments(search=search, limit=bounded_offset + bounded_limit)
        )[bounded_offset : bounded_offset + bounded_limit]
        from backtide.config import get_config

        experiment_root = Path(get_config().data.storage_path) / "experiments"
        metric_catalog = self.metric_catalog()
        for experiment in experiments:
            runs = query_strategy_runs(
                experiment["id"],
                include_equity_curve=False,
            )
            experiment["runs"] = [
                _clean(
                    public_attributes(
                        run,
                        (
                            "strategy_id",
                            "strategy_name",
                            "base_currency",
                            "is_benchmark",
                            "metrics",
                            "error",
                        ),
                    )
                )
                for run in runs
            ]
            config_text = self._read_text(
                experiment_root / experiment["id"] / "config.toml", max_bytes=500_000
            )
            self._apply_benchmark_display_name(config_text, experiment["runs"])
            config_metadata = self._experiment_config_metadata(config_text) or {}
            experiment["n_symbols"] = int(config_metadata.get("symbols", 0))
            experiment.update(
                self._primary_metric_summary(config_text, experiment["runs"], metric_catalog)
            )
        return experiments

    def experiment(self, experiment_id: str) -> dict[str, Any]:
        """Return experiment details without embedding large order histories."""
        from backtide.config import get_config
        from backtide.storage import query_experiments, query_strategy_runs

        rows = dataframe_records(query_experiments(experiment_id))
        if not rows:
            raise APIError("Experiment not found.", 404)
        runs = [
            self._serialize_run(run, include_orders=False)
            for run in query_strategy_runs(
                experiment_id,
                include_equity_curve=False,
            )
        ]
        root = Path(get_config().data.storage_path) / "experiments" / experiment_id
        config_text = self._read_text(root / "config.toml", max_bytes=500_000)
        log_text, logs_truncated = self._read_log_tail(
            root / "logs.txt",
            max_lines=1_000,
            max_bytes=200_000,
        )
        experiment = rows[0]
        self._apply_benchmark_display_name(config_text, runs)
        experiment.update(self._primary_metric_summary(config_text, runs))
        return {
            "experiment": experiment,
            "runs": runs,
            "config": config_text,
            "config_metadata": self._experiment_config_metadata(config_text),
            "logs": log_text,
            "logs_truncated": logs_truncated,
        }

    def experiment_orders(
        self,
        experiment_id: str,
        strategy_id: str | None,
        offset: int = 0,
        limit: int = 100,
    ) -> dict[str, Any]:
        """Return one newest-first page of orders for a strategy run."""
        if offset < 0:
            raise APIError("Order offset must be zero or greater.")
        if limit < 1 or limit > 500:
            raise APIError("Order limit must be between 1 and 500.")
        runs = self._query_result_runs(experiment_id)
        if not runs:
            raise APIError("Experiment runs were not found.", 404)
        run = self._select_run(runs, strategy_id)
        orders = sorted(
            run.orders,
            key=lambda item: self._order_timestamp(item.timestamp),
            reverse=True,
        )
        page = orders[offset : offset + limit]
        return {
            "orders": [self._serialize_order_record(order) for order in page],
            "offset": offset,
            "limit": limit,
            "total": len(orders),
            "has_more": offset + len(page) < len(orders),
        }

    def experiment_log(self, experiment_id: str) -> tuple[str, bytes]:
        """Return the complete experiment log and a safe download filename."""
        from backtide.config import get_config
        from backtide.storage import query_experiments

        rows = dataframe_records(query_experiments(experiment_id))
        if not rows:
            raise APIError("Experiment not found.", 404)
        experiment_root = (Path(get_config().data.storage_path) / "experiments").resolve()
        log_path = (experiment_root / experiment_id / "logs.txt").resolve()
        if experiment_root not in log_path.parents or not log_path.is_file():
            raise APIError("Experiment log not found.", 404)
        name = self._download_name(rows[0].get("name") or experiment_id)
        text = log_path.read_text(encoding="utf-8", errors="replace")
        return f"{name}-logs.txt", text.encode()

    def result_plot(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Build an existing experiment-result Plotly figure."""
        from backtide import analysis
        from backtide.backtest import ExperimentConfig
        from backtide.config import get_config
        from backtide.storage import query_bars

        experiment_id = str(payload.get("experiment_id") or "")
        plot_name = str(payload.get("plot") or "")
        runs = self._query_result_runs(experiment_id)
        if not runs:
            raise APIError("Experiment runs were not found.", 404)
        root = Path(get_config().data.storage_path) / "experiments" / experiment_id
        config_text = self._read_text(root / "config.toml", max_bytes=500_000)
        if not config_text:
            raise APIError("The experiment configuration was not found.", 404)
        config = ExperimentConfig.from_toml(config_text)
        run = self._select_run(runs, payload.get("strategy_id"))
        options = payload.get("options") or {}
        if not isinstance(options, dict):
            raise APIError("Plot options must be an object.")

        if plot_name == "pnl":
            figure = analysis.plot_pnl(
                runs,
                normalize=bool(options.get("normalize", False)),
                drawdown=bool(options.get("drawdown", True)),
                display=None,
            )
        elif plot_name == "cash":
            figure = analysis.plot_cash_holdings(runs, display=None)
        elif plot_name == "dividends":
            from backtide.storage import query_dividends

            dividends = query_dividends(list(config.data.symbols))
            figure = analysis.plot_dividends(dividends, display=None)
        elif plot_name == "pnl_histogram":
            figure = analysis.plot_pnl_histogram(
                runs,
                bins=self._optional_int(options.get("bins")),
                display=None,
            )
        elif plot_name == "rolling_returns":
            figure = analysis.plot_rolling_returns(
                runs,
                max(int(options.get("window", 30)), 2),
                display=None,
            )
        elif plot_name == "rolling_sharpe":
            periods_per_year = int(365 * 24 * 60 / config.data.interval.minutes())
            figure = analysis.plot_rolling_sharpe(
                runs,
                max(int(options.get("window", 60)), 2),
                periods_per_year,
                display=None,
            )
        elif plot_name == "trade_duration":
            figure = analysis.plot_trade_duration(
                runs,
                bins=self._optional_int(options.get("bins")),
                unit=str(options.get("unit", "auto")),
                display=None,
            )
        elif plot_name == "trade_pnl":
            figure = analysis.plot_trade_pnl(runs, display=None)
        elif plot_name == "mae_mfe":
            symbols = options.get("symbols") or sorted({trade.symbol for trade in run.trades})
            figure = analysis.plot_mae_mfe(
                run,
                interval=str(config.data.interval),
                symbols=symbols,
                display=None,
            )
        elif plot_name == "position_size":
            symbols = options.get("symbols") or sorted(
                {
                    order.order.symbol
                    for order in run.orders
                    if str(order.status).lower() == "filled"
                }
            )
            figure = analysis.plot_position_size(run, symbols=symbols, display=None)
        elif plot_name == "price":
            symbols = sorted({trade.symbol for trade in run.trades}) or list(config.data.symbols)
            symbol = str(options.get("symbol") or (symbols[0] if symbols else ""))
            if not symbol:
                raise APIError("No symbol is available for the price plot.")
            data = query_bars(symbol=symbol, interval=str(config.data.interval))
            figure = analysis.plot_price(data, run=run, display=None)
        else:
            raise APIError("Unknown result plot.", 404)
        if figure is None:
            raise APIError("The plot could not be generated.", 422)
        return json.loads(figure.to_json())

    def start_experiment(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Validate an experiment configuration and run it in the background."""
        from backtide.backtest import ExperimentConfig, run_experiment

        config = ExperimentConfig.from_dict(payload)

        def work() -> dict[str, Any]:
            result = run_experiment(config, verbose=False)
            return public_attributes(
                result,
                (
                    "experiment_id",
                    "name",
                    "status",
                    "started_at",
                    "finished_at",
                    "tags",
                    "warnings",
                ),
            )

        return self.jobs.start("experiment", work)

    def parse_experiment_config(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Parse an uploaded TOML, YAML or JSON experiment configuration."""
        text = payload.get("text")
        suffix = str(payload.get("suffix") or "").lower()
        if not isinstance(text, str):
            raise APIError("Configuration text is required.")
        if len(text.encode("utf-8")) > 500_000:
            raise APIError("Configuration file is too large.", 413)
        if suffix not in {".toml", ".json", ".yaml", ".yml"}:
            raise APIError("Use a .toml, .yaml, .yml or .json configuration file.")
        from backtide.backtest import ExperimentConfig

        try:
            if suffix == ".toml":
                config = ExperimentConfig.from_toml(text)
            elif suffix == ".json":
                config = ExperimentConfig.from_dict(json.loads(text))
            elif suffix in {".yaml", ".yml"}:
                import yaml

                config = ExperimentConfig.from_dict(yaml.safe_load(text))
        except APIError:
            raise
        except Exception as exc:
            raise APIError(f"Invalid configuration: {exc}") from exc
        return config.to_dict()

    def abort_experiment(self) -> dict[str, bool]:
        """Request cancellation of the currently running backtest."""
        from backtide.backtest import request_abort

        request_abort()
        return {"aborted": True}

    def delete_experiment(self, experiment_id: str) -> dict[str, int]:
        """Delete one persisted experiment."""
        from backtide.storage import delete_experiment

        deleted = delete_experiment(experiment_id)
        with self._result_runs_lock:
            if self._result_runs_cache and self._result_runs_cache[0] == experiment_id:
                self._result_runs_cache = None
        return {"deleted": deleted}

    def analysis_plot(self, plot_name: str, payload: dict[str, Any]) -> dict[str, Any]:
        """Build analysis metrics or one of the existing Plotly figures for the SPA."""
        from backtide import analysis
        from backtide.storage import query_bars, query_dividends

        names = {
            "candlestick": "plot_candlestick",
            "correlation": "plot_correlation",
            "dividends": "plot_dividends",
            "price": "plot_price",
            "returns": "plot_returns",
            "seasonality": "plot_seasonality",
            "volatility": "plot_volatility",
            "volume": "plot_volume",
            "vwap": "plot_vwap",
        }
        function_name = names.get(plot_name, "")
        if not function_name and plot_name != "metrics":
            raise APIError("Unknown analysis plot.", 404)
        symbols = payload.get("symbols") or []
        if not symbols:
            raise APIError("Select at least one symbol.")
        if plot_name == "dividends":
            data = query_dividends(symbols, payload.get("provider"))
        else:
            data = query_bars(
                symbols,
                payload.get("interval"),
                payload.get("provider"),
                limit=min(int(payload.get("limit", 50_000)), 100_000),
            )
        if plot_name == "metrics":
            statistics = analysis.compute_statistics(
                data,
                price_col=payload.get("price_col", "close"),
            )
            return {"rows": dataframe_records(statistics)}
        kwargs: dict[str, Any] = {"display": None}
        if plot_name in {"price", "returns", "correlation", "seasonality", "volatility"}:
            kwargs["price_col"] = payload.get("price_col", "close")
        if plot_name == "candlestick":
            kwargs["rangeslider"] = bool(payload.get("rangeslider", True))
        if plot_name == "volatility":
            kwargs["window"] = max(int(payload.get("window", 21)), 2)
        figure = getattr(analysis, function_name)(data, **kwargs)
        if figure is None:
            raise APIError("The plot could not be generated.", 422)
        return json.loads(figure.to_json())

    def strategy_catalog(self) -> dict[str, list[dict[str, Any]]]:
        """Return built-in and saved strategies with display metadata."""
        from backtide.config import get_config
        from backtide.strategies import BUILTIN_STRATEGIES
        from backtide.strategies.utils import _is_builtin_strategy, _load_stored_strategies

        builtins = []
        for cls in BUILTIN_STRATEGIES:
            instance = cls()
            builtins.append(
                {
                    "type": cls.__name__,
                    "name": instance.name,
                    "description": instance.description(),
                    "multi_asset": bool(instance.is_multi_asset),
                    "parameters": self._constructor_parameters(cls),
                }
            )
        saved = []
        for name, value in _load_stored_strategies(get_config()).items():
            builtin = _is_builtin_strategy(value)
            saved.append(
                {
                    "name": name,
                    "type": value.__class__.__name__,
                    "builtin": builtin,
                    "description": self._catalog_description(value),
                    "required_indicators": self._required_indicator_catalog(value),
                    "source": getattr(value, "_source_code", None),
                    "params": self._constructor_values(value) if builtin else {},
                }
            )
        return {"builtin": builtins, "saved": saved}

    def save_strategy(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Validate and save a built-in or custom strategy."""
        from backtide.config import get_config
        from backtide.strategies import BUILTIN_STRATEGIES
        from backtide.strategies.utils import (
            _build_custom_strategy,
            _check_strategy_code,
            _save_strategy,
        )

        instance = self._build_library_asset(
            payload,
            label="strategy",
            builtins=BUILTIN_STRATEGIES,
            validate=_check_strategy_code,
            build=_build_custom_strategy,
        )
        name = self._library_asset_name(
            payload.get("name"),
            instance if payload.get("kind") == "custom" else None,
            ignored_class_prefix="My",
        )
        _save_strategy(instance, name, get_config())
        return {"saved": name}

    def update_strategy(self, original_name: str, payload: dict[str, Any]) -> dict[str, str]:
        """Replace a saved strategy with the submitted editor configuration."""
        from backtide.config import get_config
        from backtide.strategies import BUILTIN_STRATEGIES
        from backtide.strategies.utils import (
            _build_custom_strategy,
            _check_strategy_code,
            _is_builtin_strategy,
            _load_stored_strategies,
            _save_strategy,
        )

        cfg = get_config()
        return self._update_saved_asset(
            folder="strategies",
            label="strategy",
            original_name=original_name,
            payload=payload,
            stored=_load_stored_strategies(cfg),
            storage_path=Path(cfg.data.storage_path),
            is_builtin=_is_builtin_strategy,
            validate=_check_strategy_code,
            build=_build_custom_strategy,
            rebuild=lambda value: self._build_library_asset(
                value,
                label="strategy",
                builtins=BUILTIN_STRATEGIES,
                validate=_check_strategy_code,
                build=_build_custom_strategy,
            ),
            save=lambda value, name: _save_strategy(value, name, cfg),
            ignored_class_prefix="My",
        )

    def delete_strategy(self, name: str) -> dict[str, bool]:
        """Delete one saved strategy file."""
        return {"deleted": self._delete_saved("strategies", name)}

    def indicator_catalog(self) -> dict[str, list[dict[str, Any]]]:
        """Return built-in and saved indicators with display metadata."""
        from backtide.config import get_config
        from backtide.indicators import BUILTIN_INDICATORS
        from backtide.indicators.utils import _is_builtin_indicator, _load_stored_indicators

        builtins = []
        for cls in BUILTIN_INDICATORS:
            instance = cls()
            builtins.append(
                {
                    "type": cls.__name__,
                    "name": instance.name,
                    "description": instance.description(),
                    "parameters": self._constructor_parameters(cls),
                }
            )
        saved = []
        for name, value in _load_stored_indicators(get_config()).items():
            builtin = _is_builtin_indicator(value)
            saved.append(
                {
                    "name": name,
                    "type": value.__class__.__name__,
                    "builtin": builtin,
                    "description": self._catalog_description(value),
                    "source": getattr(value, "_source_code", None),
                    "params": self._constructor_values(value) if builtin else {},
                }
            )
        return {"builtin": builtins, "saved": saved}

    def save_indicator(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Validate and save a built-in or custom indicator."""
        from backtide.config import get_config
        from backtide.indicators import BUILTIN_INDICATORS
        from backtide.indicators.utils import (
            _build_custom_indicator,
            _check_indicator_code,
            _save_indicator,
        )

        cfg = get_config()
        instance = self._build_library_asset(
            payload,
            label="indicator",
            builtins=BUILTIN_INDICATORS,
            validate=lambda code: _check_indicator_code(code, cfg),
            build=_build_custom_indicator,
        )
        name = self._library_asset_name(
            payload.get("name"),
            instance if payload.get("kind") == "custom" else None,
            ignored_class_prefix="My",
        )
        _save_indicator(instance, name, cfg)
        return {"saved": name}

    def update_indicator(self, original_name: str, payload: dict[str, Any]) -> dict[str, str]:
        """Replace a saved indicator with the submitted editor configuration."""
        from backtide.config import get_config
        from backtide.indicators import BUILTIN_INDICATORS
        from backtide.indicators.utils import (
            _build_custom_indicator,
            _check_indicator_code,
            _is_builtin_indicator,
            _load_stored_indicators,
            _save_indicator,
        )

        cfg = get_config()
        return self._update_saved_asset(
            folder="indicators",
            label="indicator",
            original_name=original_name,
            payload=payload,
            stored=_load_stored_indicators(cfg),
            storage_path=Path(cfg.data.storage_path),
            is_builtin=_is_builtin_indicator,
            validate=lambda code: _check_indicator_code(code, cfg),
            build=_build_custom_indicator,
            rebuild=lambda value: self._build_library_asset(
                value,
                label="indicator",
                builtins=BUILTIN_INDICATORS,
                validate=lambda code: _check_indicator_code(code, cfg),
                build=_build_custom_indicator,
            ),
            save=lambda value, name: _save_indicator(value, name, cfg),
            ignored_class_prefix="My",
        )

    def delete_indicator(self, name: str) -> dict[str, bool]:
        """Delete one saved indicator file."""
        return {"deleted": self._delete_saved("indicators", name)}

    def metric_catalog(self) -> dict[str, list[dict[str, Any]]]:
        """Return Rust built-in metrics and saved custom Python metrics."""
        from backtide.config import get_config
        from backtide.metrics import BUILTIN_METRICS
        from backtide.metrics.utils import _load_stored_metrics

        builtins = [
            {
                "key": value.key,
                "name": value.name,
                "type": value.key,
                "builtin": True,
                "description": value.description,
                "percentage": value.percentage,
                "higher_is_better": value.higher_is_better,
            }
            for value in BUILTIN_METRICS
        ]
        saved = [
            {
                "key": name,
                "name": name,
                "type": type(value).__name__,
                "builtin": False,
                "description": self._custom_metric_description(value),
                "percentage": bool(getattr(value, "percentage", False)),
                "higher_is_better": bool(getattr(value, "higher_is_better", True)),
                "source": getattr(value, "_source_code", None),
            }
            for name, value in _load_stored_metrics(get_config()).items()
        ]
        return {"builtin": builtins, "saved": saved}

    def sizer_catalog(self) -> dict[str, list[dict[str, Any]]]:
        """Return built-in and saved position sizers with display metadata."""
        from backtide.config import get_config
        from backtide.sizers import BUILTIN_SIZERS
        from backtide.sizers.utils import (
            BUILTIN_SIZER_DEFAULTS,
            _is_builtin_sizer,
            _load_stored_sizers,
        )

        builtins = []
        for cls in BUILTIN_SIZERS:
            doc = inspect.getdoc(cls)
            builtins.append(
                {
                    "type": cls.__name__,
                    "name": self._display_name(cls.__name__),
                    "description": doc.splitlines()[0] if doc else f"Built-in {cls.__name__}.",
                    "parameters": self._constructor_parameters(
                        cls,
                        defaults=BUILTIN_SIZER_DEFAULTS[cls.__name__],
                    ),
                }
            )
        saved = []
        for name, value in _load_stored_sizers(get_config()).items():
            builtin = _is_builtin_sizer(value)
            saved.append(
                {
                    "name": name,
                    "type": type(value).__name__,
                    "builtin": builtin,
                    "description": self._catalog_description(value),
                    "source": getattr(value, "_source_code", None),
                    "params": self._constructor_values(value) if builtin else {},
                }
            )
        return {"builtin": builtins, "saved": saved}

    def save_sizer(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Validate and persist a built-in preset or custom position sizer."""
        from backtide.config import get_config
        from backtide.sizers import BUILTIN_SIZERS
        from backtide.sizers.utils import _build_custom_sizer, _check_sizer_code, _save_sizer

        cfg = get_config()
        instance = self._build_library_asset(
            payload,
            label="sizer",
            builtins=BUILTIN_SIZERS,
            validate=_check_sizer_code,
            build=_build_custom_sizer,
        )
        name = self._library_asset_name(
            payload.get("name"),
            instance if payload.get("kind") == "custom" else None,
            ignored_class_prefix="My",
        )
        _save_sizer(instance, name, cfg)
        return {"saved": name}

    def update_sizer(self, original_name: str, payload: dict[str, Any]) -> dict[str, str]:
        """Replace a saved position sizer."""
        from backtide.config import get_config
        from backtide.sizers import BUILTIN_SIZERS
        from backtide.sizers.utils import (
            _build_custom_sizer,
            _check_sizer_code,
            _is_builtin_sizer,
            _load_stored_sizers,
            _save_sizer,
        )

        cfg = get_config()
        return self._update_saved_asset(
            folder="sizers",
            label="sizer",
            original_name=original_name,
            payload=payload,
            stored=_load_stored_sizers(cfg),
            storage_path=Path(cfg.data.storage_path),
            is_builtin=_is_builtin_sizer,
            validate=_check_sizer_code,
            build=_build_custom_sizer,
            rebuild=lambda value: self._build_library_asset(
                value,
                label="sizer",
                builtins=BUILTIN_SIZERS,
                validate=_check_sizer_code,
                build=_build_custom_sizer,
            ),
            save=lambda value, name: _save_sizer(value, name, cfg),
            ignored_class_prefix="My",
        )

    def delete_sizer(self, name: str) -> dict[str, bool]:
        """Delete one saved position-sizer file."""
        return {"deleted": self._delete_saved("sizers", name)}

    def save_metric(self, payload: dict[str, Any]) -> dict[str, str]:
        """Validate and save a custom Python metric."""
        from backtide.config import get_config
        from backtide.metrics import BUILTIN_METRICS
        from backtide.metrics.utils import _build_custom_metric, _check_metric_code, _save_metric

        reserved_names = {metric.key for metric in BUILTIN_METRICS}
        entered_name = str(payload.get("name") or "").strip()
        if entered_name:
            entered_name = self._safe_library_name(entered_name)
            if entered_name in reserved_names:
                raise APIError(f"{entered_name!r} is reserved for a built-in metric.", 409)

        instance = self._build_library_asset(
            payload,
            label="metric",
            builtins=(),
            validate=_check_metric_code,
            build=_build_custom_metric,
        )
        name = self._library_asset_name(payload.get("name"), instance, ignored_class_prefix="My")
        if name in reserved_names:
            raise APIError(f"{name!r} is reserved for a built-in metric.", 409)
        _save_metric(instance, name, get_config())
        return {"saved": name}

    def update_metric(self, original_name: str, payload: dict[str, Any]) -> dict[str, str]:
        """Replace a saved custom metric."""
        from backtide.config import get_config
        from backtide.metrics import BUILTIN_METRICS
        from backtide.metrics.utils import (
            _build_custom_metric,
            _check_metric_code,
            _load_stored_metrics,
            _save_metric,
        )

        cfg = get_config()
        return self._update_saved_asset(
            folder="metrics",
            label="metric",
            original_name=original_name,
            payload=payload,
            stored=_load_stored_metrics(cfg),
            storage_path=Path(cfg.data.storage_path),
            is_builtin=lambda _value: False,
            validate=_check_metric_code,
            build=_build_custom_metric,
            rebuild=lambda value: self._build_library_asset(
                value,
                label="metric",
                builtins=(),
                validate=_check_metric_code,
                build=_build_custom_metric,
            ),
            save=lambda value, name: _save_metric(value, name, cfg),
            ignored_class_prefix="My",
            reserved_names={metric.key for metric in BUILTIN_METRICS},
        )

    def delete_metric(self, name: str) -> dict[str, bool]:
        """Delete one saved custom metric."""
        return {"deleted": self._delete_saved("metrics", name)}

    def live_capabilities(self) -> dict[str, Any]:
        """Describe provider support for WebSocket market data."""
        try:
            from backtide.live import LiveMarketFeed
        except ImportError:
            return {
                "available": False,
                "providers": {},
                "message": "Live trading support is not available in this build.",
            }
        intervals = ("1m", "5m", "15m", "30m", "1h", "4h", "1d", "1w")
        providers = {}
        for provider in ("binance", "kraken", "coinbase", "yahoo"):
            interval_support = {}
            for interval in intervals:
                try:
                    feed = LiveMarketFeed(provider, ["BTC-USD"], interval)
                    feed.cancel()
                    supported = True
                    reason = f"{provider.title()} public WebSocket supports {interval}."
                except (RuntimeError, TypeError, ValueError) as exc:
                    supported, reason = False, str(exc)
                interval_support[interval] = {"supported": supported, "reason": reason}
            supported_intervals = [
                interval for interval, value in interval_support.items() if value["supported"]
            ]
            providers[provider] = {
                "supported": bool(supported_intervals),
                "reason": (
                    f"Supported intervals: {', '.join(supported_intervals)}."
                    if supported_intervals
                    else interval_support["1m"]["reason"]
                ),
                "intervals": interval_support,
            }
        return {"available": True, "providers": providers}

    def live_status(self) -> dict[str, Any]:
        """Return the current paper-trading state if the live module exposes it."""
        manager = getattr(self, "_live_manager", None)
        if manager is None:
            return {"status": "idle", "snapshot": None, "updates": []}
        return manager.status()

    def start_live(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Start a WebSocket-backed paper-trading session."""
        from backtide.ui.live import LiveTradingManager

        manager = getattr(self, "_live_manager", None)
        if manager is None:
            manager = self._live_manager = LiveTradingManager()
        return manager.start(payload)

    def stop_live(self) -> dict[str, Any]:
        """Stop the active paper-trading session."""
        manager = getattr(self, "_live_manager", None)
        if manager is None:
            return {"status": "idle"}
        return manager.stop()

    def live_sessions(self) -> list[dict[str, Any]]:
        """Return persisted paper-session summaries."""
        from backtide.ui.live import LiveTradingManager

        manager = getattr(self, "_live_manager", None) or LiveTradingManager()
        return manager.sessions()

    def live_session(self, session_id: str) -> dict[str, Any]:
        """Return one persisted paper session and its event journal."""
        from backtide.ui.live import LiveTradingManager

        manager = getattr(self, "_live_manager", None) or LiveTradingManager()
        return manager.session(session_id)

    def delete_live_session(self, session_id: str) -> dict[str, int]:
        """Delete one inactive persisted paper session."""
        from backtide.ui.live import LiveTradingManager

        manager = getattr(self, "_live_manager", None) or LiveTradingManager()
        return manager.delete_session(session_id)

    def replay_live(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Replay a persisted paper session through a fresh engine."""
        from backtide.ui.live import LiveTradingManager

        manager = getattr(self, "_live_manager", None)
        if manager is None:
            manager = self._live_manager = LiveTradingManager()
        return manager.replay(
            str(payload.get("session_id") or ""),
            payload.get("speed", "max"),
        )

    def pause_live(self) -> dict[str, Any]:
        """Pause the active paper strategy without closing its feed."""
        manager = getattr(self, "_live_manager", None)
        return manager.pause() if manager else {"status": "idle"}

    def resume_live(self) -> dict[str, Any]:
        """Resume the active paper strategy."""
        manager = getattr(self, "_live_manager", None)
        return manager.resume() if manager else {"status": "idle"}

    def flatten_live(self) -> dict[str, Any]:
        """Request liquidation of all paper positions."""
        manager = getattr(self, "_live_manager", None)
        return manager.flatten() if manager else {"status": "idle"}

    def cancel_live_orders(self) -> dict[str, Any]:
        """Request cancellation of every resting paper order."""
        manager = getattr(self, "_live_manager", None)
        return manager.cancel_all() if manager else {"status": "idle"}

    def paper_config_from_experiment(self, experiment_id: str) -> dict[str, Any]:
        """Translate compatible experiment settings into a live-session draft."""
        from backtide.backtest import ExperimentConfig
        from backtide.config import get_config

        root = Path(get_config().data.storage_path) / "experiments" / experiment_id
        text = self._read_text(root / "config.toml", max_bytes=500_000)
        if not text:
            raise APIError("The experiment configuration was not found.", 404)
        config = ExperimentConfig.from_toml(text).to_dict()
        data = config.get("data", {})
        portfolio = config.get("portfolio", {})
        strategy = config.get("strategy", {})
        indicators = config.get("indicators", {})
        metrics = config.get("metrics", {})
        exchange = config.get("exchange", {})
        engine = config.get("engine", {})
        benchmark = str(strategy.get("benchmark") or "").strip()
        strategy_names = [
            str(name).strip() for name in strategy.get("strategies", []) if str(name).strip()
        ]
        first_strategy = next((name for name in strategy_names if name != benchmark), None)
        commission_type = re.sub(
            r"[^a-z]", "", str(exchange.get("commission_type") or "Percentage").lower()
        )
        uses_percentage_commission = "percentage" in commission_type
        uses_fixed_commission = "fixed" in commission_type
        provider = str(data.get("provider") or "kraken").lower()
        if provider == "yahoo":
            provider = "kraken"
        interval = {
            "OneMinute": "1m",
            "FiveMinutes": "5m",
            "FifteenMinutes": "15m",
            "ThirtyMinutes": "30m",
            "OneHour": "1h",
            "FourHours": "4h",
            "OneDay": "1d",
            "OneWeek": "1w",
        }.get(str(data.get("interval")), str(data.get("interval") or "1m"))
        live_metrics = [
            key for key in metrics.get("metrics", []) if key not in {"alpha", "excess_return"}
        ]
        return {
            "provider": provider,
            "interval": interval,
            "symbols": data.get("symbols", []),
            "strategies": [first_strategy] if first_strategy else [],
            "indicators": indicators.get("indicators", []),
            "warmup_bars": int(engine.get("warmup_period") or 0),
            "config": {
                "initial_cash": portfolio.get("initial_cash", 10_000),
                "base_currency": portfolio.get("base_currency", "USD"),
                "commission_pct": (
                    exchange.get("commission_pct", 0.1) if uses_percentage_commission else 0.0
                ),
                "commission_fixed": (
                    exchange.get("commission_fixed", 0.0) if uses_fixed_commission else 0.0
                ),
                "slippage": exchange.get("slippage", 0.05),
                "allow_short": exchange.get("allow_short_selling", False),
                "allow_margin": exchange.get("allow_margin", False),
                "max_leverage": exchange.get("max_leverage", 2.0),
                "initial_margin": exchange.get("initial_margin", 50.0),
                "maintenance_margin": exchange.get("maintenance_margin", 25.0),
                "margin_interest": exchange.get("margin_interest", 0.0),
                "borrow_rate": exchange.get("borrow_rate", 0.0),
                "max_position_size": exchange.get("max_position_size", 100.0),
                "allowed_order_types": exchange.get("allowed_order_types", ["Market"]),
                "partial_fills": exchange.get("partial_fills", False),
                "metrics": live_metrics,
                "risk_free_rate": engine.get("risk_free_rate", 0.0),
            },
            "compatibility": {
                "source_experiment_id": experiment_id,
                "warnings": [
                    "Historical date ranges and benchmark execution are not copied.",
                    "Yahoo data was replaced with Kraken because Yahoo has no live WebSocket.",
                ]
                if str(data.get("provider") or "").lower() == "yahoo"
                else ["Historical date ranges and benchmark execution are not copied."],
            },
        }

    def _delete_saved(self, folder: str, name: str) -> bool:
        from backtide.config import get_config

        safe_name = self._safe_name(name)
        path = Path(get_config().data.storage_path) / folder / f"{safe_name}.pkl"
        existed = path.exists()
        path.unlink(missing_ok=True)
        return existed

    def _update_saved_asset(
        self,
        *,
        folder: str,
        label: str,
        original_name: str,
        payload: dict[str, Any],
        stored: dict[str, Any],
        storage_path: Path,
        is_builtin: Callable[[Any], bool],
        validate: Callable[[str], str | None],
        build: Callable[[str], Any],
        rebuild: Callable[[dict[str, Any]], Any] | None = None,
        save: Callable[[Any, str], None],
        ignored_class_prefix: str | None = None,
        reserved_names: set[str] | None = None,
    ) -> dict[str, str]:
        """Replace a saved library object without leaving an old file after a rename."""
        original = self._safe_name(original_name)
        original_path = storage_path / folder / f"{original}.pkl"

        if original not in stored:
            if original_path.exists():
                raise APIError(f"Saved {label} {original!r} could not be loaded.", 422)
            raise APIError(f"Saved {label} {original!r} was not found.", 404)

        instance = stored[original]
        custom = not is_builtin(instance)
        if "kind" in payload:
            if rebuild is None:
                raise APIError(f"Saved {label} configuration cannot be replaced.")
            instance = rebuild(payload)
            custom = payload.get("kind") == "custom"
        elif custom:
            code = str(payload.get("code") or "")
            if error := validate(code):
                raise APIError(error)
            instance = build(code)

        name = self._library_asset_name(
            payload.get("name"),
            instance if custom else None,
            ignored_class_prefix=ignored_class_prefix,
        )
        if reserved_names and name in reserved_names:
            raise APIError(f"{name!r} is reserved for a built-in {label}.", 409)
        target_path = storage_path / folder / f"{name}.pkl"
        if name != original and target_path.exists():
            raise APIError(f"A {label} named {name!r} already exists.", 409)

        save(instance, name)
        if name != original:
            original_path.unlink(missing_ok=True)
        return {"saved": name}

    def _serialize_run(self, run: Any, *, include_orders: bool = True) -> dict[str, Any]:
        """Serialize one strategy run, optionally omitting its order history."""
        output = public_attributes(run, self.run_fields)
        output["trades"] = [
            public_attributes(
                trade,
                (
                    "symbol",
                    "quantity",
                    "entry_ts",
                    "entry_price",
                    "exit_ts",
                    "exit_price",
                    "pnl",
                ),
            )
            for trade in run.trades
        ]
        output["order_count"] = len(run.orders)
        if include_orders:
            output["orders"] = [self._serialize_order_record(order) for order in run.orders]
        return _clean(output)

    def _serialize_order_record(self, order: Any) -> dict[str, Any]:
        """Serialize one stored order together with its submitted order details."""
        return _clean(
            public_attributes(
                order,
                ("timestamp", "status", "fill_price", "commission", "pnl", "reason"),
            )
            | {"order": self._serialize_order(order.order)}
        )

    def _primary_metric_summary(
        self,
        config_text: str | None,
        runs: list[dict[str, Any]],
        catalog: dict[str, list[dict[str, Any]]] | None = None,
    ) -> dict[str, Any]:
        """Resolve the configured headline metric and its best strategy value."""
        selected_metrics: list[str] = []
        try:
            metric_config = tomllib.loads(config_text or "").get("metrics", {})
            if not isinstance(metric_config, dict):
                raise TypeError("Metric configuration must be a table.")
            metric_key = str(metric_config.get("main_metric"))
            configured_metrics = metric_config.get("metrics", [])
            if isinstance(configured_metrics, list):
                selected_metrics = list(
                    dict.fromkeys(str(metric) for metric in configured_metrics if metric)
                )
        except (tomllib.TOMLDecodeError, TypeError):
            metric_key = "sharpe"
        if not metric_key or metric_key == "None":
            metric_key = "sharpe"
        catalog = catalog or self.metric_catalog()
        definition = next(
            (
                item
                for item in catalog["builtin"] + catalog["saved"]
                if item.get("key") == metric_key
            ),
            None,
        )
        candidates = [run for run in runs if not run.get("is_benchmark")] or runs
        values = [
            float(run["metrics"][metric_key])
            for run in candidates
            if run.get("metrics", {}).get(metric_key) is not None
        ]
        higher_is_better = bool(definition.get("higher_is_better", True)) if definition else True
        best = (max(values) if higher_is_better else min(values)) if values else None
        return {
            "primary_metric": metric_key,
            "selected_metrics": selected_metrics,
            "primary_metric_name": (
                definition.get("name", metric_key) if definition else metric_key
            ),
            "primary_metric_value": best,
            "primary_metric_percentage": bool(definition.get("percentage", False))
            if definition
            else False,
        }

    def _query_result_runs(self, experiment_id: str) -> list[Any]:
        """Load and retain one full result so adjacent plot requests reuse it."""
        from backtide.storage import query_strategy_runs

        with self._result_runs_lock:
            if self._result_runs_cache and self._result_runs_cache[0] == experiment_id:
                return self._result_runs_cache[1]
            runs = query_strategy_runs(experiment_id)
            self._result_runs_cache = (experiment_id, runs)
            return runs

    @staticmethod
    def _serialize_order(order: Any) -> dict[str, Any]:
        return public_attributes(
            order,
            ("id", "symbol", "order_type", "quantity", "price", "limit_price"),
        )

    @staticmethod
    def _order_timestamp(value: Any) -> float:
        """Normalize stored timestamp values for newest-first order sorting."""
        timestamp = getattr(value, "timestamp", None)
        if callable(timestamp):
            return float(timestamp())
        try:
            return float(value)
        except (TypeError, ValueError):
            return 0.0

    @staticmethod
    def _safe_name(value: Any) -> str:
        name = str(value or "").strip()
        if not name or len(name) > 80 or any(char in name for char in '<>:"/\\|?*'):
            raise APIError("Enter a valid name without filename control characters.")
        if name in {".", ".."}:
            raise APIError("Enter a valid name.")
        return name

    @classmethod
    def _safe_library_name(cls, value: Any) -> str:
        """Validate the compact display name used by reusable library assets."""
        name = cls._safe_name(value)
        if len(name) > 20:
            raise APIError("Library names must be 20 characters or fewer.")
        return name

    @classmethod
    def _library_asset_name(
        cls,
        value: Any,
        instance: Any | None = None,
        *,
        ignored_class_prefix: str | None = None,
    ) -> str:
        """Return an entered library name or infer it from a custom Python instance."""
        entered = str(value or "").strip()
        if entered:
            return cls._safe_library_name(entered)
        if instance is None:
            return cls._safe_library_name(entered)

        inferred = type(instance).__name__
        if ignored_class_prefix and inferred.casefold().startswith(
            ignored_class_prefix.casefold()
        ):
            raise APIError("Enter a name or rename the placeholder Python class.")
        return cls._safe_library_name(inferred)

    @staticmethod
    def _display_name(value: str) -> str:
        """Split a CamelCase Python type into a readable display name."""
        return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", " ", value)

    @staticmethod
    def _download_name(value: Any) -> str:
        """Return a filename-safe ASCII experiment name for browser downloads."""
        name = "".join(
            char if char.isascii() and (char.isalnum() or char in " -_.") else "_"
            for char in str(value or "")
        ).strip(" ._")
        return name[:80] or "experiment"

    @staticmethod
    def _variants(enum: Any) -> list[str]:
        return [str(value) for value in enum.variants()]

    @staticmethod
    def _currency_options(enum: Any) -> list[dict[str, str]]:
        """Return currency display metadata sourced from the Rust enum."""
        return [
            {
                "code": str(currency),
                "name": currency.name,
                "flag": currency.country.flag,
                "country_code": currency.country.alpha2.lower(),
            }
            for currency in enum.variants()
        ]

    @staticmethod
    def _select_run(runs: list[Any], strategy_id: Any) -> Any:
        if strategy_id:
            for run in runs:
                if run.strategy_id == strategy_id:
                    return run
            raise APIError("Strategy run not found.", 404)
        return runs[0]

    @staticmethod
    def _optional_int(value: Any) -> int | None:
        return max(int(value), 1) if value not in (None, "") else None

    @staticmethod
    def _read_text(path: Path, max_bytes: int) -> str | None:
        if not path.is_file():
            return None
        with path.open("rb") as file:
            data = file.read(max_bytes + 1)
        if len(data) > max_bytes:
            data = data[-max_bytes:]
        return data.decode("utf-8", errors="replace")

    @staticmethod
    def _read_log_tail(
        path: Path,
        *,
        max_lines: int,
        max_bytes: int,
    ) -> tuple[str | None, bool]:
        """Read a bounded tail of a potentially large experiment log."""
        if not path.is_file():
            return None, False
        size = path.stat().st_size
        start = max(0, size - max_bytes)
        with path.open("rb") as file:
            file.seek(start)
            text = file.read(max_bytes).decode("utf-8", errors="replace")
        lines = text.splitlines()
        line_truncated = len(lines) > max_lines
        if line_truncated:
            text = "\n".join(lines[-max_lines:])
        return text, start > 0 or line_truncated

    @staticmethod
    def _experiment_config_metadata(config_text: str | None) -> dict[str, Any] | None:
        """Return the display metadata needed by the experiment result summary."""
        if not config_text:
            return None
        try:
            data = tomllib.loads(config_text).get("data", {})
        except tomllib.TOMLDecodeError:
            return None
        if not isinstance(data, dict):
            return None
        symbols = data.get("symbols")
        return {
            "symbols": len(symbols) if isinstance(symbols, list) else 0,
            "instrument_type": str(data.get("instrument_type") or ""),
            "interval": str(data.get("interval") or "—"),
            "full_history": bool(data.get("full_history", False)),
            "start_date": data.get("start_date"),
            "end_date": data.get("end_date"),
        }

    @staticmethod
    def _apply_benchmark_display_name(config_text: str | None, runs: list[dict[str, Any]]) -> None:
        """Display benchmark runs under their configured market symbol."""
        try:
            strategy = tomllib.loads(config_text or "").get("strategy", {})
        except (tomllib.TOMLDecodeError, TypeError):
            return
        if not isinstance(strategy, dict):
            return
        benchmark = str(strategy.get("benchmark") or "").strip()
        if not benchmark:
            return
        for run in runs:
            if run.get("is_benchmark"):
                run["strategy_name"] = benchmark

    @staticmethod
    def _constructor_parameters(
        cls: Any,
        *,
        defaults: dict[str, Any] | None = None,
    ) -> list[dict[str, Any]]:
        signature = inspect.signature(cls)
        defaults = defaults or {}
        documented_types: dict[str, str] = {}
        for line in (inspect.getdoc(cls) or "").splitlines():
            name, separator, type_name = line.strip().partition(":")
            if separator and name.strip() in signature.parameters:
                documented_types[name.strip()] = type_name.strip().lower()
        parameters = []
        for name, parameter in signature.parameters.items():
            default = parameter.default
            if default is inspect.Parameter.empty:
                default = defaults.get(name)
            annotation = parameter.annotation
            annotation_name = (
                annotation if isinstance(annotation, str) else getattr(annotation, "__name__", "")
            ).lower()
            documented_type = documented_types.get(name, "")
            if (
                isinstance(default, bool)
                or annotation is bool
                or annotation_name == "bool"
                or documented_type.startswith("bool")
            ):
                kind = "boolean"
            elif (
                isinstance(default, (int, float))
                or annotation in (int, float)
                or annotation_name in {"int", "float"}
                or documented_type.startswith(("int", "float"))
            ):
                kind = "number"
            else:
                kind = "text"
            parameters.append(
                {
                    "name": name,
                    "label": name.replace("_", " ").title(),
                    "kind": kind,
                    "default": default,
                    "required": parameter.default is inspect.Parameter.empty
                    and name not in defaults,
                }
            )
        return parameters

    @staticmethod
    def _constructor_values(value: Any) -> dict[str, Any]:
        """Return the constructor values stored in a built-in library object."""
        return {name: getattr(value, name) for name in inspect.signature(type(value)).parameters}

    @staticmethod
    def _build_library_asset(
        payload: dict[str, Any],
        *,
        label: str,
        builtins: Sequence[Any],
        validate: Callable[[str], str | None],
        build: Callable[[str], Any],
    ) -> Any:
        """Build a complete built-in or custom library object from an editor payload."""
        kind = payload.get("kind")
        if kind == "custom":
            code = str(payload.get("code") or "")
            if error := validate(code):
                raise APIError(error)
            return build(code)
        if kind != "builtin":
            raise APIError(f"Unknown {label} kind.")

        asset_type = payload.get("type")
        asset = next((cls for cls in builtins if cls.__name__ == asset_type), None)
        if asset is None:
            raise APIError(f"Unknown built-in {label}.")
        params = payload.get("params") or {}
        if not isinstance(params, dict):
            raise APIError("Built-in parameters must be a JSON object.")
        return asset(**params)

    @staticmethod
    def _catalog_description(value: Any) -> str:
        """Return stable display text for built-in and custom library objects."""
        description = getattr(value, "description", None)
        if callable(description):
            return str(description())
        if description:
            return str(description)
        doc = inspect.getdoc(type(value))
        return doc.splitlines()[0] if doc else f"Custom {type(value).__name__}."

    @staticmethod
    def _custom_metric_description(value: Any) -> str:
        """Return the custom metric class docstring as its catalog description."""
        doc = type(value).__doc__
        return inspect.cleandoc(doc) if doc else f"Custom {type(value).__name__}."

    @classmethod
    def _required_indicator_catalog(cls, strategy: Any) -> list[dict[str, str]]:
        """Return display metadata for indicators auto-injected by a strategy."""
        from backtide.strategies.utils import _resolve_auto_indicators

        return [
            {
                "name": name,
                "type": type(indicator).__name__,
                "description": cls._catalog_description(indicator),
            }
            for name, indicator, _source in _resolve_auto_indicators([strategy])
        ]

    @staticmethod
    def _download_date(value: Any, timezone: Any, *, fallback: date) -> date:
        """Convert a download-plan boundary to a date in the configured timezone."""
        if value in (None, ""):
            return fallback
        if isinstance(value, datetime):
            parsed = value
        elif isinstance(value, date):
            return value
        elif isinstance(value, (int, float)):
            return datetime.fromtimestamp(value, timezone).date()
        else:
            try:
                parsed = datetime.fromisoformat(str(value))
            except ValueError as exc:
                raise APIError(f"Invalid ISO date: {value!r}.") from exc
        if parsed.tzinfo is not None:
            parsed = parsed.astimezone(timezone)
        return parsed.date()

    @staticmethod
    def _estimate_download_bars(
        profile: Any,
        interval: Any,
        start: date,
        end: date,
    ) -> int:
        """Estimate provider rows using the market-hours assumptions from the legacy UI."""
        if end < start:
            return 0
        delta_minutes = max((end - start).total_seconds() / 60, 1)
        delta_days = (end - start).days
        interval_minutes = interval.minutes()
        if profile.instrument_type.is_equity:
            if interval.is_intraday():
                return max(int(delta_minutes * (5 / 7) * (8 / 24) // interval_minutes), 1)
            return max(int(delta_days * (5 / 7) // (interval_minutes / 1_440)), 1)
        if str(profile.instrument_type).casefold() == "forex":
            if interval.is_intraday():
                return max(int(delta_minutes * (5 / 7) // interval_minutes), 1)
            return max(int(delta_days * (5 / 7) // (interval_minutes / 1_440)), 1)
        return max(int(delta_minutes // interval_minutes), 1)

    @staticmethod
    def _date_boundary(value: Any, *, end: bool) -> int | None:
        if value in (None, ""):
            return None
        if isinstance(value, (int, float)):
            return int(value)
        try:
            parsed = datetime.fromisoformat(str(value))
        except ValueError as exc:
            raise APIError(f"Invalid ISO date: {value!r}.") from exc
        if end and len(str(value)) == 10:
            parsed = parsed.replace(hour=23, minute=59, second=59)
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=UTC)
        return int(parsed.timestamp())
