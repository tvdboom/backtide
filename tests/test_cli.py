"""Backtide.

Author: Mavs
Description: Unit tests for the CLI commands.

"""

import json
import runpy
from types import SimpleNamespace
from typing import Any, ClassVar
from unittest.mock import MagicMock, patch
import warnings

import click
from click.testing import CliRunner
import pytest

from backtide.backtest import ExperimentAborted, WalkForwardConfig
import backtide.cli as cli_module
from backtide.cli import (
    download,
    launch,
    main,
    run_experiment_command,
    run_study_command,
    start_live_session,
)


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

    def test_main_invoked(self, monkeypatch):
        """The main() function is called when run as __main__."""
        calls = []
        monkeypatch.setattr(click.Group, "__call__", lambda group: calls.append(group.name))

        with warnings.catch_warnings():
            warnings.simplefilter("ignore", RuntimeWarning)
            runpy.run_module("backtide.cli", run_name="__main__")

        assert calls == ["main"]


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

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Experiment")
    def test_abort_exits_with_shell_interrupt_status(
        self,
        mock_experiment,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
    ):
        """An interrupted experiment exits with the conventional shell status."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_experiment.return_value.run.side_effect = ExperimentAborted("stopped")
        config = tmp_path / "experiment.toml"
        config.write_text("", encoding="utf-8")

        result = runner.invoke(run_experiment_command, [str(config)])

        assert result.exit_code == 130
        assert "Experiment aborted" in result.output
        mock_logging.assert_called_once_with("warn")


class TestRunStudyCommand:
    """Tests for the `run-study` CLI subcommand."""

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Study")
    def test_toml_study_runs_and_reports_result(
        self,
        mock_study,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
    ):
        """A TOML study config constructs and runs the public Study API."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_study.return_value.run.return_value = SimpleNamespace(
            study_id="study-1",
            candidates=[object(), object()],
            folds=[object()],
            best_candidate_id="candidate-2",
            warnings=[],
        )
        cfg_path = tmp_path / "study.toml"
        cfg_path.write_text(
            """
[config]
metrics = ["sharpe"]
[config.general]
name = "Study test"
[config.strategy]
strategies = ["Saved strategy"]
[study]
min_trades = 2
max_drawdown = 0.25
[study.parameter_space]
lookback = [10, 20]
[study.walk_forward]
training_days = 100
test_days = 20
""".strip(),
            encoding="utf-8",
        )

        result = runner.invoke(run_study_command, [str(cfg_path), "--no-verbose"])

        assert result.exit_code == 0, result.output
        assert "study study-1 completed (2 candidates, 1 walk-forward fold)" in result.output
        assert "Best candidate: candidate-2" in result.output
        mock_logging.assert_called_once_with("warn")
        mock_study.return_value.run.assert_called_once_with(verbose=False)
        assert mock_study.call_args.kwargs["parameter_space"] == {"lookback": [10, 20]}
        assert mock_study.call_args.kwargs["min_trades"] == 2
        assert mock_study.call_args.kwargs["max_drawdown"] == 0.25
        assert mock_study.call_args.kwargs["walk_forward"] == WalkForwardConfig(
            training_days=100,
            test_days=20,
        )

    @pytest.mark.parametrize(
        ("filename", "contents"),
        [
            (
                "study.json",
                json.dumps(
                    {
                        "config": {"metrics": ["sharpe"]},
                        "study": {"parameter_space": {"lookback": [10, 20]}},
                    }
                ),
            ),
            (
                "study.yaml",
                (
                    "config:\n  metrics: [sharpe]\n"
                    "study:\n  parameter_space:\n    lookback: [10, 20]\n"
                ),
            ),
        ],
    )
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Study")
    def test_json_and_yaml_configs(
        self,
        mock_study,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
        filename,
        contents,
    ):
        """JSON and YAML study files use the same command schema."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_study.return_value.run.return_value = SimpleNamespace(
            study_id="study-1",
            candidates=[object(), object()],
            folds=[],
            best_candidate_id="candidate-1",
            warnings=[],
        )
        cfg_path = tmp_path / filename
        cfg_path.write_text(contents, encoding="utf-8")

        result = runner.invoke(run_study_command, [str(cfg_path)])

        assert result.exit_code == 0, result.output
        assert mock_study.call_args.kwargs["parameter_space"] == {"lookback": [10, 20]}
        mock_logging.assert_called_once_with("warn")

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_requires_config_and_study_mappings(
        self,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
    ):
        """A malformed top-level file reports an actionable usage error."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        cfg_path = tmp_path / "study.toml"
        cfg_path.write_text("[study.parameter_space]\nlookback = [10, 20]\n", encoding="utf-8")

        result = runner.invoke(run_study_command, [str(cfg_path)])

        assert result.exit_code == 2
        assert "require config and study mappings" in result.output
        mock_logging.assert_called_once_with("warn")

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Study")
    def test_abort_exits_with_shell_interrupt_status(
        self,
        mock_study,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
    ):
        """An interrupted study exits with status 130."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        mock_study.return_value.run.side_effect = ExperimentAborted("stopped")
        cfg_path = tmp_path / "study.json"
        cfg_path.write_text(
            json.dumps(
                {
                    "config": {"metrics": ["sharpe"]},
                    "study": {"parameter_space": {"lookback": [10]}},
                }
            ),
            encoding="utf-8",
        )

        result = runner.invoke(run_study_command, [str(cfg_path)])

        assert result.exit_code == 130
        assert "Study aborted" in result.output
        mock_logging.assert_called_once_with("warn")

    @pytest.mark.parametrize(
        ("payload", "message"),
        [
            (
                {"config": {}, "study": {"parameter_space": {}}, "unexpected": True},
                "Unknown study file field",
            ),
            (
                {"config": {}, "study": {"parameter_space": {}, "unexpected": True}},
                "Unknown study field",
            ),
            ({"config": {}, "study": {"parameter_space": []}}, "must be a mapping"),
            (
                {
                    "config": {},
                    "study": {"parameter_space": {}, "walk_forward": []},
                },
                "walk_forward must be a mapping",
            ),
        ],
    )
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_invalid_study_fields_are_rejected(
        self,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
        payload,
        message,
    ):
        """Study command validation rejects unknown and malformed fields."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        config = tmp_path / "study.json"
        config.write_text(json.dumps(payload), encoding="utf-8")

        result = runner.invoke(run_study_command, [str(config)])

        assert result.exit_code == 2
        assert message in result.output
        mock_logging.assert_called_once_with("warn")

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    @patch("backtide.cli.Study")
    def test_study_errors_and_warnings_are_user_facing(
        self,
        mock_study,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
    ):
        """Completed study warnings and runtime validation errors are printed clearly."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        config = tmp_path / "study.json"
        config.write_text(
            json.dumps({"config": {}, "study": {"parameter_space": {"x": [1]}}}),
            encoding="utf-8",
        )
        mock_study.return_value.run.return_value = SimpleNamespace(
            study_id="study-1",
            candidates=[object()],
            folds=[object(), object()],
            best_candidate_id=None,
            warnings=["candidate failed"],
        )

        result = runner.invoke(run_study_command, [str(config)])

        assert result.exit_code == 0
        assert "no eligible candidate" in result.output
        assert "candidate failed" in result.output

        mock_study.return_value.run.side_effect = ValueError("invalid sweep")
        failed = runner.invoke(run_study_command, [str(config)])
        assert failed.exit_code == 1
        assert "invalid sweep" in failed.output
        assert mock_logging.call_count == 2


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
    def test_live_session_waits_for_conversion_legs_and_loads_metrics(
        self,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
        monkeypatch,
    ):
        """Non-base quotes initialize conversion feeds before processing targets."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            data=SimpleNamespace(storage_path=tmp_path),
        )
        leg = SimpleNamespace(
            symbol="ETH-EUR",
            interval="1m",
            close=2_000.0,
            close_ts=100,
        )
        target = SimpleNamespace(
            symbol="AAVE-ETH",
            interval="1m",
            close=0.05,
            close_ts=101,
        )

        class Feed:
            created: ClassVar[list[Any]] = []

            def __init__(self, _provider, symbols, _interval, *, include_partial):
                assert include_partial
                self.symbols = symbols
                self.calls = 0
                self.canceled = False
                Feed.created.append(self)

            def collect(self, **_kwargs):
                self.calls += 1
                if self.calls == 1:
                    return [leg, target]
                raise KeyboardInterrupt

            def cancel(self):
                self.canceled = True

        class Session:
            rates: ClassVar[list[tuple[str, str, float, int]]] = []

            def __init__(self, config, strategy):
                assert config == "session-config"
                assert strategy is None

            def set_exchange_rate(self, base, quote, rate, timestamp):
                Session.rates.append((base, quote, rate, timestamp))

            def on_bar(self, market):
                assert market is target
                return SimpleNamespace(
                    processed=False,
                    snapshot=SimpleNamespace(equity=100.0),
                    fills=[],
                )

            def snapshot(self):
                return SimpleNamespace(processed_bars=0, equity=100.0)

        cfg_path = tmp_path / "conversion.json"
        cfg_path.write_text(
            json.dumps(
                {
                    "provider": "kraken",
                    "symbols": ["AAVE-ETH"],
                    "session": {"base_currency": "EUR", "metrics": ["sharpe"]},
                }
            ),
            encoding="utf-8",
        )
        events = []
        monkeypatch.setattr(cli_module, "LiveMarketFeed", Feed)
        monkeypatch.setattr(cli_module, "SessionConfig", lambda **_kwargs: "session-config")
        monkeypatch.setattr(cli_module, "Session", Session)
        monkeypatch.setattr(cli_module, "_load_session_metrics", lambda values, _cfg: values)
        monkeypatch.setattr(
            cli_module,
            "_live_currency_plan",
            lambda *_args: (
                {"AAVE-ETH": "ETH"},
                {"ETH-EUR": ("ETH", "EUR")},
            ),
        )
        monkeypatch.setattr(cli_module, "_write_cli_live_manifest", lambda *_args, **_kwargs: None)
        monkeypatch.setattr("backtide.live_history.new_session_id", lambda: "session")
        monkeypatch.setattr("backtide.live_history.utc_now", lambda: "now")
        monkeypatch.setattr(
            "backtide.live_history.serialize_combined_update",
            lambda market, _updates: {"market": {"symbol": market.symbol}},
        )
        monkeypatch.setattr(
            "backtide.live_history.append_event",
            lambda session_id, update: events.append((session_id, update)),
        )

        result = runner.invoke(start_live_session, [str(cfg_path)])

        assert result.exit_code == 0, result.output
        assert Feed.created[0].canceled
        assert Feed.created[1].symbols == ["AAVE-ETH", "ETH-EUR"]
        assert Session.rates == [("ETH", "EUR", 2_000.0, 100)]
        assert events[0][1]["exchange_rates"]["ETH-EUR"]["rate"] == 2_000.0
        mock_logging.assert_called_once_with("warn")

    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_live_session_preserves_runtime_error_when_error_manifest_fails(
        self,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
        monkeypatch,
    ):
        """Best-effort error persistence never replaces the original feed failure."""
        mock_cfg.return_value = MagicMock(
            general=MagicMock(log_level="warn"),
            data=SimpleNamespace(storage_path=tmp_path),
        )

        class Feed:
            def __init__(self, *_args, **_kwargs):
                self.canceled = False

            def collect(self, **_kwargs):
                raise RuntimeError("feed failed")

            def cancel(self):
                self.canceled = True

        session = SimpleNamespace(snapshot=lambda: SimpleNamespace(equity=1.0))
        writes = []

        def write_manifest(*_args, **kwargs):
            writes.append(kwargs["status"])
            if kwargs["status"] == "error":
                raise OSError("disk failed")

        cfg_path = tmp_path / "failure.json"
        cfg_path.write_text(
            json.dumps({"provider": "kraken", "symbols": ["BTC-USD"]}),
            encoding="utf-8",
        )
        monkeypatch.setattr(cli_module, "LiveMarketFeed", Feed)
        monkeypatch.setattr(cli_module, "SessionConfig", lambda **_kwargs: object())
        monkeypatch.setattr(cli_module, "Session", lambda *_args: session)
        monkeypatch.setattr(cli_module, "_write_cli_live_manifest", write_manifest)
        monkeypatch.setattr("backtide.live_history.new_session_id", lambda: "session")
        monkeypatch.setattr("backtide.live_history.utc_now", lambda: "now")

        result = runner.invoke(start_live_session, [str(cfg_path)])

        assert result.exit_code == 1
        assert "feed failed" in result.output
        assert writes == ["running", "error"]
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

    @pytest.mark.parametrize(
        ("payload", "message"),
        [
            ({"provider": "kraken", "symbols": ["BTC-USD"], "extra": 1}, "Unknown"),
            ({"provider": "", "symbols": ["BTC-USD"]}, "provider must be"),
            ({"provider": "kraken", "symbols": ["BTC-USD"], "interval": ""}, "interval"),
            (
                {"provider": "kraken", "symbols": ["BTC-USD"], "session": []},
                "session must be a mapping",
            ),
            (
                {"provider": "kraken", "symbols": ["BTC-USD"], "batch_size": "bad"},
                "must be numeric",
            ),
            (
                {"provider": "kraken", "symbols": ["BTC-USD"], "timeout_seconds": 0},
                "must be positive",
            ),
        ],
    )
    @patch("backtide.cli.get_config")
    @patch("backtide.cli.init_logging")
    def test_invalid_live_fields_are_rejected(
        self,
        mock_logging,
        mock_cfg,
        runner,
        tmp_path,
        payload,
        message,
    ):
        """Live session configuration rejects malformed fields before connecting."""
        mock_cfg.return_value = MagicMock(general=MagicMock(log_level="warn"))
        config = tmp_path / "live.json"
        config.write_text(json.dumps(payload), encoding="utf-8")

        result = runner.invoke(start_live_session, [str(config)])

        assert result.exit_code == 2
        assert message in result.output
        mock_logging.assert_called_once_with("warn")


class TestCliConfigurationHelpers:
    """Tests for CLI file parsing and reusable live-session helpers."""

    @pytest.mark.parametrize(
        ("reader", "filename", "contents", "message"),
        [
            (cli_module._read_study_config, "study.txt", "{}", "Unsupported"),
            (cli_module._read_study_config, "study.json", "{", "Invalid study"),
            (cli_module._read_study_config, "study.yaml", "[]", "must be a mapping"),
            (cli_module._read_live_session_config, "live.txt", "{}", "Unsupported"),
            (cli_module._read_live_session_config, "live.json", "{", "Invalid live-session"),
            (cli_module._read_live_session_config, "live.yaml", "[]", "must be a mapping"),
        ],
    )
    def test_config_readers_report_invalid_files(
        self,
        reader,
        filename,
        contents,
        message,
        tmp_path,
    ):
        """CLI configuration readers translate format and shape errors."""
        path = tmp_path / filename
        path.write_text(contents, encoding="utf-8")

        with pytest.raises(click.UsageError, match=message):
            reader(path)

    def test_live_strategy_loader_handles_optional_and_saved_names(self, monkeypatch):
        """The live strategy loader accepts omission and resolves a saved name."""
        config = object()
        strategy = object()
        monkeypatch.setattr(
            "backtide.strategies.utils._load_stored_strategies",
            lambda _cfg: {"Saved": strategy},
        )

        assert cli_module._load_live_strategy(None, config) is None
        assert cli_module._load_live_strategy("Saved", config) is strategy
        with pytest.raises(click.UsageError, match="name of a saved strategy"):
            cli_module._load_live_strategy([], config)
        with pytest.raises(click.UsageError, match="was not found"):
            cli_module._load_live_strategy("Missing", config)

    def test_session_metric_loader_resolves_builtin_and_saved_metrics(self, monkeypatch):
        """CLI live metrics preserve built-ins and wrap saved custom objects."""
        metric = object()
        monkeypatch.setattr(
            "backtide.metrics.utils._load_stored_metrics",
            lambda _cfg: {"custom": metric},
        )

        assert cli_module._load_session_metrics(["sharpe", "custom"], object()) == [
            "sharpe",
            {"custom": metric},
        ]
        with pytest.raises(click.UsageError, match="Metric 'missing' was not found"):
            cli_module._load_session_metrics(["missing"], object())

    def test_live_manifest_helper_serializes_terminal_state(self, monkeypatch):
        """CLI live manifests include terminal time, health, snapshot, and errors."""
        written = []
        monkeypatch.setattr("backtide.live_history.utc_now", lambda: "finished")
        monkeypatch.setattr(
            "backtide.live_history.serialize_snapshot",
            lambda snapshot: {"snapshot": snapshot},
        )
        monkeypatch.setattr(
            "backtide.live_history.write_manifest",
            lambda session_id, value: written.append((session_id, value)),
        )

        cli_module._write_cli_live_manifest(
            "session",
            status="error",
            started_at="started",
            config={"provider": "kraken"},
            snapshot="value",
            last_message_at="message",
            received_events=4,
            error="failed",
        )

        assert written[0][1]["finished_at"] == "finished"
        assert written[0][1]["snapshot"] == {"snapshot": "value"}
        assert written[0][1]["health"]["received_events"] == 4
        assert written[0][1]["error"] == "failed"
