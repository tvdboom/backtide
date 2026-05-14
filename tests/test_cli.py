"""Backtide.

Author: Mavs
Description: Unit tests for the CLI commands.

"""

import json
import subprocess
import sys
from unittest.mock import MagicMock, patch

from click.testing import CliRunner
import pytest

from backtide.cli import download, launch, main
from backtide.cli import run_experiment as run_experiment_cmd


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
            patch("backtide.cli.resolve_profiles", return_value=[]),
            patch("backtide.cli.download_bars", return_value=mock_result),
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
            patch("backtide.cli.resolve_profiles", return_value=[]),
            patch("backtide.cli.download_bars", return_value=mock_result),
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
            patch("backtide.cli.resolve_profiles", return_value=[]),
            patch("backtide.cli.download_bars", return_value=mock_result),
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
            patch("backtide.cli.resolve_profiles", return_value=[]),
            patch("backtide.cli.download_bars", return_value=mock_result),
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
        result = runner.invoke(launch, ["-p", "9000"])
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
        result = runner.invoke(launch, ["-a", "0.0.0.0"])
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


# ─────────────────────────────────────────────────────────────────────────────
# run-experiment command
# ─────────────────────────────────────────────────────────────────────────────


class TestRunExperimentCommand:
    """Tests for the `run-experiment` CLI subcommand."""

    @staticmethod
    def _stub_result(status, warnings=None):
        """Build a stub ExperimentResult-like object."""
        from backtide.backtest import ExperimentStatus

        return MagicMock(
            status=ExperimentStatus.Success if status == "success" else ExperimentStatus.Error,
            warnings=warnings or [],
            strategies=[MagicMock()],
            experiment_id="abc123",
        )

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.run_backtest")
    def test_toml_success(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A TOML config runs end-to-end and reports success."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value = self._stub_result("success")

        cfg_path = tmp_path / "exp.toml"
        cfg_path.write_text('[general]\nname = "t"\n', encoding="utf-8")

        result = runner.invoke(run_experiment_cmd, [str(cfg_path)])
        assert result.exit_code == 0, result.output
        assert "Done" in result.output
        assert "completed" in result.output
        mock_run.assert_called_once()

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.run_backtest")
    def test_json_config(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A `.json` config is parsed through `from_dict`."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value = self._stub_result("success")

        cfg_path = tmp_path / "exp.json"
        cfg_path.write_text(json.dumps({"general": {"name": "t"}}), encoding="utf-8")

        result = runner.invoke(run_experiment_cmd, [str(cfg_path)])
        assert result.exit_code == 0
        mock_run.assert_called_once()

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.run_backtest")
    def test_yaml_config(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A `.yaml` config is parsed through `from_dict`."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value = self._stub_result("success")

        cfg_path = tmp_path / "exp.yaml"
        cfg_path.write_text("general:\n  name: t\n", encoding="utf-8")

        result = runner.invoke(run_experiment_cmd, [str(cfg_path)])
        assert result.exit_code == 0
        mock_run.assert_called_once()

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_unsupported_extension(self, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """Unsupported file extensions raise a UsageError."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))

        cfg_path = tmp_path / "exp.ini"
        cfg_path.write_text("[general]\nname=t\n", encoding="utf-8")

        result = runner.invoke(run_experiment_cmd, [str(cfg_path)])
        assert result.exit_code != 0
        assert "Unsupported config extension" in result.output

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.run_backtest")
    def test_success_with_warnings(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A successful run with warnings echoes each warning."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value = self._stub_result("success", warnings=["w1", "w2"])

        cfg_path = tmp_path / "exp.toml"
        cfg_path.write_text("", encoding="utf-8")

        result = runner.invoke(run_experiment_cmd, [str(cfg_path)])
        assert result.exit_code == 0
        assert "warning" in result.output.lower()
        assert "w1" in result.output
        assert "w2" in result.output

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.run_backtest")
    def test_failed_status_exits_nonzero(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A failed run exits with status 1 and writes warnings to stderr."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value = self._stub_result("failed", warnings=["boom"])

        cfg_path = tmp_path / "exp.toml"
        cfg_path.write_text("", encoding="utf-8")

        result = runner.invoke(run_experiment_cmd, [str(cfg_path)])
        assert result.exit_code == 1
        # stderr (mixed_stderr default = True) ends up in `output`.
        assert "failed" in result.output.lower()
        assert "boom" in result.output

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.run_backtest")
    def test_custom_log_level(self, mock_run, mock_logging, mock_cfg, runner, tmp_path):
        """`--log_level` overrides the config value."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value = self._stub_result("success")

        cfg_path = tmp_path / "exp.toml"
        cfg_path.write_text("", encoding="utf-8")

        result = runner.invoke(run_experiment_cmd, [str(cfg_path), "--log_level", "debug"])
        assert result.exit_code == 0
        mock_logging.assert_called_once_with("debug")
