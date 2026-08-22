# Run and inspect an experiment

This example runs two built-in strategies over the same historical data, compares their summary
metrics, inspects the best run's records, and plots normalized profit and loss. Missing market data
is downloaded automatically, and the completed experiment is persisted to local storage.

```python
import pandas as pd

from backtide.analysis import plot_pnl
from backtide.backtest import (
    DataExpConfig,
    Experiment,
    ExperimentConfig,
    GeneralExpConfig,
    StrategyExpConfig,
)
from backtide.strategies import BuyAndHold, SmaNaive

config = ExperimentConfig(
    general=GeneralExpConfig(name="Compare Apple strategies"),
    data=DataExpConfig(
        symbols=["AAPL"],
        interval="1d",
        start_date="2022-01-01",
        end_date="2024-12-31",
        full_history=False,
    ),
    strategy=StrategyExpConfig(benchmark="SPY"),
    metrics=["total_return", "sharpe", "pnl", "max_dd"],
)
result = Experiment(
    config,
    strategies=[BuyAndHold(), SmaNaive()],
).run()

print(f"Experiment {result.experiment_id}: {result.status}")
for warning in result.warnings:
    print(f"Warning: {warning}")

successful = [run for run in result.strategies if run.error is None]
if not successful:
    errors = [run.error for run in result.strategies]
    raise RuntimeError(f"Every strategy failed: {errors}")

summary = pd.DataFrame(
    {
        "strategy": run.strategy_name,
        "benchmark": run.is_benchmark,
        "total_return": run.metrics.get("total_return"),
        "sharpe": run.metrics.get("sharpe"),
        "pnl": run.metrics.get("pnl"),
        "max_dd": run.metrics.get("max_dd"),
        "trades": len(run.trades),
        "orders": len(run.orders),
    }
    for run in successful
).sort_values("sharpe", ascending=False)
summary.style.format(precision=2)

strategy_runs = [run for run in successful if not run.is_benchmark]
best = max(strategy_runs, key=lambda run: run.metrics.get("sharpe", float("-inf")))
print(
    f"Inspecting {best.strategy_name}: "
    f"{len(best.equity_curve)} equity samples and {len(best.trades)} closed trades"
)

trades = pd.DataFrame(
    {
        "symbol": trade.symbol,
        "quantity": trade.quantity,
        "entry": trade.entry_ts,
        "exit": trade.exit_ts,
        "pnl": trade.pnl,
    }
    for trade in best.trades
)
trades.head()

plot_pnl(successful, normalize=True, title="Strategy comparison")
```

Always inspect `result.status`, `result.warnings`, and each `run.error` before ranking runs. The
returned [`RunResult`] objects also expose the complete `equity_curve`, `trades`, and `orders`
collections. The same experiment can be reopened later with
[`query_experiments`] and [`query_strategy_runs`].
