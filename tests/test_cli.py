"""Backtide.

Author: Mavs
Description: Unit tests for the CLI commands.

"""

from unittest.mock import MagicMock, patch

import pytest
from click.testing import CliRunner

from backtide.cli import download, launch, main


@pytest.fixture
def runner():
    return CliRunner()


class TestMainGroup:
    """Tests for the CLI main group."""

    def test_help(self, runner):
        result = runner.invoke(main, ["--help"])
        assert result.exit_code == 0
        assert "CLI application" in result.output


class TestDownload:
    """Tests for the 'download' CLI command."""

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_success(self, mock_logging, mock_cfg, runner):
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
    def test_partial_failure(self, mock_logging, mock_cfg, runner):
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
    def test_all_failure(self, mock_logging, mock_cfg, runner):
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
    def test_instrument_type_param(self, mock_logging, mock_cfg, runner, instrument_type):
        """--instrument-type flag is forwarded correctly."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_result = MagicMock(n_succeeded=1, n_failed=0, warnings=[])
        with (
            patch("backtide.data.resolve_profiles", return_value=[]) as mock_resolve,
            patch("backtide.data.download_bars", return_value=mock_result),
        ):
            result = runner.invoke(download, ["AAPL", "-t", instrument_type])
            assert result.exit_code == 0


class TestLaunch:
    """Tests for the 'launch' CLI command."""

    @patch("backtide.cli.run")
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_launch_default(self, mock_logging, mock_cfg, mock_run, runner):
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
    def test_launch_custom_port(self, mock_logging, mock_cfg, mock_run, runner):
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            display=MagicMock(port=8501, address=None),
        )
        result = runner.invoke(launch, ["-P", "9000"])
        assert result.exit_code == 0
        call_kwargs = mock_run.call_args
        assert call_kwargs[1]["flag_options"]["server.port"] == "9000"

