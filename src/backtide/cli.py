"""Backtide.

Author: Mavs
Description: Entry point for the CLI application.

"""

import json
from pathlib import Path
import tomllib
from typing import Any

import click
import yaml

from backtide.backtest import ExperimentAborted, ExperimentConfig, ExperimentStatus
from backtide.backtest import run_experiment as run_backtest
from backtide.core.config import get_config
from backtide.core.utils import init_logging
from backtide.data import download_bars, resolve_profiles
from backtide.live import (
    LiveMarketFeed,
    PaperTradingConfig,
    PaperTradingSession,
    _live_currency_plan,
)


@click.group()
def main():
    """CLI application entry point."""


@main.command()
@click.argument("symbols", nargs=-1, required=True)
@click.option(
    "--instrument-type",
    "-t",
    default="stocks",
    show_default=True,
    help="Instrument type: stocks, etf, forex, crypto.",
)
@click.option(
    "--interval",
    "-i",
    multiple=True,
    default=("1d",),
    show_default=True,
    help="Bar interval(s). Can be repeated, e.g., -i 1d -i 1h.",
)
@click.option(
    "--start",
    "-s",
    default=None,
    help="Start date in Unix seconds. If None, the full available history is downloaded.",
)
@click.option(
    "--end",
    "-e",
    default=None,
    help="End date in Unix seconds. Defaults to now.",
)
@click.option(
    "--log_level",
    "-l",
    help="Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.",
)
@click.option(
    "--verbose/--no-verbose",
    "-v",
    default=True,
    show_default=True,
    help="Show a progress bar while downloading.",
)
def download(symbols, instrument_type, interval, start, end, log_level, verbose):
    """Download OHLCV bar data for one or more symbols and persist it locally.

    Fetches open/high/low/close/volume bars from the configured data provider and
    stores them in the local database. Any currency conversion legs required by
    the requested symbols are resolved and downloaded automatically.  Already
    cached bars are skipped, so it is safe to re-run the command to top up an
    existing dataset.

    Read more in the [user guide][data].

    Parameters
    ----------
    symbols : tuple[str, ...]
        One or more ticker symbols to download (e.g., `AAPL`, `BTC-USD`). Multiple
        symbols can be listed space-separated.

    --instrument_type, -t : str, default="stocks"
        Asset class of the requested symbols.  Choose from `stocks`, `etf`, `forex`
        or `crypto`.  All symbols in a single invocation must belong to the same
        instrument type.

    --interval, -i : tuple[str, ...], default="1d"
        One or more bar intervals to download. The flag can be repeated to fetch
        several resolutions in one call (e.g., `-i 1d -i 1h`). Supported values are:
        `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `1d`, `1wk`.

    --start, -s : str | None, default=None
        Earliest bar to include, expressed as a Unix timestamp in seconds. When
        omitted the provider's maximum available history is downloaded.

    --end, -e : str | None, default=None
        Latest bar to include, expressed as a Unix timestamp in seconds. Defaults
        to the current time when omitted.

    --log_level, -l : str, default="warn"
        Minimum log level to emit. Choose from: `error`, `warn`, `info` or
        `debug`.

    --verbose/--no-verbose, -v : bool, default=True
        Whether to display a progress bar while bars are being downloaded.

    See Also
    --------
    - backtide.data:download_bars
    - backtide.data:resolve_profiles
    - backtide.cli:run_experiment

    Examples
    --------
    Download the full available daily history for a single stock:
    ```
    backtide download AAPL
    ```

    Download both daily and hourly bars for several crypto symbols:
    ```
    backtide download BTC-USD ETH-USD -t crypto -i 1d -i 1h
    ```

    Download forex bars starting from a specific date:
    ```
    backtide download EUR-USD -t forex --start 1672531200
    ```

    """
    cfg = get_config()
    init_logging(log_level or cfg.general.log_level)

    profiles = resolve_profiles(list(symbols), instrument_type, list(interval), verbose=verbose)
    result = download_bars(profiles, start=start, end=end, verbose=verbose)

    for warn in result.warnings:
        click.echo(f"WARNING: {warn}", err=True)

    if result.n_failed and result.n_succeeded:
        click.echo(
            f"Done ({result.n_succeeded}/{result.n_succeeded + result.n_failed} "
            f"instruments downloaded).",
        )
    elif result.n_failed:
        click.echo(f"ERROR: All {result.n_failed} downloads failed.", err=True)
    else:
        click.echo("Done.")


@main.command()
@click.option(
    "--address",
    "-a",
    help=(
        "The address where the server will listen for client and browser connections. "
        "Use this if you want to bind the server to a specific address. If set, the server "
        "will only be available from this address, and not from any aliases (like localhost)."
    ),
)
@click.option(
    "--port",
    "-p",
    help="The port where the server will listen for browser connections.",
)
@click.option(
    "--log_level",
    "-l",
    help="Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.",
)
def launch(address: str, port: str, log_level: str):
    """Launch the Backtide UI in a local web browser.

    Starts the bundled graphical interface, which lets you browse stored
    experiments, inspect equity curves, trade logs, performance metrics and
    paper-trading sessions without writing any code.

    Read more in the [user guide][application].

    Parameters
    ----------
    --address, -a : str
        The address where the server will listen for client and browser
        connections. Use this if you want to bind the server to a specific
        address. If set, the server will only be available from this address,
        and not from any aliases (like localhost).

    --port, -p : str, default=8501
        TCP port the server listens on.

    --log_level, -l : str, default="warn"
        Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.

    See Also
    --------
    - backtide.config:Config
    - backtide.cli:download
    - backtide.cli:run_experiment

    Examples
    --------
    Launch with default settings:
    ```
    backtide launch
    ```

    Launch on a custom port and address:
    ```
    backtide launch --port 9000 --address 0.0.0.0
    ```
    """
    cfg = get_config()
    init_logging(log_level or cfg.general.log_level)

    click.echo("Launching app...")

    from backtide.ui import launch as launch_ui

    launch_ui(
        address=address or cfg.display.address or "localhost",
        port=int(port or cfg.display.port),
    )


@main.command(name="run-experiment")
@click.argument(
    "config",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
)
@click.option(
    "--log_level",
    "-l",
    help="Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.",
)
@click.option(
    "--verbose/--no-verbose",
    "-v",
    default=True,
    show_default=True,
    help="Show a progress bar while the experiment is running.",
)
def run_experiment(config: Path, log_level: str, *, verbose: bool):
    """Run a backtest experiment defined in a configuration file.

    Reads an experiment configuration from a `.toml`, `.yaml`/`.yml` or `.json`
    file, executes the full backtest pipeline — data resolution, indicator
    computation, parallel strategy runs — and persists the results to the local
    database. The outcome can then be explored interactively via `backtide launch`.

    Read more in the [user guide][experiment].

    Parameters
    ----------
    config : Path
        Path to the [experiment configuration][experimentconfig] (`.toml`,
        `.yaml`/`.yml` or `.json`).

    --log_level, -l : str, default="warn"
        Minimum log level to emit. Choose from: `error``, `warn`, `info` or `debug`.

    --verbose/--no-verbose, -v : bool, default=True
        Whether to display a progress bar while the experiment is running.

    See Also
    --------
    - backtide.backtest:ExperimentResult
    - backtide.cli:launch
    - backtide.backtest:run_experiment

    Examples
    --------
    Run an experiment from a TOML config file:
    ```
    backtide run-experiment experiment.toml
    ```

    """
    cfg = get_config()
    init_logging(log_level or cfg.general.log_level)

    text = config.read_text(encoding="utf-8")
    suffix = config.suffix.lower()
    if suffix == ".toml":
        exp_cfg = ExperimentConfig.from_toml(text)
    elif suffix == ".json":
        exp_cfg = ExperimentConfig.from_dict(json.loads(text))
    elif suffix in (".yaml", ".yml"):
        exp_cfg = ExperimentConfig.from_dict(yaml.safe_load(text))
    else:
        raise click.UsageError(
            f"Unsupported config extension {suffix!r}. Use .toml, .yaml/.yml or .json."
        )

    click.echo(f"Running experiment from {config.name}...")

    try:
        result = run_backtest(exp_cfg, verbose=verbose)
    except (KeyboardInterrupt, ExperimentAborted):
        click.echo("\nExperiment aborted. Nothing was stored.", err=True)
        raise SystemExit(130) from None

    n = len(result.strategies)
    if result.status == ExperimentStatus.Success and not result.warnings:
        click.echo(
            f"Done - experiment {result.experiment_id} completed "
            f"({n} strateg{'y' if n == 1 else 'ies'})."
        )
    elif result.status == ExperimentStatus.Success:
        click.echo(
            f"WARNING: Experiment {result.experiment_id} completed with "
            f"{len(result.warnings)} warning(s):"
        )
        for w in result.warnings:
            click.echo(f"   - {w}")
    else:
        click.echo(f"ERROR: Experiment {result.experiment_id} failed.", err=True)
        for w in result.warnings:
            click.echo(f"   - {w}", err=True)
        raise SystemExit(1)


def _read_live_session_config(path: Path) -> dict[str, Any]:
    """Read a live-session configuration mapping from a supported file."""
    text = path.read_text(encoding="utf-8")
    suffix = path.suffix.lower()
    try:
        if suffix == ".toml":
            data = tomllib.loads(text)
        elif suffix == ".json":
            data = json.loads(text)
        elif suffix in (".yaml", ".yml"):
            data = yaml.safe_load(text)
        else:
            raise click.UsageError(
                f"Unsupported config extension {suffix!r}. Use .toml, .yaml/.yml or .json."
            )
    except (json.JSONDecodeError, tomllib.TOMLDecodeError, yaml.YAMLError) as exc:
        raise click.UsageError(f"Invalid live-session configuration: {exc}") from exc

    if not isinstance(data, dict):
        raise click.UsageError("The live-session configuration must be a mapping.")
    return data


def _load_live_strategy(name: Any, cfg: Any) -> Any:
    """Load an optional saved strategy for a CLI live session."""
    if name is None:
        return None
    if not isinstance(name, str) or not name.strip():
        raise click.UsageError("strategy must be the name of a saved strategy.")

    from backtide.strategies.utils import _load_stored_strategies

    strategies = _load_stored_strategies(cfg)
    if name not in strategies:
        raise click.UsageError(f"Saved strategy {name!r} was not found.")
    return strategies[name]


def _write_cli_live_manifest(
    session_id: str,
    *,
    status: str,
    started_at: str,
    config: dict[str, Any],
    snapshot: Any,
    last_message_at: str | None,
    received_events: int,
    error: str | None = None,
) -> None:
    """Persist CLI state using the UI live-session manifest contract."""
    from backtide.live_history import serialize_snapshot, utc_now, write_manifest

    write_manifest(
        session_id,
        {
            "id": session_id,
            "status": status,
            "started_at": started_at,
            "finished_at": utc_now() if status in {"stopped", "error"} else None,
            "config": config,
            "snapshot": serialize_snapshot(snapshot),
            "health": {
                "last_message_at": last_message_at,
                "received_events": received_events,
                "warmup_bars_loaded": 0,
                "replay": None,
            },
            "error": error,
        },
    )


@main.command(name="start-live-session")
@click.argument(
    "config",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
)
@click.option(
    "--log_level",
    "-l",
    help="Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.",
)
def start_live_session(config: Path, log_level: str | None) -> None:
    """Start a WebSocket-backed paper-trading session from a configuration file.

    Reads a live-session configuration from a `.toml`, `.yaml`/`.yml`, or
    `.json` file, connects to the selected public exchange WebSocket, and feeds
    normalized candles into a local [PaperTradingSession]. The command runs
    until interrupted with Ctrl+C; no real orders are submitted. Session state
    and replayable events are saved to the same history used by the application.

    Read more in the [paper-trading guide][paper-trading].

    Parameters
    ----------
    config : Path
        Path to a live-session configuration. Top-level fields are `provider`,
        `symbols`, `interval`, optional saved `strategy`, `batch_size`, and
        `timeout_seconds`. Put [PaperTradingConfig] fields under `paper`.

    --log_level, -l : str, default="warn"
        Minimum log level to emit. Choose from: `error`, `warn`, `info` or
        `debug`.

    See Also
    --------
    - backtide.live:LiveMarketFeed
    - backtide.live:PaperTradingSession
    - backtide.cli:run_experiment

    Examples
    --------
    Create `live.toml`:

    ```toml
    provider = "kraken"
    symbols = ["BTC-USD"]
    interval = "1m"
    strategy = "my-saved-strategy"

    [paper]
    initial_cash = 25000
    commission_pct = 0.1
    slippage = 0.05
    ```

    Start the session and press Ctrl+C to stop it:

    ```console
    backtide start-live-session live.toml
    ```

    """
    cfg = get_config()
    init_logging(log_level or cfg.general.log_level)
    values = _read_live_session_config(config)

    allowed = {
        "provider",
        "symbols",
        "interval",
        "strategy",
        "paper",
        "batch_size",
        "timeout_seconds",
    }
    if unexpected := sorted(values.keys() - allowed):
        raise click.UsageError(f"Unknown live-session field(s): {', '.join(unexpected)}.")

    provider = values.get("provider")
    symbols = values.get("symbols")
    interval = values.get("interval", "1m")
    paper = values.get("paper", {})
    if not isinstance(provider, str) or not provider.strip():
        raise click.UsageError("provider must be a non-empty string.")
    if (
        not isinstance(symbols, list)
        or not symbols
        or not all(isinstance(symbol, str) and symbol.strip() for symbol in symbols)
    ):
        raise click.UsageError("symbols must be a non-empty list of symbol strings.")
    if not isinstance(interval, str) or not interval.strip():
        raise click.UsageError("interval must be a non-empty string.")
    if not isinstance(paper, dict):
        raise click.UsageError("paper must be a mapping of PaperTradingConfig fields.")

    try:
        batch_size = int(values.get("batch_size", 10))
        timeout_seconds = float(values.get("timeout_seconds", 5.0))
    except (TypeError, ValueError) as exc:
        raise click.UsageError("batch_size and timeout_seconds must be numeric.") from exc
    if batch_size <= 0 or timeout_seconds <= 0:
        raise click.UsageError("batch_size and timeout_seconds must be positive.")

    from backtide.live_history import (
        append_event,
        new_session_id,
        serialize_combined_update,
        utc_now,
    )

    feed = None
    session = None
    session_id: str | None = None
    started_at: str | None = None
    history_config: dict[str, Any] = {}
    last_message_at: str | None = None
    received_events = 0
    try:
        trading_config = PaperTradingConfig(**paper)
        strategy = _load_live_strategy(values.get("strategy"), cfg)
        feed = LiveMarketFeed(provider, symbols, interval, include_partial=True)
        base_currency = str(paper.get("base_currency", "USD")).upper()
        inferred_quotes = {symbol: symbol.rsplit("-", 1)[-1].upper() for symbol in symbols}
        target_quotes = inferred_quotes
        conversion_legs: dict[str, tuple[str, str]] = {}
        if any(quote != base_currency for quote in inferred_quotes.values()):
            feed.cancel()
            target_quotes, conversion_legs = _live_currency_plan(
                provider,
                symbols,
                base_currency,
            )
            feed = LiveMarketFeed(
                provider,
                [*symbols, *conversion_legs],
                interval,
                include_partial=True,
            )
        session = PaperTradingSession(trading_config, strategy)
        strategy_name = values.get("strategy")
        strategy_label = str(strategy_name) if strategy_name else "Monitor"
        session_id = new_session_id()
        started_at = utc_now()
        history_config = {
            "mode": "paper",
            "provider": provider,
            "interval": interval,
            "symbols": symbols,
            "strategy": strategy_name,
            "strategies": [strategy_name] if strategy_name else [],
            "indicators": [],
            "config": paper,
            "target_quotes": target_quotes,
            "conversion_legs": {
                symbol: {"base": base, "quote": quote}
                for symbol, (base, quote) in conversion_legs.items()
            },
            "warmup_bars": 0,
        }
        _write_cli_live_manifest(
            session_id,
            status="running",
            started_at=started_at,
            config=history_config,
            snapshot=session.snapshot(),
            last_message_at=last_message_at,
            received_events=received_events,
        )
        observed_conversion_legs: set[str] = set()
        exchange_rates: dict[str, dict[str, Any]] = {}
        click.echo(
            f"Starting live paper session for {', '.join(symbols)} on "
            f"{provider} ({interval}); history id {session_id}. Press Ctrl+C to stop."
        )

        try:
            while True:
                for market in feed.collect(
                    max_events=batch_size,
                    timeout_seconds=timeout_seconds,
                ):
                    last_message_at = utc_now()
                    received_events += 1
                    if market.symbol in conversion_legs:
                        base, quote = conversion_legs[market.symbol]
                        session.set_exchange_rate(base, quote, market.close, market.close_ts)
                        observed_conversion_legs.add(market.symbol)
                        exchange_rates[str(market.symbol)] = {
                            "base": base,
                            "quote": quote,
                            "rate": float(market.close),
                            "timestamp": int(market.close_ts),
                        }
                        if market.symbol not in symbols:
                            continue
                    if set(conversion_legs) - observed_conversion_legs:
                        continue
                    update = session.on_bar(market)
                    persisted_update = serialize_combined_update(
                        market,
                        {strategy_label: update},
                    )
                    persisted_update["exchange_rates"] = dict(exchange_rates)
                    persisted_update["received_at"] = last_message_at
                    append_event(session_id, persisted_update)
                    if update.processed:
                        click.echo(
                            f"{market.symbol} {market.interval} close={market.close:.8g} "
                            f"equity={update.snapshot.equity:.8g} fills={len(update.fills)}"
                        )
        except KeyboardInterrupt:
            click.echo("\nStopping live paper session...")

        snapshot = session.snapshot()
        _write_cli_live_manifest(
            session_id,
            status="stopped",
            started_at=started_at,
            config=history_config,
            snapshot=snapshot,
            last_message_at=last_message_at,
            received_events=received_events,
        )
        click.echo(
            f"Stopped - processed {snapshot.processed_bars} market "
            f"update{'s' if snapshot.processed_bars != 1 else ''}; "
            f"final equity={snapshot.equity:.8g}."
        )
    except (OSError, RuntimeError, TypeError, ValueError) as exc:
        if session_id is not None and started_at is not None:
            try:
                snapshot = session.snapshot() if session is not None else None
                _write_cli_live_manifest(
                    session_id,
                    status="error",
                    started_at=started_at,
                    config=history_config,
                    snapshot=snapshot,
                    last_message_at=last_message_at,
                    received_events=received_events,
                    error=str(exc),
                )
            except (OSError, RuntimeError, TypeError, ValueError):
                # Preserve the original session failure when best-effort error recording fails.
                pass
        raise click.ClickException(str(exc)) from exc
    finally:
        if feed is not None:
            feed.cancel()


if __name__ == "__main__":
    main()
