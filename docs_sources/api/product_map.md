# Public API product map

Backtide's public Python API follows the same product structure as the application.
The detailed reference remains grouped by Python module, while this page maps those
modules to the workflow in which users encounter them.

## Overview product area

- `backtide.config` loads application, storage, display, and provider configuration.
- `backtide launch` opens the local application.

## Research product area

- `backtide.backtest` configures and runs experiments.
- `backtide.analysis` calculates statistics and creates result plots.
- Experiment, run, order, trade, and equity models live under the backtest model reference.

## Trading product area

- `backtide.live.LiveMarketFeed` consumes normalized public WebSocket candles.
- `backtide.live.PaperTradingSession` evaluates strategies and simulates local execution.
- `PaperTradingConfig` contains fees, execution, margin, risk, metrics, and bounded-history
  settings shared by live paper sessions and deterministic replays.

## Library product area

- `backtide.strategies` contains built-in and custom strategy contracts.
- `backtide.indicators` contains indicator implementations used by research and trading.
- `backtide.metrics` defines experiment measures; the built-in live-compatible subset can also be
  maintained incrementally during paper sessions.
- `backtide.sizers` contains reusable position-sizing policies.

## Data product area

- `backtide.data` discovers instruments and downloads normalized bars.
- `backtide.storage` queries and manages locally persisted market and experiment data.

See [Application endpoints] for the same mapping at the local JSON API boundary.
