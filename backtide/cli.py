"""Backtide.

Author: Mavs
Description: Entry point for the CLI application.

"""

import click
from streamlit.web.bootstrap import run

from backtide.core import init_tracing
from backtide.core.config import get_config


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
    click.echo("🚀  Launching app...")

    cfg = get_config()
    init_tracing(log_level)

    run(
        main_script_path="backtide/ui/app.py",
        is_hello=False,
        args=[],
        flag_options={
            "server.port": port or cfg.display.port,
            "server.address": address or cfg.display.address,
        },
    )


if __name__ == "__main__":
    main()
