# Metrics

Metrics reduce a completed strategy run to one finite scalar. Backtide ships a built-in catalog
and lets you save custom Python metrics alongside custom strategies and indicators.

## Selecting metrics

Use the **Metrics** step of the experiment builder to choose the metrics computed for every
strategy, then drag them into the desired order. The first selected metric ranks the strategy runs
and appears by name on the results overview. Sharpe ratio is first by default. The Metrics tab for
each run shows every selected value in the same order.

Pass a built-in metric by its exact key, not its display name. For example, use
`"ann_volatility"` rather than `"Annualized volatility"`. The complete built-in catalog is:

| String key | Display name | Meaning |
|------------|--------------|---------|
| `total_return` | Total return | Net portfolio return over the experiment. |
| `pnl` | Profit and loss | Final equity minus initial cash. |
| `final_equity` | Final equity | Portfolio value at the final sample. |
| `cagr` | CAGR | Compound annual growth rate. |
| `ann_volatility` | Annualized volatility | Annualized standard deviation of returns. |
| `sharpe` | Sharpe ratio | Annualized excess return per unit of volatility. |
| `sortino` | Sortino ratio | Annualized excess return per unit of downside deviation. |
| `max_dd` | Maximum drawdown | Largest fractional fall from a running equity peak. |
| `calmar` | Calmar ratio | CAGR divided by absolute maximum drawdown. |
| `n_trades` | Trades | Number of completed round-trip trades. |
| `win_rate` | Win rate | Fraction of completed trades with positive PnL. |
| `profit_factor` | Profit factor | Gross winning PnL divided by gross losing PnL. |
| `expectancy` | Expectancy | Average PnL per completed trade. |
| `avg_win` | Average win | Average PnL of profitable trades. |
| `avg_loss` | Average loss | Average PnL of losing trades. |
| `best_trade` | Best trade | Largest completed-trade PnL. |
| `worst_trade` | Worst trade | Smallest completed-trade PnL. |
| `payoff_ratio` | Payoff ratio | Average win divided by absolute average loss. |
| `recovery_factor` | Recovery factor | Net PnL divided by absolute maximum drawdown amount. |
| `excess_return` | Excess return | Return above the compounded risk-free rate. |
| `alpha` | Alpha | Return above the selected benchmark over the shared window. |

`ExperimentConfig()` selects `sharpe`, `total_return`, `pnl`, `max_dd`, `cagr`,
`n_trades`, `win_rate`, `sortino`, `ann_volatility`, `final_equity`, `excess_return`,
and `alpha` by default. The remaining built-ins are opt-in.

You can also inspect the catalog programmatically. `MetricDefinition.key` is the exact value
accepted in the `metrics` list on [`ExperimentConfig`] and [`SessionConfig`]:

```python
from backtide.metrics import list_builtin_metrics

for metric in list_builtin_metrics():
    print(metric.key, metric.name)
```

## Custom Python metrics

Subclass [`BaseMetric`] and implement `compute(self, equity_curve, trades)`. Backtide calls the
method once after each strategy finishes. Both inputs use the configured dataframe library and
are new result tables, so changing them cannot mutate the stored run.

```python
from backtide.metrics import BaseMetric


class GainToPain(BaseMetric):
    """Return gross winning PnL divided by gross losing PnL."""

    percentage = False
    greater_is_better = True

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
value is a fraction such as `0.12`. Backtide already knows whether larger or smaller values are
preferred for built-in metrics. For a custom metric, set `greater_is_better = False` when the
smallest value should be considered best; it defaults to `True` when omitted.

Built-in keys and custom Python objects belong in the same config list. This is the only place
metrics are specified: there is no separate metrics parameter on [`Experiment`] or [`Session`].
Use a `dict[name, instance]` entry when you want an explicit persisted name; otherwise the custom
class name is used. Serialization stores the resolved names while an in-memory run retains the
Python implementations:

```python
from backtide import DataExpConfig, Experiment, ExperimentConfig

config = ExperimentConfig(
    data=DataExpConfig(symbols=["AAPL"]),
    metrics=["total_return", "sharpe", GainToPain()],
)
result = Experiment(config, strategies=[strategy]).run()
```

The live interface uses the same selection shape:

```python
from backtide.live import Session, SessionConfig

session = Session(SessionConfig(metrics=["pnl", {"gain_to_pain": GainToPain()}]))
```

See the [Gain to pain](../../examples/metrics/gain_to_pain.md) and
[Ulcer index](../../examples/metrics/ulcer_index.md) pages for complete custom metrics. For expensive
array calculations, read the [Performance guide](performance.md#metric-performance).

## Formulas

Every strategy run carries a `metrics` dictionary of named scalars. Return-flavored metrics are
stored as fractions, so `0.12` means 12%.

### Final equity, PnL, and total return

$$
\text{final_equity} = \text{equity_curve}[-1]
$$

$$
\text{pnl} = \text{final_equity} - \text{initial_cash}
$$

$$
\text{total_return} = \frac{\text{final_equity} - \text{initial_cash}}{\text{initial_cash}}
$$

Final equity combines cash and marked-to-market positions in the portfolio base currency.

### Trade count and win rate

$$
\text{n_trades} = |\text{trades}|
\qquad
\text{win_rate} = \frac{|\{t \in \text{trades} : t.\text{pnl} > 0\}|}{\text{n_trades}}
$$

Only closed round trips count as trades. A trade wins only when its PnL is strictly positive.

### CAGR and annualized volatility

For bar-to-bar returns $r_i = V_{i+1}/V_i - 1$, the annualization factor is derived from the
equity-curve density. The compound annual growth rate and volatility are:

$$
\text{cagr} = \left(\frac{V_n}{V_0}\right)^{1/n_\text{years}} - 1
$$

$$
\text{ann_volatility} = \sigma(r) \sqrt{\text{ann}}
$$

### Sharpe and Sortino

$$
\text{sharpe} = \frac{\bar{r} - r_f / \text{ann}}{\sigma(r)} \sqrt{\text{ann}}
$$

$$
\text{sortino} = \frac{\bar{r} - r_f / \text{ann}}{\sigma(r_-)} \sqrt{\text{ann}},
\qquad r_- = \{r_i : r_i < 0\}
$$

Sharpe uses all return variation; Sortino uses only negative returns for downside deviation. Both
return zero when the relevant deviation is zero.

### Maximum drawdown

For cumulative returns $C_i = \prod_{k \le i}(1+r_k)$:

$$
\text{max_dd} = \min_i \frac{C_i - \max_{k \le i} C_k}{\max_{k \le i} C_k}
$$

Maximum drawdown is zero or negative. A value of `-0.25` means equity was 25% below its prior peak.

### Alpha and excess return

Alpha compares the strategy and benchmark only over their overlapping window:

$$
R(c) = \frac{c[-1] - c[\text{window_start}]}{c[\text{window_start}]}
\qquad
\text{alpha} = R(\text{strategy}) - R(\text{benchmark})
$$

Excess return compares the strategy return with compounded risk-free return over the same period:

$$
\text{excess_return} = R(\text{strategy}) - ((1+r_f)^{n_\text{years}}-1)
$$
