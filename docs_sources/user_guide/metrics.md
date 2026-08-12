# Metrics

Metrics reduce a completed strategy run to one finite scalar. Backtide ships a catalog of
Rust metrics and lets you save custom Python metrics alongside custom strategies and indicators.

## Selecting metrics

Use the **Metrics** step of the experiment builder to choose the metrics computed for every
strategy. Sharpe ratio is the default main metric. The main metric ranks the strategy runs and
appears by name on the results overview. The Metrics tab for each run
shows every selected value.

Built-in metrics execute in the Rust engine and are distributed across the same Rayon worker
pool as strategy results. The catalog includes returns, PnL, CAGR, volatility, Sharpe, Sortino,
maximum drawdown, Calmar, trade counts, win rate, profit factor, expectancy, average and extreme
trades, payoff ratio, recovery factor, excess return, and benchmark alpha.

## Custom Python metrics

Subclass [`BaseMetric`] and implement `compute(self, equity_curve, trades)`. Backtide calls the
method once after each strategy finishes. Both inputs use the configured dataframe library and
are new result tables, so changing them cannot mutate the stored run.

```python
from backtide.metrics import BaseMetric


class GainToPain(BaseMetric):
    """Return gross winning PnL divided by gross losing PnL."""

    percentage = False
    higher_is_better = True

    def compute(self, equity_curve, trades):
        pnl = trades["pnl"]
        gains = pnl[pnl > 0].sum()
        losses = abs(pnl[pnl < 0].sum())
        return float(gains / losses) if losses else 0.0


GainToPain()
```

The class docstring is used as the metric description in the library and experiment builder;
there is no separate `description` attribute. The last expression must instantiate the metric,
and the returned value must convert to a finite `float`. Set `percentage = True` when the returned
value is a fraction such as `0.12`, and set `higher_is_better = False` for metrics where the
smallest value should be considered best.

Saved metrics can be selected in the experiment builder or passed directly to
[`run_experiment`]:

```python
result = run_experiment(
    symbols=["AAPL"],
    strategies=[strategy],
    metrics=["total_return", "sharpe", GainToPain()],
    main_metric="GainToPain",
)
```
