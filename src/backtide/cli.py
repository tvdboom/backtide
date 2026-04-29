"""Backtide.

Author: Mavs
Description: Entry point for the CLI application.

"""

import json
from pathlib import Path

import click
import yaml
from streamlit.web.bootstrap import run

from backtide.core.config import get_config
from backtide.core.utils import init_logging


@click.group()
def main():
    """CLI application entry point."""


@main.command()
@click.option(
    "--address",
    "-A",
    help=(
        "The address where the server will listen for client and browser connections. "
        "Use this if you want to bind the server to a specific address. If set, the server "
        "will only be available from this address, and not from any aliases (like localhost)."
    ),
)
@click.option(
    "--port",
    "-P",
    help="The port where the server will listen for browser connections.",
)
@click.option(
    "--log_level",
    help="Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.",
)
def launch(address: str, port: str, log_level: str):
    """Launch the Streamlit application.

    Parameters
    ----------
    address : str
        The address where the server will listen for client and browser
        connections. Use this if you want to bind the server to a specific
        address. If set, the server will only be available from this address,
        and not from any aliases (like localhost).

    port : str, default=8501
        TCP port the server listens on.

    log_level : str, default="warn"
        Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.

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
    """Download OHLCV data for one or more symbols.

    Downloads the bars for the requested symbols. Required currency legs are
    automatically downloaded as well.

    Parameters
    ----------
    symbols : tuple[str, ...]
        One or more ticker symbols to download.

    instrument_type : str, default="stocks"
        Instrument type. Choose from: `stocks`, `etf`, `forex` or `crypto`.

    interval : tuple[str, ...], default=("1d",)
        Bar intervals. Can be repeated, e.g., `-i 1d -i 1h`.

    start : str | None, default=None
        Start date in Unix seconds. If `None`, the full available history is
        downloaded.

    end : str | None, default=None
        End date in Unix seconds. Defaults to now.

    log_level : str, default="warn"
        Minimum log level to emit. Choose from: `error`, `warn`, `info` or
        `debug`.

    verbose : bool, default=True
        Show a progress bar while downloading.

    """
    from backtide.data import download_bars, resolve_profiles

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


@main.command(name="run-experiment")
@click.argument(
    "config_file",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
)
@click.option(
    "--log_level",
    help="Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.",
)
@click.option(
    "--verbose/--no-verbose",
    "-v",
    default=True,
    show_default=True,
    help="Show a progress bar while the experiment is running.",
)
def run_experiment_cli(config_file: Path, log_level: str, verbose: bool):
    """Run a backtest experiment from a configuration file.

    Parameters
    ----------
    config_file : Path
        Path to the experiment configuration (`.toml`, `.yaml`/`.yml` or `.json`).

    log_level : str, default="warn"
        Minimum log level to emit. Choose from: `error``, `warn`, `info` or `debug`.

    verbose : bool, default=True
        Show a progress bar while the experiment is running.

    """
    from backtide.backtest import ExperimentConfig, run_experiment

    cfg = get_config()
    init_logging(log_level or cfg.general.log_level)

    text = config_file.read_text(encoding="utf-8")
    suffix = config_file.suffix.lower()
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

    click.echo(f"🚀  Running experiment from {config_file.name}...")
    result = run_experiment(exp_cfg, verbose=verbose)

    n = len(result.strategies)
    if result.status == "completed" and not result.warnings:
        click.echo(
            f"✅  Done — experiment {result.experiment_id} completed "
            f"({n} strateg{'y' if n == 1 else 'ies'})."
        )
    elif result.status == "completed":
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
