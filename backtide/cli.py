"""Backtide.

Author: Mavs
Description: Entry point for the CLI application.

"""

import click
from streamlit.web.bootstrap import run

from backtide.core import init_tracing


@click.group()
def main():
    """CLI application entry point."""


@main.command()
@click.option(
    "--address",
    "-A",
    default="",
    show_default=True,
    help=(
        "The address where the server will listen for client and browser connections. "
        "Use this if you want to bind the server to a specific address. "
        "If set, the server will only be available from this address, "
        "and not from any aliases (like localhost)."
    ),
)
@click.option(
    "--port",
    "-P",
    default="8501",
    show_default=True,
    help="The port where the server will listen for browser connections.",
)
@click.option(
    "--log_level",
    default="warn",
    show_default=True,
    help="Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.",
)
def launch(address: str, port: str, log_level: str):
    """Launch the Streamlit application.

    Parameters
    ----------
    address : str, default=""
        The address where the server will listen for client and browser
        connections. Use this if you want to bind the server to a specific
        address. If set, the server will only be available from this address,
        and not from any aliases (like localhost).

    port : str, default=8501
        The port where the server will listen for browser connections.

    log_level : str, default="warn"
        Minimum log level to emit. Choose from: `error`, `warn`, `info` or `debug`.

    """
    click.echo("🚀  Launching app...")

    init_tracing(log_level)

    run(
        main_script_path="backtide/ui/app.py",
        is_hello=False,
        args=[],
        flag_options={
            "server.port": port,
            "server.address": address,
        },
    )


if __name__ == "__main__":
    main()
