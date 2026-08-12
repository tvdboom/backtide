# Research APIs

Research covers experiment configuration, historical simulation, stored results, and analysis.

## Public Python API

| Module | Main public surface |
| --- | --- |
| `backtide.backtest` | `ExperimentConfig`, its section configs, `run_experiment`, orders, trades, runs, and result models |
| `backtide.analysis` | Statistics plus price, P&L, risk, rolling, seasonality, and trade plots |
| `backtide.storage` | Stored experiment and strategy-run queries used for later analysis |

Use a completed experiment's **Paper trade** action to translate compatible settings into the
Trading setup wizard. Historical ranges and benchmark execution are intentionally not promoted.

## Local application integration

The Research UI uses `/api/experiments`, `/api/experiments/{id}`, paged order and log routes,
`/api/results/plot`, and `/api/analysis`. The promotion boundary is
`GET /api/experiments/{id}/paper-config`.

See [Application endpoints] for the complete method-level table and the `backtest` and `analysis`
sections below for generated object reference pages.

[Application endpoints]: application_endpoints.md
