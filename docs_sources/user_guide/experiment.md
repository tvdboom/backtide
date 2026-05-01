# Experiment
------------

An **experiment** is a single end-to-end backtest run. It binds together a set
of [strategies], a universe of [symbols][data], a portfolio definition, a
benchmark, an exchange model and an engine configuration into one reproducible
unit. Every experiment is persisted to [storage] so it can be reopened, compared
and re-analysed long after the original run finished.

You configure an experiment in the **Experiment** tab of the [application][application],
or programmatically via [`ExperimentConfig`] and [`run_experiment`].

<br>

## Lifecycle

When [`run_experiment`] is invoked, the engine runs the following phases in
order. Failures in any phase emit warnings (visible on the results page) but
do not stop the run unless they leave it without a single tradeable bar.

1. **Resolve** instrument profiles for every symbol and the configured benchmark.
2. **Download** any missing OHLCV bars on the chosen interval, clamped to
   `start_date` / `end_date` if set.
3. **Load** bars from storage and align them onto a master timeline (the union
   of all symbol timestamps, sorted). Empty bars are filled according to
   [`EmptyBarPolicy`].
4. **Compute indicators** once over the full dataset, in parallel across
   `(symbol, indicator)`. Strategies declare their required indicators via
   `required_indicators()` and the engine deduplicates them automatically — see
   [auto-injected indicators][auto-injected].
5. **Run strategies in parallel** (rayon for built-in Rust strategies, sequential
   under the GIL for custom Python strategies). Each strategy gets its own
   portfolio, order book, equity log and trade log.
6. **Persist** the aggregate result and per-strategy artefacts to DuckDB and
   return them to the caller.

[auto-injected]: strategies.md#auto-injected-indicators

<br>

## Configuration sections

[`ExperimentConfig`] is a thin wrapper around seven typed sub-configurations.
They map one-to-one to the tabs in the application's experiment page.

| Section | What it controls |
|---|---|
| [`GeneralExpConfig`] | Name, tags, free-text description. |
| [`DataExpConfig`] | Instrument type, symbols, date range, interval. |
| [`PortfolioExpConfig`] | Initial cash, base currency, starting positions. |
| [`StrategyExpConfig`] | Selected strategies and the benchmark symbol. |
| [`IndicatorExpConfig`] | Extra indicators to compute on top of the auto-injected ones. |
| [`ExchangeExpConfig`] | Commission, slippage, allowed order types, margin, short selling, currency conversion. |
| [`EngineExpConfig`] | Warmup period, trade-on-close, risk-free rate, exclusive orders, RNG seed, empty-bar policy. |

The full TOML representation is what's stored next to every experiment under
`<storage_path>/experiments/<id>/config.toml`. You can re-create a config from
disk with `ExperimentConfig.from_toml(...)`.

<br>

## Benchmark

If [`StrategyExpConfig.benchmark`][`StrategyExpConfig`] is non-empty, the engine:

- Folds the benchmark symbol into the data download list so its bars are
  available for every strategy.
- Auto-injects an extra strategy run named `Benchmark (<SYMBOL>)` that holds a
  pure passive `BuyAndHold(symbol=<SYMBOL>)` over the same window. This is the
  series used to compute [alpha](#alpha).
- **Does not** let the benchmark symbol leak into other strategies. Only the
  symbols you explicitly added in the data tab are visible to user strategies;
  if you want a strategy to also trade the benchmark, add it to the symbol list.

<br>

## Results

Each experiment produces an [`ExperimentResult`] containing one
[`StrategyRunResult`] per evaluated strategy.

| Component | Description |
|---|---|
| `equity_curve` | One [`EquitySample`] per simulated bar (timestamp, equity, cash, drawdown). |
| `orders` | One [`OrderRecord`] per processed order (filled, cancelled or rejected) — see the full [order anatomy](#orders). |
| `trades` | One [`Trade`] per closed round-trip (open + matching close, FIFO). |
| `metrics` | The named scalar metrics described [below](#metrics). |
| `error` | First fatal exception raised by the strategy, if any. |


### Visualising results

The experiment page surfaces an interactive **PnL-over-time** chart between
the high-level metrics and the per-strategy breakdown. It plots one line per
strategy on a shared time axis so the user can compare the cumulative
performance of every strategy at a glance — the auto-injected
`Benchmark (<SYMBOL>)` run is drawn as a dashed line for easy distinction.

A *Relative* toggle in the chart's options switches between absolute PnL
(the default) and a percentage return relative to each strategy's starting
equity. The relative view is most useful when strategies start with very
different capital allocations.

The same chart is exposed programmatically as [`plot_pnl`][plot_pnl] for
custom notebooks and reports:

```python
from backtide.analysis import plot_pnl
from backtide.storage import query_strategy_runs, query_experiments

exp = query_experiments()[0]
runs = query_strategy_runs(exp.id)
plot_pnl(runs, normalize=True)
```

[plot_pnl]: ../api/analysis/plot_pnl.md

### Orders

Every fill, cancellation and rejection produces an [`OrderRecord`]. The relevant
fields for inspection are:

| Field | Meaning |
|---|---|
| `timestamp` | Bar timestamp at which the order was processed. |
| `order.symbol` | Traded instrument. |
| `order.quantity` | Signed quantity. **Buy** when positive, **sell** when negative. |
| `fill_price` | Slippage-adjusted average fill price. `None` for non-fills. |
| `status` | `"filled"`, `"cancelled"`, `"rejected"` or `"pending"`. |
| `reason` | Human-readable note (e.g., `"insufficient funds"`, `"exclusive_orders"`). |
| `commission` | Commission charged on this fill, in the order's quote currency. Zero on non-fills. |
| `pnl` | Realised PnL on **closing** fills only (sells that flatten or reduce a long), in the base currency, after this leg's commission. `None` for opening fills. |

### Trades

A [`Trade`] is a closed round-trip: an opening fill paired with the matching
closing fill (FIFO). One sell that closes a 100-share long entered in two
separate buys becomes two `Trade` rows — one per cost-basis lot consumed.

| Field | Meaning |
|---|---|
| `entry_ts` / `entry_price` | Open leg's timestamp and quantity-weighted average fill price. |
| `exit_ts` / `exit_price` | Close leg's timestamp and slippage-adjusted fill price. |
| `quantity` | Round-trip size (always positive). |
| `pnl` | $(\text{exit\_price} - \text{entry\_price}) \cdot \text{quantity} - \text{closing\_commission}$, in the base currency. |

!!! note
    Only the **closing** leg's commission is subtracted from `Trade.pnl`. The
    opening leg's commission is paid out of cash but does not appear in the
    per-trade PnL — it shows up only in the equity curve and the headline
    `pnl` metric.

<br>

## Metrics

Every strategy run carries a `metrics` dict of named scalars. They are
computed in [`compute_metrics`][compute_metrics] from the equity curve and the
trade log, plus an extra alignment pass for `alpha` and `excess_return`. All
return-flavoured metrics are stored as **fractions** (e.g., `0.12` = 12 %); the
UI multiplies by 100 for display.

[compute_metrics]: https://github.com/tvdboom/backtide/blob/master/src/backtide_core/src/backtest/engine.rs

### Final equity & PnL

$$
\text{final\_equity} = \text{equity\_curve}[-1]
$$

$$
\text{pnl} = \text{final\_equity} - \text{initial\_cash}
$$

$$
\text{total\_return} = \frac{\text{final\_equity} - \text{initial\_cash}}{\text{initial\_cash}}
$$

`final_equity` is the last value of the equity curve, which itself is the sum
of cash (across all currencies, treated 1:1) plus the mark-to-market value of
every open position at every bar's close.

### `n_trades` & `win_rate`

$$
\text{n\_trades} = |\text{trades}|
\qquad
\text{win\_rate} = \frac{|\{t \in \text{trades} : t.\text{pnl} > 0\}|}{\text{n\_trades}}
$$

A trade is **winning** iff its `pnl` is **strictly positive**. Break-even
trades (`pnl == 0`) and losing trades (`pnl < 0`) are not counted as wins.

Only **closed round-trips** count toward `n_trades`. Positions still open at
the very last bar are auto-liquidated by the engine and the resulting closes
do flow into the trade list, so they are included as well.

### `cagr` & `ann_volatility`

These come from [`compute_series_stats`][analysis.rs] applied to the equity
curve, so the analysis page and the backtest engine produce identical numbers.

The annualisation factor `ann` is derived from the equity-curve density:

$$
\text{ann} = \mathrm{round}\!\left(\frac{n_\text{returns}}{\Delta t / (365.25 \cdot 86400)}\right)
$$

where $\Delta t$ is the time span of the equity curve in seconds. For daily
bars this lands very close to 252.

Bar-to-bar simple returns are

$$
r_i = \frac{V_{i+1}}{V_i} - 1
$$

with $V_i$ the equity at bar $i$. The compound annual growth rate is

$$
\text{cagr} = \left(\frac{V_n}{V_0}\right)^{1/n_\text{years}} - 1,
\qquad
n_\text{years} = \frac{n_\text{returns}}{\text{ann}}
$$

falling back to a simple total return if the period is too short for CAGR to
be numerically stable.

Annualised volatility is the standard deviation of bar-to-bar returns scaled
by $\sqrt{\text{ann}}$:

$$
\text{ann\_volatility} = \sigma(r) \cdot \sqrt{\text{ann}}
$$

[analysis.rs]: https://github.com/tvdboom/backtide/blob/master/src/backtide_core/src/analysis.rs

### `sharpe`

The classic risk-adjusted return:

$$
\text{sharpe} = \frac{\bar{r} - r_f / \text{ann}}{\sigma(r)} \cdot \sqrt{\text{ann}}
$$

where $r_f$ is the annualised `risk_free_rate` (a fraction). The numerator is
the per-period excess return; the denominator is the per-period return
standard deviation. Multiplying by $\sqrt{\text{ann}}$ converts the ratio to
its annualised form. Returns 0 if returns have zero variance.

### `sortino`

Like Sharpe but punishes only **downside** deviation:

$$
\text{sortino} = \frac{\bar{r} - r_f / \text{ann}}{\sigma(r_-)} \cdot \sqrt{\text{ann}},
\qquad r_- = \{r_i : r_i < 0\}
$$

Useful when you don't want upside volatility to be penalised the same way as
downside volatility.

### `max_dd`

Maximum drawdown is the largest fractional drop from a running peak on the
**cumulative-return path** $C_i = \prod_{k \le i} (1 + r_k)$:

$$
\text{max\_dd} = \min_i \frac{C_i - \max_{k \le i} C_k}{\max_{k \le i} C_k}
$$

It is always $\le 0$. A reading of `-0.25` means the equity curve, at its
worst, was 25 % below its all-time high.

### `alpha`

Alpha is the **windowed total-return difference** between the strategy and the
benchmark, computed only when a benchmark is configured. The window is aligned
to the *later* of the two equity-curve start dates so that strategies with
deeper history aren't penalised by missing benchmark data:

$$
\text{window\_start} = \max(\text{strat\_start}, \text{bench\_start})
$$

$$
R(c) = \frac{c[-1] - c[\text{window\_start}]}{c[\text{window\_start}]}
$$

$$
\text{alpha} = R(\text{strategy}) - R(\text{benchmark})
$$

Positive alpha means the strategy out-performed buy-and-hold of the benchmark
over the overlapping period. Alpha is **not** computed on the benchmark run
itself.

### `excess_return`

Same idea as alpha but against the **risk-free rate** instead of the benchmark:

$$
\text{excess\_return} = R(\text{strategy}) - \big((1 + r_f)^{n_\text{years}} - 1\big)
$$

with $n_\text{years} = (\text{strat\_end} - \text{window\_start}) / (365.25 \cdot 86400)$
and $r_f$ the configured `risk_free_rate / 100`.

<br>

## Reproducibility

Set [`EngineExpConfig.random_seed`][`EngineExpConfig`] to a fixed integer to
make stochastic strategies and any RNG-dependent fills deterministic. With a
seed set, two runs of the same `ExperimentConfig` against the same stored bars
produce identical equity curves, orders and trades. Saving the
`config.toml` is enough to reproduce the run later — the bars themselves are
already on disk via [storage].
