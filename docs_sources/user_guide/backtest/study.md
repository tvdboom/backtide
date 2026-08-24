# Study {#study-guide}
--------------------

A study contains multiple related experiments. Backtide uses those experiments to assess a
strategy's robustness across a neighborhood of constructor values instead of relying on one
fortunate parameter choice. Each parameter combination is a candidate experiment with the same
data, portfolio, execution, risk, engine, and metric settings. Optional walk-forward experiments
then test whether a candidate selected on training data continues to work on untouched data.

In Backtide terminology, **study** is the container and **experiment** is one evaluation within it.

Use a study to answer three related questions:

- Does performance remain acceptable near the best parameter combination?
- Does the winner still work after minimum-trade and drawdown constraints are applied?
- Does a candidate selected on a training window retain performance on the untouched period that
  follows it?

!!! warning
    Robustness is evidence about parameter stability, not proof of future profitability.

<br>

## Configure a study in the application

Open **Experiment** and change **Run mode** from **Single run** to **Study**. The same
eight experiment tabs remain available. Market data, portfolio, indicators, metrics, execution,
risk, and engine behavior are applied unchanged to every candidate.

On the **Strategy** tab:

1. Select exactly one saved strategy.
2. Enable one or more numeric constructor parameters in **Parameter sweep**.
3. Set an inclusive minimum, maximum, and positive step for each enabled parameter.
4. On the **Metrics** tab, put the metric that should rank candidates first. This is the main
   experiment metric and therefore the study objective as well.
5. Optionally require a minimum number of closed trades or a maximum drawdown.
6. Optionally enable walk-forward validation and configure its training, test, and step lengths.
   Backtide creates folds from the history returned by the full-sample experiments, whether the
   market-data range uses explicit dates or full available history.

Backtide uses each metric's preferred direction when ranking candidates. See
[Custom Python metrics](../library/metrics.md#custom-python-metrics) for how built-in and custom
metrics define whether larger or smaller values are preferred.

The candidate count is the product of the enabled parameter value counts. For example, three
`fast` values and four `slow` values create 12 candidates. Backtide limits a study to 10,000
combinations so an accidental range cannot create unbounded work.

!!! tip "Start with a coarse grid"
    Sweep a small, meaningful neighborhood first. A dense grid across many parameters can consume
    substantial time while making the winner more vulnerable to multiple-testing bias.

<br>

## Run a study from the CLI

Use `backtide run-study study.toml` to run the same parameter sweep without opening the
application. A study file contains a `config` mapping with the shared experiment settings and a
`study` mapping with `parameter_space`, selection constraints, and optional walk-forward settings.
TOML, YAML, and JSON files are supported. See the [`run-study` CLI reference](../../cli/run_study.md)
for a complete configuration example and all command options.

<br>

## Custom strategies

Saved custom strategies use the same controls as built-ins. Backtide reads the class constructor
signature and the matching values stored on the saved instance. Each candidate is a fresh instance,
so mutable state cannot leak from one combination or fold to another.

Custom strategies must preserve their constructor configuration on same-named attributes or
provide constructor defaults. See
[Preserve constructor configuration](../library/strategies.md#constructor-configuration) for the
recommended pattern.

Positional-only and variadic constructor arguments cannot be swept because they do not provide a
stable named configuration. Non-numeric constructor parameters remain fixed at their saved values.

<br>

## Walk-forward validation

Every fold has a training window followed immediately by an untouched test window. Backtide runs
all candidates on training data, selects the best eligible candidate using only that window, then
runs only that selected constructor configuration on the test data.

!!! info
    Walk-forward results are diagnostic only. They do not affect the overall candidate ranking,
    winner, or headline metric.

The earliest and latest equity samples in the full-sample parent result define the available
walk-forward range. You do not need to enter separate dates for validation.

With rolling training, both boundaries advance by `step_days`. With anchored training, the first
training date stays fixed while its end expands. If `step_days` is empty, it defaults to the test
window length. Only complete folds are evaluated.

Training and test experiments are temporary. Their detailed engine artifacts are removed after the
fold is summarized, preventing the Results list from filling with internal runs.

<br>

## Read the results

The Results page groups the experiments into one **Study** card instead of showing a separate card
for every parameter combination. Its detail view contains four tabs:

- **Sweep** shows candidate counts and a heatmap when exactly two parameters were swept. Other
  dimensions use a table.
- **Candidates** lists every combination, main-metric value, trade count, eligibility, and rank.
- **Walk-forward** shows the selected parameters and training/test main metric for every fold.
- **Report** summarizes the best eligible full-sample candidate, selection constraints, and
  favorable out-of-sample folds.

Charts, selectors, and fold summaries identify combinations with compact names such as `C1` and
`C2`. Exact constructor values stay in the Parameters columns and the information marker beside a
candidate name, so a large parameter set never expands chart legends. A candidate is eligible only
when its run succeeds, computes the main metric, reaches the configured minimum trade count, and
does not breach the optional maximum-drawdown limit. Only eligible candidates can win the study.

Choose **Reuse best setup** in the study header to create a normal single-run experiment draft. The
action saves an isolated strategy copy named with the source strategy and compact candidate name,
applies the winning constructor values, and carries the study's other experiment settings forward.
The original saved strategy is not changed.

Choose **Rerun study** to reopen the builder in study mode with the same parameter values,
eligibility constraints, and walk-forward settings. Shared experiment settings come from the
study's `config.toml`; sweep and validation settings come from `study.json`.

The study owns the full-sample experiment results. Backtide keeps every candidate in storage for
reproducibility, but the standard detailed charts load only the three highest-ranked eligible runs
plus the benchmark. The compact `study.json` beside `config.toml` stores the complete candidate and
fold summaries.

For Python usage, see the [study example](../../examples/backtest/study.md) and the
[`Study` API](../../api/backtest/study.md).
