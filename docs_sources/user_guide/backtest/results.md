# Results

[`run_experiment`] returns an [`ExperimentResult`]. It contains experiment-level status and
warnings plus one [`RunResult`] for each strategy (and the benchmark, when configured).

```python
from backtide.backtest import run_experiment
from backtide.strategies import BuyAndHold

result = run_experiment(
    name="Inspect Apple results",
    symbols=["AAPL"],
    strategies=[BuyAndHold()],
)

print(result.status, result.warnings)
for run in result.strategies:
    print(run.strategy_name, run.metrics.get("total_return"), run.error)
```

## Extract the useful parts

Each strategy result exposes four main collections:

- `metrics` is a `dict[str, float]` for quick ranking and reporting.
- `equity_curve` contains chronological [`EquitySample`] objects for equity and drawdown analysis.
- `trades` contains closed round trips, including entry, exit, quantity, and PnL.
- `orders` contains every processed order, including fills, cancellations, and rejections.

Use ordinary Python to select the run you need and convert records to tabular data:

```python
import pandas as pd

successful = [run for run in result.strategies if run.error is None and not run.is_benchmark]
best = max(successful, key=lambda run: run.metrics.get("sharpe", float("-inf")))

metric_row = {"strategy": best.strategy_name, **best.metrics}
trades = pd.DataFrame(
    {
        "symbol": trade.symbol,
        "quantity": trade.quantity,
        "entry_ts": trade.entry_ts,
        "exit_ts": trade.exit_ts,
        "pnl": trade.pnl,
    }
    for trade in best.trades
)
equity = pd.DataFrame(
    {
        "timestamp": sample.timestamp,
        "equity": sample.equity,
        "drawdown": sample.drawdown,
    }
    for sample in best.equity_curve
)
```

Check `result.status`, `result.warnings`, and every `run.error` before comparing metrics. A partial
experiment may still contain valid strategy results, but failed runs should not silently enter a
ranking. Use [Plots](plots.md) when the sequence and shape of results matters more than a scalar.
