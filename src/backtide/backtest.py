"""Backtide.

Author: Mavs
Description: Public Python interface for the backtest module.
"""

from __future__ import annotations

import threading
from typing import Any
import uuid

from backtide.core.backtest import (
    CommissionType,
    ConversionPeriod,
    CurrencyConversionMode,
    DataExpConfig,
    EmptyBarPolicy,
    EngineExpConfig,
    EquitySample,
    ExchangeExpConfig,
    ExperimentConfig,
    ExperimentResult,
    ExperimentStatus,
    GeneralExpConfig,
    IndicatorExpConfig,
    Order,
    OrderRecord,
    OrderStatus,
    OrderType,
    Portfolio,
    PortfolioExpConfig,
    RunResult,
    State,
    StrategyExpConfig,
    Trade,
    _run_experiment,
    experiment_log,
    request_abort,
)
from backtide.core.storage import delete_experiment as _delete_experiment
from backtide.core.storage import query_experiments as _query_experiments
from backtide.utils.utils import _to_list, _to_pandas

# Threading event used to signal an abort from external code (for example, the web UI).
_abort_event: threading.Event | None = None


class ExperimentAborted(KeyboardInterrupt):
    """Raised when an experiment is aborted by the user."""


def _cleanup_experiment(experiment_id: str | None, name: str):
    """Best-effort removal of a (partially) persisted experiment."""
    if experiment_id:
        try:
            _delete_experiment(experiment_id)
        except Exception:  # noqa: BLE001
            pass
        return

    # The Rust core may have persisted the experiment before the interrupt
    # reached Python. Try to find and remove the most recent experiment
    # matching the config name.
    try:
        df = _to_pandas(_query_experiments(search=name, limit=1))
        if not df.empty:
            _delete_experiment(df.iloc[0]["id"])
    except Exception:  # noqa: BLE001
        pass


class Experiment:
    """Configure and run one historical backtest experiment.

    Performs the full pipeline end-to-end:

    1. Resolves and downloads the required market data (skipped if already
       present in the database).
    2. Computes indicators over the entire dataset.
    3. Runs every strategy in parallel. Each strategy has its own independent
       portfolio, order book and equity curve.
    4. Persists the results into the database.

    Read more in the [user guide][experiment].

    Parameters
    ----------
    config : [ExperimentConfig] | None, default=None
        Serializable data, portfolio, execution, engine, and metric settings.
        Uses defaults when omitted.

    strategies : str | object | dict[str, object] | list | None, default=None
        Runtime strategies for this experiment. Accepts stored names, strategy
        instances, explicit `dict[name, instance]` mappings, or a list mixing
        those forms. When omitted, uses the stored names in `config`.

    indicators : str | object | dict[str, object] | list | None, default=None
        Runtime indicators to compute in addition to strategy-required
        indicators. Accepts the same forms as `strategies`. When omitted, uses
        the stored names in `config`.

    Examples
    --------
    ```pycon
    from backtide.backtest import DataExpConfig, Experiment, ExperimentConfig
    from backtide.strategies import BuyAndHold

    config = ExperimentConfig(
        data=DataExpConfig(
            symbols=["AAPL", "MSFT"],
            interval="1d",
        )
    )
    result = Experiment(config, strategies=[BuyAndHold()]).run()
    print(result)
    ```
    """

    def __init__(
        self,
        config: ExperimentConfig | None = None,
        strategies: Any = None,
        indicators: Any = None,
    ) -> None:
        self.config = config or ExperimentConfig()
        self.strategies = strategies
        self.indicators = indicators

    @staticmethod
    def _resolve_runtime_param(values: Any) -> tuple[list[str], dict[str, Any]]:
        """Resolve the list of stored strategies/indicators."""
        elements: list[str] = []
        overrides: dict[str, Any] = {}
        values = [values] if isinstance(values, dict) else _to_list(values)
        for elem in values:
            if isinstance(elem, str):
                elements.append(elem)
            elif isinstance(elem, dict):
                overrides.update(elem)
                elements.extend(elem.keys())
            else:
                name = elem.__class__.__name__
                overrides[name] = elem
                elements.append(name)

        return elements, overrides

    def run(self, *, verbose: bool = True) -> ExperimentResult:
        """Run the configured experiment and return its persisted result."""
        strategy_values = (
            self.config.strategy.strategies if self.strategies is None else self.strategies
        )
        indicator_values = (
            self.config.indicators.indicators if self.indicators is None else self.indicators
        )
        strategies, strategy_overrides = self._resolve_runtime_param(strategy_values)
        indicators, indicator_overrides = self._resolve_runtime_param(indicator_values)
        general = self.config.general
        config = ExperimentConfig(
            general=GeneralExpConfig(
                name=general.name.strip() or str(uuid.uuid4())[:8],
                icon=general.icon,
                tags=general.tags,
                description=general.description,
            ),
            data=self.config.data,
            portfolio=self.config.portfolio,
            strategy=StrategyExpConfig(
                benchmark=self.config.strategy.benchmark,
                strategies=strategies,
            ),
            indicators=IndicatorExpConfig(indicators=indicators),
            metrics=self.config.metrics,
            exchange=self.config.exchange,
            engine=self.config.engine,
        )

        if not config.data.symbols:
            raise ValueError("Experiment configuration has no symbols.")
        if not config.strategy.strategies and not strategy_overrides:
            raise ValueError("Experiment configuration has no strategies.")

        try:
            result = _run_experiment(
                config,
                verbose,
                strategy_overrides,
                indicator_overrides,
            )
        except KeyboardInterrupt:
            _cleanup_experiment(None, config.general.name)
            raise ExperimentAborted("Experiment aborted by user.") from None

        if _abort_event is not None and _abort_event.is_set():
            _cleanup_experiment(result.experiment_id, config.general.name)
            raise ExperimentAborted("Experiment aborted by user.")
        return result
