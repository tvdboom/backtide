"""Backtide.

Author: Mavs
Description: Entry point for the CLI application.

"""

import json
from pathlib import Path

import click
from streamlit.web.bootstrap import run
import yaml

from backtide.backtest import ExperimentConfig, ExperimentStatus
from backtide.backtest import run_experiment as run_backtest
from backtide.core.config import get_config
from backtide.core.utils import init_logging
from backtide.data import download_bars, resolve_profiles


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
        click.echo(f"   ⚠️  {warn}", err=True)

    if result.n_failed and result.n_succeeded:
        click.echo(
            f"✅  Done ({result.n_succeeded}/{result.n_succeeded + result.n_failed} "
            f"instruments downloaded).",
        )
    elif result.n_failed:
        click.echo(f"❌  All {result.n_failed} downloads failed.", err=True)
    else:
        click.echo("✅  Done.")


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

    Starts the Streamlit-based graphical interface, which lets you browse stored
    experiments, inspect equity curves, trade logs, and performance metrics without
    writing any code.

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

    click.echo("🚀  Launching app...")

    run(
        main_script_path=str(Path(__file__).resolve().parent / "ui" / "app.py"),
        is_hello=False,
        args=[],
        flag_options={
            "server.port": port or cfg.display.port,
            "server.address": address or cfg.display.address,
        },
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
    database.  The outcome can then be explored interactively via `backtide launch`.

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

    click.echo(f"🚀  Running experiment from {config.name}...")
    result = run_backtest(exp_cfg, verbose=verbose)

    n = len(result.strategies)
    if result.status == ExperimentStatus.Success and not result.warnings:
        click.echo(
            f"✅  Done — experiment {result.experiment_id} completed "
            f"({n} strateg{'y' if n == 1 else 'ies'})."
        )
    elif result.status == ExperimentStatus.Success:
        click.echo(
            f"⚠️  Experiment {result.experiment_id} completed with "
            f"{len(result.warnings)} warning(s):"
        )
        for w in result.warnings:
            click.echo(f"   - {w}")
    else:
        click.echo(f"❌  Experiment {result.experiment_id} failed.", err=True)
        for w in result.warnings:
            click.echo(f"   - {w}", err=True)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
