"""Backtide.

Author: Mavs
Description: Unit tests for the CLI commands.

"""

import json
import subprocess
import sys
from types import SimpleNamespace
from unittest.mock import MagicMock, patch

from click.testing import CliRunner
import pytest

from backtide.cli import download, launch, main, run_experiment_command, start_live_session


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

    @patch("backtide.ui.launch")
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_launch_default(self, _mock_logging, mock_cfg, mock_run, runner):  # noqa: PT019
        """Launch with defaults calls the bundled web server."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            display=MagicMock(port=8501, address=None),
        )
        result = runner.invoke(launch)
        assert result.exit_code == 0
        assert result.output.encode("cp1252") == b"Launching app...\n"
        mock_run.assert_called_once()

    @patch("backtide.ui.launch")
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_launch_custom_port(self, _mock_logging, mock_cfg, mock_run, runner):  # noqa: PT019
        """Launch with -p sets a custom port."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            display=MagicMock(port=8501, address=None),
        )
        result = runner.invoke(launch, ["-p", "9000"])
        assert result.exit_code == 0
        assert mock_run.call_args.kwargs["port"] == 9000

    @patch("backtide.ui.launch")
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
        assert mock_run.call_args.kwargs["address"] == "0.0.0.0"

    @patch("backtide.ui.launch")
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
    @patch("backtide.cli.Experiment")
    def test_toml_success(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A TOML config runs end-to-end and reports success."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value.run.return_value = self._stub_result("success")

        cfg_path = tmp_path / "exp.toml"
        cfg_path.write_text('[general]\nname = "t"\n', encoding="utf-8")

        result = runner.invoke(run_experiment_command, [str(cfg_path)])
        assert result.exit_code == 0, result.output
        assert "Done" in result.output
        assert "completed" in result.output
        mock_run.assert_called_once()

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Experiment")
    def test_json_config(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A `.json` config is parsed through `from_dict`."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value.run.return_value = self._stub_result("success")

        cfg_path = tmp_path / "exp.json"
        cfg_path.write_text(json.dumps({"general": {"name": "t"}}), encoding="utf-8")

        result = runner.invoke(run_experiment_command, [str(cfg_path)])
        assert result.exit_code == 0
        mock_run.assert_called_once()

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Experiment")
    def test_yaml_config(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A `.yaml` config is parsed through `from_dict`."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value.run.return_value = self._stub_result("success")

        cfg_path = tmp_path / "exp.yaml"
        cfg_path.write_text("general:\n  name: t\n", encoding="utf-8")

        result = runner.invoke(run_experiment_command, [str(cfg_path)])
        assert result.exit_code == 0
        mock_run.assert_called_once()

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_unsupported_extension(self, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """Unsupported file extensions raise a UsageError."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))

        cfg_path = tmp_path / "exp.ini"
        cfg_path.write_text("[general]\nname=t\n", encoding="utf-8")

        result = runner.invoke(run_experiment_command, [str(cfg_path)])
        assert result.exit_code != 0
        assert "Unsupported config extension" in result.output

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Experiment")
    def test_success_with_warnings(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A successful run with warnings echoes each warning."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value.run.return_value = self._stub_result(
            "success", warnings=["w1", "w2"]
        )

        cfg_path = tmp_path / "exp.toml"
        cfg_path.write_text("", encoding="utf-8")

        result = runner.invoke(run_experiment_command, [str(cfg_path)])
        assert result.exit_code == 0
        assert "warning" in result.output.lower()
        assert "w1" in result.output
        assert "w2" in result.output

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Experiment")
    def test_failed_status_exits_nonzero(self, mock_run, _mock_log, mock_cfg, runner, tmp_path):  # noqa: PT019
        """A failed run exits with status 1 and writes warnings to stderr."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value.run.return_value = self._stub_result("failed", warnings=["boom"])

        cfg_path = tmp_path / "exp.toml"
        cfg_path.write_text("", encoding="utf-8")

        result = runner.invoke(run_experiment_command, [str(cfg_path)])
        assert result.exit_code == 1
        # stderr (mixed_stderr default = True) ends up in `output`.
        assert "failed" in result.output.lower()
        assert "boom" in result.output

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Experiment")
    def test_custom_log_level(self, mock_run, mock_logging, mock_cfg, runner, tmp_path):
        """`--log_level` overrides the config value."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_run.return_value.run.return_value = self._stub_result("success")

        cfg_path = tmp_path / "exp.toml"
        cfg_path.write_text("", encoding="utf-8")

        result = runner.invoke(run_experiment_command, [str(cfg_path), "--log_level", "debug"])
        assert result.exit_code == 0
        mock_logging.assert_called_once_with("debug")


class TestStartLiveSessionCommand:
    """Tests for the `start-live-session` CLI subcommand."""

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_toml_session_runs_until_interrupted(
        self,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
    ):
        """A TOML session processes updates and cancels its feed on Ctrl+C."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            data=SimpleNamespace(storage_path=tmp_path),
        )
        market = SimpleNamespace(symbol="BTC-USD", interval="1m", close=101.25)

        class Feed:
            canceled = False

            def __init__(self, provider, symbols, interval, *, include_partial):
                assert (provider, symbols, interval, include_partial) == (
                    "kraken",
                    ["BTC-USD"],
                    "1m",
                    True,
                )
                self.calls = 0

            def collect(self, *, max_events, timeout_seconds):
                assert (max_events, timeout_seconds) == (4, 2.0)
                self.calls += 1
                if self.calls == 1:
                    return [market]
                raise KeyboardInterrupt

            def cancel(self):
                Feed.canceled = True

        class Session:
            def __init__(self, config, strategy):
                assert config == "session-config"
                assert strategy is None

            def on_bar(self, value):
                assert value is market
                snapshot = SimpleNamespace(equity=101.25)
                return SimpleNamespace(
                    market=value,
                    processed=True,
                    snapshot=snapshot,
                    fills=[],
                    orders_submitted=0,
                    indicators={},
                )

            def snapshot(self):
                return SimpleNamespace(processed_bars=1, equity=101.25)

        cfg_path = tmp_path / "live.toml"
        cfg_path.write_text(
            'provider = "kraken"\nsymbols = ["BTC-USD"]\nbatch_size = 4\ntimeout_seconds = 2\n',
            encoding="utf-8",
        )

        with (
            patch("backtide.cli.LiveMarketFeed", Feed),
            patch("backtide.cli.SessionConfig", return_value="session-config"),
            patch("backtide.cli.Session", Session),
            patch("backtide.live_history.new_session_id", return_value="0123456789abcdef"),
        ):
            result = runner.invoke(start_live_session, [str(cfg_path)])

        assert result.exit_code == 0, result.output
        assert "Starting live session" in result.output
        assert "BTC-USD 1m close=101.25 equity=101.25 fills=0" in result.output
        assert "processed 1 market update" in result.output
        assert Feed.canceled
        from backtide.ui.live import LiveTradingManager

        manager = LiveTradingManager()
        persisted = manager.session("0123456789abcdef")
        manifest = persisted
        events = persisted["updates"]
        assert manifest["status"] == "stopped"
        assert manifest["snapshot"]["equity"] == 101.25
        assert manifest["health"]["received_events"] == 1
        assert events[0]["market"]["symbol"] == "BTC-USD"
        assert events[0]["strategies"]["Monitor"]["snapshot"]["equity"] == 101.25
        manager.delete_session("0123456789abcdef")
        mock_logging.assert_called_once_with("warn")

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_provider_validation_error_is_user_facing(
        self,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
    ):
        """An unsupported provider exits cleanly with the feed's explanation."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        cfg_path = tmp_path / "live.json"
        cfg_path.write_text(
            json.dumps({"provider": "yahoo", "symbols": ["AAPL"]}),
            encoding="utf-8",
        )

        with patch(
            "backtide.cli.LiveMarketFeed",
            side_effect=ValueError("Yahoo Finance does not expose an official live WebSocket."),
        ):
            result = runner.invoke(start_live_session, [str(cfg_path)])

        assert result.exit_code == 1
        assert "Yahoo Finance does not expose an official live WebSocket" in result.output
        mock_logging.assert_called_once_with("warn")

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_symbols_are_required(self, mock_logging, mock_cfg, runner, tmp_path):
        """A session config must define at least one symbol."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        cfg_path = tmp_path / "live.yaml"
        cfg_path.write_text("provider: kraken\n", encoding="utf-8")

        result = runner.invoke(start_live_session, [str(cfg_path)])

        assert result.exit_code == 2
        assert "symbols must be a non-empty list" in result.output
        mock_logging.assert_called_once_with("warn")
