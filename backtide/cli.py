"""Backtide.

Author: Mavs
Description: Entry point for the CLI application.

"""

import click
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

    port : str
        TCP port the server listens on.

    log_level : str
        Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.

    """
    cfg = get_config()
    init_logging(log_level or cfg.general.log_level)

    click.echo("🚀  Launching app...")

    run(
        main_script_path="backtide/ui/app.py",
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
    help="Show progress bars during resolve and download.",
)
def download(symbols, instrument_type, interval, start, end, log_level, verbose):
    """Download OHLCV data for one or more symbols.

    Downloads the bars for the requested symbols. Required currency legs are
    automatically downloaded as well.

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


if __name__ == "__main__":
    main()
