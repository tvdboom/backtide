# Metrics

Metrics reduce a completed strategy run to one finite scalar. Backtide ships a built-in catalog
and lets you save custom Python metrics alongside custom strategies and indicators.

## Selecting metrics

Use the **Metrics** step of the experiment builder to choose the metrics computed for every
strategy, then drag them into the desired order. The first selected metric ranks the strategy runs
and appears by name on the results overview. Sharpe ratio is first by default. The Metrics tab for
each run shows every selected value in the same order.

The catalog includes returns, PnL, CAGR, volatility, Sharpe, Sortino,
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
    metrics=[GainToPain(), "total_return", "sharpe"],
)
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
