"""Backtide.

Author: Mavs
Description: Public Python interface for the backtest module.
"""

from __future__ import annotations

from typing import Any

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
    GeneralExpConfig,
    IndicatorExpConfig,
    Order,
    OrderRecord,
    OrderType,
    Portfolio,
    PortfolioExpConfig,
    RunResult,
    State,
    StrategyExpConfig,
    Trade,
)
from backtide.core.backtest import (
    run_experiment as _run_experiment,
)
from backtide.utils.utils import _to_list


def run_experiment(
    config: ExperimentConfig | None = None,
    *,
    verbose: bool = True,
    **kwargs: Any,
) -> ExperimentResult:
    """Run a backtest experiment with the provided configuration.

    Performs the full pipeline end-to-end:

    1. Resolves and downloads the required market data (skipped if already
       present in the database).
    2. Computes indicators over the entire dataset.
    3. Runs every strategy in parallel. Each strategy has its own independent
       portfolio, order book and equity curve.
    4. Persists the [`ExperimentResult`] (and per-strategy artifacts) into the
       database.

    Parameters
    ----------
    config : [ExperimentConfig] | None = None
        The complete experiment configuration. If `None`, the configuration
        must be provided through `kwargs`. Missing parameters are filled with
        the default values.

    verbose : bool, default=True
        Whether to display a progress bar while running.

    **kwargs
        Any combination of:

        * Sub-config objects via keyword (`general`, `data`, `portfolio`,
          `strategy`, `indicators`, `exchange`, `engine`).
        * Flat keyword arguments matching any field of the sub-configs
          (e.g., `name`, `symbols`, `interval`, `initial_cash`).

        The `strategies` and `indicators` keyword arguments additionally accept,
        beyond a list of stored names, any of:

        * A single string (name of a stored strategy / indicator).
        * A [`BaseStrategy`] / [`BaseIndicator`] subclass instance (the class'
          name is used as the display name).
        * A `dict[str, instance]` mapping explicit names to instances.

        Keyword arguments take precedence over the corresponding fields in the
        `config` object.

    Returns
    -------
    [ExperimentResult]
        The aggregated result of the run.

    Examples
    --------
    >>> from backtide.backtest import run_experiment
    >>> from backtide.strategies import BuyAndHold
    >>>
    >>> result = run_experiment(
    ...     name="Apple and Microsoft",
    ...     symbols=["AAPL", "MSFT"],
    ...     interval="1d",
    ...     strategies=[BuyAndHold()],
    ... )
    >>> print(result)
    """

    def resolve_polymorphic_param(values: Any) -> tuple[list[str], dict[str, Any]]:
        """Resolve the list of stored strategies/indicators."""
        elements = []
        overrides = {}
        for elem in _to_list(values):
            if isinstance(elem, str):
                elements.append(elem)
            elif isinstance(elem, dict):
                overrides.update(elem)
            else:
                overrides.update({elem.__class__.__name__: elem})

        return elements, overrides

    kwargs = kwargs.copy()
    cfg = config or ExperimentConfig()

    # Retrieve a config parameter from the arguments
    get = lambda k, s: (
        kwargs.pop(k, getattr(kwargs.get(s), k, None)) or getattr(getattr(cfg, s), k)
    )

    strategies, strategy_overrides = resolve_polymorphic_param(get("strategies", "strategy"))
    indicators, indicator_overrides = resolve_polymorphic_param(get("indicators", "indicators"))

    cfg = ExperimentConfig(
        general=kwargs.pop(
            "general",
            GeneralExpConfig(
                name=get("name", "general"),
                tags=get("tags", "general"),
                description=get("description", "general"),
            ),
        ),
        data=kwargs.pop(
            "data",
            DataExpConfig(
                instrument_type=get("instrument_type", "data"),
                symbols=get("symbols", "data"),
                full_history=get("full_history", "data"),
                start_date=get("start_date", "data"),
                end_date=get("end_date", "data"),
                interval=get("interval", "data"),
            ),
        ),
        portfolio=kwargs.pop(
            "portfolio",
            PortfolioExpConfig(
                initial_cash=get("initial_cash", "portfolio"),
                base_currency=get("base_currency", "portfolio"),
                starting_positions=get("starting_positions", "portfolio"),
            ),
        ),
        strategy=kwargs.pop(
            "strategy",
            StrategyExpConfig(
                benchmark=get("benchmark", "strategy"),
                strategies=strategies,
            ),
        ),
        indicators=kwargs.pop(
            "indicators",
            IndicatorExpConfig(
                indicators=indicators,
            ),
        ),
        exchange=kwargs.pop(
            "exchange",
            ExchangeExpConfig(
                commission_type=get("commission_type", "exchange"),
                commission_pct=get("commission_pct", "exchange"),
                commission_fixed=get("commission_fixed", "exchange"),
                slippage=get("slippage", "exchange"),
                allowed_order_types=get("allowed_order_types", "exchange"),
                partial_fills=get("partial_fills", "exchange"),
                allow_margin=get("allow_margin", "exchange"),
                max_leverage=get("max_leverage", "exchange"),
                initial_margin=get("initial_margin", "exchange"),
                maintenance_margin=get("maintenance_margin", "exchange"),
                margin_interest=get("margin_interest", "exchange"),
                allow_short_selling=get("allow_short_selling", "exchange"),
                borrow_rate=get("borrow_rate", "exchange"),
                max_position_size=get("max_position_size", "exchange"),
                conversion_mode=get("conversion_mode", "exchange"),
                conversion_threshold=get("conversion_threshold", "exchange"),
                conversion_period=get("conversion_period", "exchange"),
                conversion_interval=get("conversion_interval", "exchange"),
            ),
        ),
        engine=kwargs.pop(
            "engine",
            EngineExpConfig(
                warmup_period=get("warmup_period", "engine"),
                trade_on_close=get("trade_on_close", "engine"),
                risk_free_rate=get("risk_free_rate", "engine"),
                exclusive_orders=get("exclusive_orders", "engine"),
                random_seed=get("random_seed", "engine"),
                empty_bar_policy=get("empty_bar_policy", "engine"),
            ),
        ),
    )

    if kwargs:
        raise ValueError(f"Unknown keyword arguments: {', '.join(kwargs)}")

    if not cfg.data.symbols:
        raise ValueError("Experiment configuration has no symbols.")

    if not cfg.strategy.strategies and not strategy_overrides:
        raise ValueError("Experiment configuration has no strategies.")

    return _run_experiment(cfg, verbose, strategy_overrides, indicator_overrides)
