"""Backtide.

Author: Mavs
Description: Entry point for the CLI application.

"""

from datetime import UTC, datetime

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
    "--asset-type",
    "-t",
    default="crypto",
    show_default=True,
    help="Asset type: stocks, etf, forex, crypto.",
)
@click.option(
    "--interval",
    "-i",
    multiple=True,
    default=("1d",),
    show_default=True,
    help="Bar interval(s). Can be repeated, e.g. -i 1d -i 1h.",
)
@click.option(
    "--start",
    "-s",
    required=True,
    help="Start date (YYYY-MM-DD) or Unix timestamp.",
)
@click.option(
    "--end",
    "-e",
    default=None,
    help="End date (YYYY-MM-DD) or Unix timestamp. Defaults to now.",
)
@click.option(
    "--log_level",
    help="Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.",
)
def download(symbols, asset_type, interval, start, end, log_level):
    """Download OHLCV data for one or more symbols.

    SYMBOLS are canonical symbol names, e.g. BTC-USDT AAPL.

    Examples
    --------
        backtide download BTC-USDT ETH-USDT -t crypto -i 1d -s 2020-01-01

        backtide download AAPL MSFT -t stocks -i 1d -i 1h -s 2023-01-01 -e 2024-01-01
    """
    from backtide.data import download_assets, get_download_info

    cfg = get_config()
    init_logging(log_level or cfg.general.log_level)

    # Parse dates
    start_ts = _parse_timestamp(start)
    end_ts = _parse_timestamp(end) if end else None

    intervals = list(interval)
    symbols = list(symbols)

    click.echo("📊  Resolving downloads...")

    info = get_download_info(symbols, asset_type, intervals)

    n_assets = len(info.assets)
    n_legs = len(info.legs)
    click.echo(f"   --> {n_assets} assets.")
    click.echo(f"   --> {n_legs} legs.")

    click.echo("⬇️  Downloading …")
    result = download_assets(info, start=start_ts, end=end_ts)

    for warn in result.warnings:
        click.echo(f"   ⚠️  {warn}", err=True)

    if result.n_failed and result.n_succeeded:
        click.echo(
            f"✅  Done ({result.n_succeeded}/{result.n_succeeded + result.n_failed} "
            f"assets downloaded).",
        )
    elif result.n_failed:
        click.echo(f"❌  All {result.n_failed} downloads failed.", err=True)
    else:
        click.echo("✅  Done.")


def _parse_timestamp(value: str) -> int:
    """Parse a date string (YYYY-MM-DD) or raw Unix timestamp."""
    try:
        return int(value)
    except ValueError:
        pass
    try:
        dt = datetime.strptime(value, "%Y-%m-%d").replace(tzinfo=UTC)
        return int(dt.timestamp())
    except ValueError:
        raise click.BadParameter(
            f"Cannot parse '{value}' as YYYY-MM-DD or Unix timestamp.",
        )


if __name__ == "__main__":
    main()
