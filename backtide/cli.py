"""Backtide.

Author: Mavs
Description: Entry point for the CLI application.

"""

import click
from streamlit.web.bootstrap import run


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
def launch(address: str, port: str):
    """Launch the Streamlit application.

    Parameters
    ----------
    address : str
        The address where the server will listen for client and browser
        connections. Use this if you want to bind the server to a specific
        address. If set, the server will only be available from this address,
        and not from any aliases (like localhost).

    port : str
        The port where the server will listen for browser connections.

    """
    click.echo("🚀  Launching app...")

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
