# Introduction
--------------

## What is a backtest?

A backtest is a simulation that applies a trading strategy to historical
market data and records what would have happened — fills, positions, equity
changes, commissions, slippage and drawdowns — as if the strategy had been
running in real time.

Instead of risking real money to see whether an idea works, you replay the
past and measure the result. A single backtest answers questions like:

- Would buying every time the 50-day moving average crosses above the 200-day
  average and selling on the opposite cross have made money on the S&P 500 over
  the last 10 years?
- What would my maximum drawdown have been?
- How does this strategy compare to simply holding the index?

Past performance never guarantees future returns, but it's the closest thing
retail investors have to a laboratory for trading ideas.

<br>

## Why should you backtest?

Most retail investors rely on intuition, tips, or a handful of cherry-picked
chart examples to decide whether a strategy is worth following. Backtesting
replaces anecdotes with data:

| Benefit                   | What it means in practice                                                                                                                                                |
|---------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **Objective evaluation**  | Numbers replace gut feelings. You see *exact* returns, drawdowns, win rates and risk-adjusted ratios before committing any capital.                                      |
| **Risk awareness**        | A strategy that returned 40% but had a 60% drawdown on the way is very different from one that returned 30% with a 10% drawdown. Backtesting surfaces those differences. |
| **Parameter sensitivity** | Changing a single look-back period from 14 to 20 days can flip a strategy from profitable to losing. Running multiple experiments reveals which parameters are fragile.  |
| **Benchmark comparison**  | It's easy to feel good about a 15% return — until you notice the benchmark returned 25%. Backtesting against a benchmark keeps expectations honest.                      |
| **Emotional detachment**  | Real-time trading invites fear and greed. A backtest is dispassionate: it follows the rules exactly, every single bar.                                                   |
| **Faster learning**       | You can compress years of market experience into minutes of simulation time and iterate on ideas much faster than trading live.                                          |

!!! warning
    Backtesting has well-known pitfalls: **overfitting** (tuning parameters
    until they match the past perfectly), **look-ahead bias** (using future data
    that wouldn't have been available in real time), and **survivorship bias**
    (testing only on assets that still exist). Backtide's engine guards against
    look-ahead bias by default, but overfitting and data quality are the user's
    responsibility.

<br>

## How Backtide compares to other Python backtesters

The Python ecosystem has several well-known backtesting libraries. Each one
makes different tradeoffs between speed, flexibility, ease of use and feature
scope. The table below summarizes how Backtide sits relative to the most
popular alternatives.

| Feature                      | Backtide                         | Backtrader            | Zipline          | vectorbt        | bt             |
|------------------------------|----------------------------------|-----------------------|------------------|-----------------|----------------|
| **Engine language**          | Rust                             | Python                | Python / Cython  | Python / NumPy  | Python         |
| **Event-driven loop**        | ✅                                | ✅                     | ✅                | ❌ (vectorized)  | ❌ (vectorized) |
| **Built-in strategies**      | 20+ ready to use                 | ❌                     | ❌                | ❌               | ❌              |
| **Built-in indicators**      | 12+ (Rust)                       | 100+ (Python)         | ~10 (via ta-lib) | Via pandas-ta   | ❌              |
| **Built-in position sizers** | 7 (Rust)                         | 1                     | 1                | ❌               | ❌              |
| **Multi-asset support**      | ✅                                | ✅                     | ✅                | ✅               | ✅              |
| **Currency conversion**      | Automatic FX table               | Manual                | ❌                | ❌               | ❌              |
| **Interactive UI**           | Streamlit app                    | ❌                     | ❌                | ❌               | ❌              |
| **Data download**            | Yahoo, Binance, Kraken, Coinbase | Manual                | Quandl bundle    | Manual          | Manual         |
| **Local storage**            | DuckDB                           | ❌                     | HDF5 bundle      | ❌               | ❌              |
| **Analysis plots**           | 20+ built-in                     | Manual via matplotlib | Via pyfolio      | Built-in        | Built-in       |
| **Configuration**            | TOML + Python API                | Python only           | Python + YAML    | Python only     | Python only    |
| **Custom strategies**        | Python class                     | Python class          | Python class     | NumPy functions | Python tree    |
| **Actively maintained**      | ✅                                | ⚠️ Stale              | ⚠️ Archived      | ✅               | ✅              |

<br>

### Where Backtide stands out

**Speed without complexity.** The core simulation loop — bar alignment, order
matching, portfolio accounting, equity tracking, FX conversion, indicator
computation and all built-in strategies — runs in compiled Rust. You write
plain Python; the engine does the heavy lifting at native speed.

**Batteries included.** Most backtesters give you an engine and leave the rest
to you: data download, storage, indicators, position sizing, result analysis,
plotting. Backtide ships all of these out of the box so you can go from an
idea to a result without gluing together five different libraries.

**Interactive application.** The built-in Streamlit app lets you configure
experiments, run strategies and inspect results visually — no code required.
When you do need code, the same API powers the app and your scripts, so
there is zero translation overhead.

**Reproducibility by default.** Every experiment config, equity curve, order
log and trade log is persisted to local storage. Returning to an old experiment
months later is as easy as querying it.

<br>

## Why Backtide works better for beginner and retail investors

Institutional backtesting platforms (and even some open-source ones) assume
you already know what you're doing: you're expected to write your own strategy
from scratch, hook up a data pipeline, handle position sizing, implement
commissions and slippage, and build your own analysis dashboard. That's a lot
of work before you even get to the question you started with: *"Would this
idea have worked?"*

Backtide flips the script:

1. **Start without code.** Launch the app, pick a few symbols, select a
   built-in strategy, hit *Run*. You have a full backtest result — equity
   curve, trade log, benchmark comparison and 20+ analysis plots — in under a
   minute. No Python required.

2. **Learn incrementally.** When you're ready to go deeper, switch to the
   Python API. The same concepts you learned in the app (experiments,
   strategies, indicators, sizers) map one-to-one to Python classes and
   functions.

3. **Sensible defaults everywhere.** Commission, slippage, warmup periods,
   empty-bar handling, currency conversion — all have reasonable defaults that
   match what a retail investor trading through a typical online broker would
   experience. You only need to change what you want to change.

4. **Built-in strategies as learning tools.** The 20+ built-in strategies
   (SMA Crossover, RSI, Bollinger Mean Reversion, Turtle Trading, portfolio
   rotations, etc.) are not just trade-ready — they are *readable examples* of
   how to structure and evaluate trading ideas. Compare their performance
   side-by-side to build intuition before writing your own.

5. **Guard rails against common mistakes.** The engine prevents look-ahead
   bias by only feeding data up to the current bar to each strategy call.
   Auto-injected indicators are deduplicated so you don't accidentally compute
   conflicting values. Orders that would exceed available cash or violate margin
   limits are rejected with a clear reason string.

6. **Affordable infrastructure.** Everything runs locally on your machine.
   There are no API keys, cloud subscriptions, or rate-limited data feeds to
   manage. Market data from Yahoo Finance, Binance, Kraken and Coinbase is
   downloaded and stored in a local database for fast, offline access.

In short, Backtide is designed so that **your first backtest takes minutes, not
days** — and the same tool scales with you as your skills and ambitions grow.

!!! tip
    If you are completely new to backtesting, start with the [Getting started]
    guide, then run a simple [Buy-and-Hold experiment][experiment] to see the full
    workflow end to end.
