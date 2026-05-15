# Experiment
------------

An **experiment** is a single end-to-end backtest run. It binds together a set
of [strategies], a universe of [symbols][data], a portfolio definition, a
benchmark, an exchange model and an engine configuration into one reproducible
unit. Every experiment is persisted to [storage] so it can be reopened, compared
and re-analysed long after the original run finished.

You configure an experiment in the **Experiment** tab of the [application][application],
or programmatically via [`ExperimentConfig`] and [`run_experiment`].

[`run_experiment`] also accepts every field of every sub-config as a flat
keyword argument, so a one-liner is often enough for ad-hoc runs:

```python
from backtide.backtest import run_experiment
from backtide.indicators import SimpleMovingAverage
from backtide.strategies import BuyAndHold

result = run_experiment(
    name="Apple buy-and-hold",
    symbols=["AAPL"],
    interval="1d",
    start_date="2020-01-01",
    end_date="2024-12-31",
    full_history=False,
    strategies=[BuyAndHold()],
    indicators=[SimpleMovingAverage(20)],
)
```

Strategies and indicators can be passed as a stored name, an instance (the
class name is used as display name), a `dict[name, instance]`, or any list
mixing those forms. Instances are used directly and **not** persisted to disk.

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
4. **Compute indicators** once over the full dataset in parallel.
5. **Run strategies** (in parallel for built-in strategies, sequential for custom
   Python strategies). Each strategy gets its own portfolio, order book, equity log
   and trade log.
6. **Persist** the aggregate result and per-strategy artifacts to the database and
   return them to the caller.

<br>

## Configuration sections

[`ExperimentConfig`] is a thin wrapper around seven typed sub-configurations.
They map one-to-one to the tabs in the application's experiment page.

| Section                | What it controls                                                                             |
|------------------------|----------------------------------------------------------------------------------------------|
| [`GeneralExpConfig`]   | Name, tags, free-text description.                                                           |
| [`DataExpConfig`]      | Instrument type, symbols, date range, interval.                                              |
| [`PortfolioExpConfig`] | Initial cash, base currency, starting positions.                                             |
| [`StrategyExpConfig`]  | Selected strategies and the benchmark symbol.                                                |
| [`IndicatorExpConfig`] | Extra indicators to compute on top of the auto-injected ones.                                |
| [`ExchangeExpConfig`]  | Commission, slippage, allowed order types, margin, short selling, currency conversion.       |
| [`EngineExpConfig`]    | Warmup period, trade-on-close, risk-free rate, exclusive orders, RNG seed, empty-bar policy. |

The full TOML representation is what's stored next to every experiment under
`<storage_path>/experiments/<id>/config.toml`. You can re-create a config from
disk with `ExperimentConfig.from_toml(...)`.

<br>

## Benchmark

If [`StrategyExpConfig.benchmark`][StrategyExpConfig] is non-empty, the engine:

- Folds the benchmark symbol into the data download list so its bars are
  available for every strategy.
- Auto-injects an extra strategy run named `Benchmark` that holds a pure passive
  `BuyAndHold(<SYMBOL>)` over the same window. This is the series used to compute
  [alpha](#alpha). The benchmark run ignores starting positions.
- **Does not** let the benchmark symbol leak into other strategies. Only the
  symbols you explicitly added in the data tab are visible to user strategies;
  if you want a strategy to also trade the benchmark, add it to the symbol list.

<br>

## Margin trading

Margin trading lets you borrow funds from the broker to open positions larger than
your cash balance. In other words, you can control more shares (or contracts) than
you could afford outright, amplifying both gains and losses.

### How it works in Backtide

Margin is controlled through [`ExchangeExpConfig`]. By default, it is **disabled**
(`allow_margin=False`), meaning you can only buy what your available cash covers.

| Parameter               | Default | Description                                                                                                                                                                                                                   |
|-------------------------|---------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `allow_margin`          | `False` | Master switch. Set to `True` to enable margin.                                                                                                                                                                                |
| `max_leverage`          | `2.0`   | Maximum ratio of total exposure to equity. A value of `2.0` means you can control up to twice your equity.                                                                                                                    |
| `initial_margin`        | `50.0`  | Percentage of the order's notional value that must be covered by equity at the time the order is placed. At `50 %` and a \$10 000 purchase, you need at least \$5 000 in equity.                                              |
| `maintenance_margin`    | `25.0`  | Minimum equity percentage that must be maintained at all times. If equity drops below this threshold, the engine issues a **margin call**.                                                                                    |
| `margin_interest`       | `0.0`   | Annualised interest rate on borrowed funds. Accrued daily and deducted from the portfolio's cash balance.                                                                                                                     |
| `raise_on_margin_limit` | `False` | When `True`, the engine raises an error if an order would breach `max_leverage` or if equity falls below `maintenance_margin`. When `False`, orders are auto-shrunk or rejected with a warning instead. |

```python
from backtide.backtest import run_experiment
from backtide.strategies import SmaCrossover

result = run_experiment(
    name="SMA crossover with 2x margin",
    symbols=["AAPL"],
    interval="1d",
    strategies=[SmaCrossover()],
    allow_margin=True,
    max_leverage=3.0,
    initial_margin=50.0,
    maintenance_margin=25.0,
    margin_interest=8.0,
)
```

### What to consider

!!! warning "Amplified losses"
    Margin amplifies losses just as much as gains. A 2x leveraged position that
    drops 25% wipes out 50% of your equity — and at higher leverage the numbers
    escalate fast.

- **Margin calls.** When your equity falls below the `maintenance_margin`
  percentage the engine triggers a margin call. With `raise_on_margin_limit=False`
  (the default), the position is reduced automatically and a warning is logged.
  With `raise_on_margin_limit=True`, the run aborts so you can investigate.
- **Interest costs.** Borrowed money is not free. The `margin_interest` rate
  is charged annually but accrued daily, eroding your returns even on flat days.
  Make sure your strategy's expected return exceeds the borrowing cost.
- **Max leverage.** Start with a low `max_leverage` (e.g., 1.5–2.0) and
  increase only after verifying that drawdowns remain tolerable. In live
  trading, most retail brokers enforce similar limits.
- **Backtesting bias.** Margin strategies that look great in a backtest can
  blow up in practice because backtests don't capture extreme events like flash
  crashes, exchange halts, or liquidity gaps that prevent timely liquidation.

<br>

## Short selling

**Short selling** (or *shorting*) means selling a security you do not own,
with the intention of buying it back later at a lower price. You profit when
the price falls and lose when it rises. Short selling is essential for
strategies that need to express bearish views or hedge long exposure.

### How it works in Backtide

Short selling is controlled by two fields in [`ExchangeExpConfig`]:

| Parameter                  | Default | Description                                                                                                                                                                                                                          |
|----------------------------|---------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `allow_short_selling`      | `False` | Master switch. When `False`, any sell order for a symbol you do not hold is rejected.                                                                                                                                                |
| `borrow_rate`              | `0.0`   | Annualised cost of borrowing shares for a short position. Accrued daily and deducted from cash, simulating the stock-loan fee a real broker would charge.                                                                            |
| `raise_on_short_violation` | `False` | When `True`, the engine raises an error and aborts the run if a sell order would create or increase a short position while `allow_short_selling` is `False`. When `False`, such orders are silently rejected with a warning instead. |

When enabled, a strategy can place a sell order with a negative quantity for a
symbol it does not currently hold. The engine:

1. Credits the portfolio cash with the proceeds of the sale (price x quantity).
2. Records a negative position in the symbol.
3. Accrues the `borrow_rate` daily against the notional value of the short.
4. Closes the short when the strategy places an equal-and-opposite buy order,
   or when the engine auto-liquidates all positions at the end of the
   simulation.

```python
from backtide.backtest import run_experiment, Order
from backtide.indicators import RelativeStrengthIndex
from backtide.strategies import BaseStrategy


class ShortOnRsiExtreme(BaseStrategy):
    """Short when RSI > 80, cover when RSI < 50."""

    def required_indicators(self):
        return [RelativeStrengthIndex(14)]

    def evaluate(self, data, portfolio, state, indicators):
        orders = []
        for symbol, df in data.items():
            rsi = indicators[symbol]["rsi_14"]
            if rsi is None or len(rsi) < 1:
                continue

            rsi = rsi.iloc[-1]
            qty = portfolio.positions.get(symbol, 0)

            if rsi > 80 and qty >= 0:
                orders.append(Order(symbol=symbol, order_type="market", quantity=-100))
            elif rsi < 50 and qty < 0:
                orders.append(Order(symbol=symbol, order_type="market", quantity=-qty))

        return orders


result = run_experiment(
    name="Short on extreme RSI",
    symbols=["AAPL"],
    interval="1d",
    strategies=[ShortOnRsiExtreme()],
    allow_short_selling=True,
    borrow_rate=3.5,
)
```

### What to consider

!!! warning "Unlimited downside"
    When you buy a stock, the most you can lose is your investment (the price
    drops to zero). When you short a stock, your potential loss is theoretically
    unlimited since the price can rise forever.

- **Borrow costs.** The `borrow_rate` is a steady drag on returns. Hard-to-borrow
  stocks can have annualized rates well above 10%, which can eat into or erase a
  modest short profit.
- **Short squeezes.** A rapid price spike forces short sellers to cover at
  sharply higher prices. Backtesting cannot fully capture the liquidity dynamics
  of a squeeze, so treat squeeze-prone environments with extra caution.
- **Dividends.** In real markets, short sellers are responsible for paying
  dividends to the share lender. Keep this in mind when backtesting short
  strategies on high-dividend stocks.
- **Margin interaction.** Short selling and margin trading are often used
  together. When both are enabled, the engine applies `initial_margin`,
  `maintenance_margin` and `max_leverage` checks to the combined long and short
  exposure. Make sure the margin parameters are realistic for your broker.
- **Catching accidental shorts.** Set `raise_on_short_violation=True` when
  developing a long-only strategy. The engine will abort on the first order
  that would accidentally go short, making bugs easy to spot. In production
  backtests you can leave it `False` so rejected orders are simply logged.

<br>

## Results

Each experiment produces an [`ExperimentResult`] containing one [`RunResult`]
per evaluated strategy. Every fill, cancellation and rejection produces an
[`OrderRecord`]. The stored [trades][trade] is a closed round-trip: an opening
fill paired with the matching closing fill (FIFO). One sell that closes a
100-share long entered in two separate buys becomes two rows — one per
cost-basis lot consumed.

!!! note
    Only the closing leg's commission is subtracted from `Trade.pnl`. The opening
    leg's commission is paid out of cash but does not appear in the per-trade PnL,
    it shows up only in the equity curve and the headline `pnl` metric.

<br>

## Metrics

Every strategy run carries a `metrics` dict of named scalars. They are computed from
the equity curve and the trade log, plus an extra alignment pass for `alpha` and
`excess_return`. All return-flavored metrics are stored as fractions (e.g., `0.12` = 12%).

### Final equity & PnL

$$
\text{final_equity} = \text{equity_curve}[-1]
$$

$$
\text{pnl} = \text{final_equity} - \text{initial_cash}
$$

$$
\text{total_return} = \frac{\text{final_equity} - \text{initial_cash}}{\text{initial_cash}}
$$

`final_equity` is the last value of the equity curve, which itself is the sum
of every cash bucket converted to the portfolio base currency at each bar via
the FX table (forward-filled to the latest known rate ≤ the bar's
timestamp), plus the mark-to-market value of every open position at every
bar's close. Buckets or positions for which no FX rate is available at a
given timestamp fall back to a 1:1 conversion so equity stays a finite,
comparable number.

### n_trades & win_rate

$$
\text{n\_trades} = |\text{trades}|
\qquad
\text{win_rate} = \frac{|\{t \in \text{trades} : t.\text{pnl} > 0\}|}{\text{n_trades}}
$$

A trade is winning iff its `pnl` is strictly positive. Break-even trades (`pnl == 0`)
and losing trades (`pnl < 0`) are not counted as wins.

Only closed round-trips count toward `n_trades`. Positions still open at
the very last bar are auto-liquidated by the engine and the resulting closes
do flow into the trade list, so they are included as well.

### cagr & ann_volatility

These come from `compute_series_stats` applied to the equity
curve, so the analysis page and the backtest engine produce identical numbers.

The annualization factor `ann` is derived from the equity-curve density:

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
\text{ann_volatility} = \sigma(r) \cdot \sqrt{\text{ann}}
$$

### sharpe

The classic risk-adjusted return:

$$
\text{sharpe} = \frac{\bar{r} - r_f / \text{ann}}{\sigma(r)} \cdot \sqrt{\text{ann}}
$$

where $r_f$ is the annualised `risk_free_rate` (a fraction). The numerator is
the per-period excess return; the denominator is the per-period return
standard deviation. Multiplying by $\sqrt{\text{ann}}$ converts the ratio to
its annualized form. Returns 0 if returns have zero variance.

### sortino

Like Sharpe but punishes only downside deviation:

$$
\text{sortino} = \frac{\bar{r} - r_f / \text{ann}}{\sigma(r_-)} \cdot \sqrt{\text{ann}},
\qquad r_- = \{r_i : r_i < 0\}
$$

Useful when you don't want upside volatility to be penalised the same way as
downside volatility.

### max_dd

Maximum drawdown is the largest fractional drop from a running peak on the
cumulative-return path $C_i = \prod_{k \le i} (1 + r_k)$:

$$
\text{max_dd} = \min_i \frac{C_i - \max_{k \le i} C_k}{\max_{k \le i} C_k}
$$

It is always $\le 0$. A reading of `-0.25` means the equity curve, at its
worst, was 25 % below its all-time high.

### alpha

Alpha is the windowed total-return difference between the strategy and the
benchmark, computed only when a benchmark is configured. The window is aligned
to the *later* of the two equity-curve start dates so that strategies with
deeper history aren't penalized by missing benchmark data:

$$
\text{window_start} = \max(\text{strat_start}, \text{bench_start})
$$

$$
R(c) = \frac{c[-1] - c[\text{window_start}]}{c[\text{window_start}]}
$$

$$
\text{alpha} = R(\text{strategy}) - R(\text{benchmark})
$$

Positive alpha means the strategy out-performed buy-and-hold of the benchmark
over the overlapping period. Alpha is **not** computed on the benchmark run
itself.

### excess_return

Same idea as alpha but against the risk-free rate instead of the benchmark:

$$
\text{excess_return} = R(\text{strategy}) - \big((1 + r_f)^{n_\text{years}} - 1\big)
$$

with $n_\text{years} = (\text{strat_end} - \text{window_start}) / (365.25 \cdot 86400)$
and $r_f$ the configured `risk_free_rate / 100`.
