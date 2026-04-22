"""Backtide.

Author: Mavs
Description: Unit tests for the CLI commands.

"""

from pathlib import Path
import subprocess
import sys
from unittest.mock import MagicMock, patch

from click.testing import CliRunner
import pytest

from backtide.cli import download, launch, main


@pytest.fixture
def runner():
    """Create a CLI test runner."""
    return CliRunner()


class TestMainGroup:
    """Tests for the CLI main group."""

    def test_help(self, runner):
        """Test --help flag."""
        result = runner.invoke(main, ["--help"])
        assert result.exit_code == 0
        assert "CLI application" in result.output


class TestDownload:
    """Tests for the 'download' CLI command."""

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_success(self, _mock_logging, mock_cfg, runner):  # noqa: PT019
        """Download succeeds with mocked resolve/download."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            display=MagicMock(port=8501, address=None),
        )
        mock_result = MagicMock(n_succeeded=1, n_failed=0, warnings=[])
        with (
            patch("backtide.data.resolve_profiles", return_value=[]),
            patch("backtide.data.download_bars", return_value=mock_result),
        ):
            result = runner.invoke(download, ["AAPL"])
            assert result.exit_code == 0
            assert "Done" in result.output

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_partial_failure(self, _mock_logging, mock_cfg, runner):  # noqa: PT019
        """Partial failure shows count."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
        )
        mock_result = MagicMock(n_succeeded=1, n_failed=1, warnings=["timeout"])
        with (
            patch("backtide.data.resolve_profiles", return_value=[]),
            patch("backtide.data.download_bars", return_value=mock_result),
        ):
            result = runner.invoke(download, ["AAPL", "BAD"])
            assert result.exit_code == 0

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_all_failure(self, _mock_logging, mock_cfg, runner):  # noqa: PT019
        """All failures show error message."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
        )
        mock_result = MagicMock(n_succeeded=0, n_failed=2, warnings=["err1", "err2"])
        with (
            patch("backtide.data.resolve_profiles", return_value=[]),
            patch("backtide.data.download_bars", return_value=mock_result),
        ):
            result = runner.invoke(download, ["A", "B"])
            assert result.exit_code == 0

    @pytest.mark.parametrize("instrument_type", ["stocks", "etf", "forex", "crypto"])
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_instrument_type_param(self, _mock_logging, mock_cfg, runner, instrument_type):  # noqa: PT019
        """--instrument-type flag is forwarded correctly."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_result = MagicMock(n_succeeded=1, n_failed=0, warnings=[])
        with (
            patch("backtide.data.resolve_profiles", return_value=[]),
            patch("backtide.data.download_bars", return_value=mock_result),
        ):
            result = runner.invoke(download, ["AAPL", "-t", instrument_type])
            assert result.exit_code == 0


class TestLaunch:
    """Tests for the 'launch' CLI command."""

    @patch("backtide.cli.run")
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_launch_default(self, _mock_logging, mock_cfg, mock_run, runner):  # noqa: PT019
        """Launch with defaults calls run()."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            display=MagicMock(port=8501, address=None),
        )
        result = runner.invoke(launch)
        assert result.exit_code == 0
        mock_run.assert_called_once()

    @patch("backtide.cli.run")
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_launch_custom_port(self, _mock_logging, mock_cfg, mock_run, runner):  # noqa: PT019
        """Launch with -P flag sets custom port."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            display=MagicMock(port=8501, address=None),
        )
        result = runner.invoke(launch, ["-P", "9000"])
        assert result.exit_code == 0
        call_kwargs = mock_run.call_args
        assert call_kwargs[1]["flag_options"]["server.port"] == "9000"

    @patch("backtide.cli.run")
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_launch_custom_address(self, _mock_logging, mock_cfg, mock_run, runner):  # noqa: PT019
        """Launch with custom address passes it to flag_options."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            display=MagicMock(port=8501, address=None),
        )
        result = runner.invoke(launch, ["-A", "0.0.0.0"])
        assert result.exit_code == 0
        assert mock_run.call_args[1]["flag_options"]["server.address"] == "0.0.0.0"

    @patch("backtide.cli.run")
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_launch_custom_log_level(self, mock_logging, mock_cfg, _mock_run, runner):  # noqa: PT019
        """Launch with --log_level uses that instead of config."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            display=MagicMock(port=8501, address=None),
        )
        result = runner.invoke(launch, ["--log_level", "debug"])
        assert result.exit_code == 0
        mock_logging.assert_called_once_with("debug")


class TestMainBlock:
    """Test the __main__ guard."""

    def test_main_invoked(self):
        """The main() function is called when run as __main__."""
        result = subprocess.run(
            [sys.executable, "-m", "backtide.cli", "--help"],
            capture_output=True,
            text=True,
            timeout=10,
        )

        assert result.returncode == 0
        assert "CLI application" in result.stdout
