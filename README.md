<div align="center">
<p align="center">
	<img src="https://github.com/tvdboom/backtide/blob/master/images/logo transparent.png?raw=true" alt="backtide" title="backtide" height="300" width="300"/>
</p>

## A refreshingly simple trading backtester for beginner retail investors
</div>

<br>

📜 Overview
-----------

**General Information** | |
--- | ---
**Repository** | [![Project Status: Active](https://www.repostatus.org/badges/latest/active.svg)](https://www.repostatus.org/#active) [![License: MIT](https://img.shields.io/github/license/tvdboom/backtide)](https://opensource.org/licenses/MIT) [![Downloads](https://static.pepy.tech/badge/backtide)](https://pepy.tech/project/backtide) [![PyPI version](https://img.shields.io/pypi/v/backtide)](https://pypi.org/project/backtide/)
**Build** | [![Publish](https://github.com/tvdboom/backtide/actions/workflows/publish.yml/badge.svg)](https://github.com/tvdboom/backtide/actions/workflows/publish.yml) [![Linting and tests](https://github.com/tvdboom/backtide/actions/workflows/test.yml/badge.svg)](https://github.com/tvdboom/backtide/actions/workflows/test.yml) [![codecov](https://codecov.io/gh/tvdboom/backtide/branch/master/graph/badge.svg)](https://codecov.io/gh/tvdboom/backtide)
**Code** | [![Python](https://img.shields.io/badge/python-3.11%20%7C%203.12%20%7C%203.13%20%7C%203.14-blue?logo=python)](https://www.python.org) [![uv-managed](https://img.shields.io/badge/uv-managed-blueviolet)](https://docs.astral.sh/uv/) [![PEP8](https://img.shields.io/badge/code%20style-pep8-orange.svg)](https://www.python.org/dev/peps/pep-0008/) [![ruff](https://custom-icon-badges.demolab.com/badge/Ruff-261230.svg?logo=ruff-logo)](https://docs.astral.sh/ruff/) [![ty](https://custom-icon-badges.demolab.com/badge/ty-261230.svg?logo=ty-astral-logo)](https://docs.astral.sh/ty/)

<br>

<table>
<tr>
<td><img src="https://github.com/tvdboom/backtide/blob/master/images/scenery/home.png?raw=true" alt="Home" width="100%"/></td>
<td><img src="https://github.com/tvdboom/backtide/blob/master/images/scenery/experiment.png?raw=true" alt="Experiment" width="100%"/></td>
</tr>
<tr>
<td><img src="https://github.com/tvdboom/backtide/blob/master/images/scenery/results.png?raw=true" alt="Results" width="100%"/></td>
<td><img src="https://github.com/tvdboom/backtide/blob/master/images/scenery/stats.png?raw=true" alt="Stats" width="100%"/></td>
</tr>
<tr>
<td><img src="https://github.com/tvdboom/backtide/blob/master/images/scenery/trades.png?raw=true" alt="Trades" width="100%"/></td>
<td><img src="https://github.com/tvdboom/backtide/blob/master/images/scenery/analysis.png?raw=true" alt="Analysis" width="100%"/></td>
</tr>
</table>

<br>

💡 Introduction
---------------

Backtide is an open-source backtesting platform for Python, built for retail
investors who want to test trading ideas without drowning in complexity. A
Rust-powered core keeps simulations fast, while sensible defaults let you go
from raw multi-exchange data to validated strategies in just a few lines of
code. Every setting can still be fine-tuned when needed, but you never have to.

<br>

❗ Why you should use Backtide?
-------------------------------

- **Fast** — Rust-powered engine runs backtests in a fraction of a second.
- **Simple** — Sensible defaults mean a working backtest in minutes, not hours of configuration.
- **Flexible** — Every parameter is exposed and customizable when you need full control.
- **Multi-exchange** — Stocks, ETFs, forex and crypto from Yahoo, Binance, Kraken and more.
- **Batteries included** — Built-in strategies and technical indicators out of the box.
- **Rich analytics** — 20+ plots cover PnL, returns, drawdown and more.
- **Interactive UI** — Professional UI to configure, run and analyze experiments visually.
- **Open source** — MIT-licensed, community-driven and free forever.

<br>

📈 Performance
--------------

Based on comprehensive [benchmarks](https://tvdboom.github.io/backtide/latest/contributing#benchmarks):

**Data download & storage**

| Operation                      | Performance    | Use Case        |
|--------------------------------|----------------|-----------------|
| OHLC download (1 symbol - 1m)* | ~33ms          | Data ingestion  |
| OHLC download (1 symbol - 1d)* | ~31ms          | Data ingestion  |
| Batch insert (100 bars)        | ~20ms          | Bulk processing |
| Batch insert (10k bars)        | ~45ms          | Bulk processing |
| Historical read (1000 bars)    | ~1.5ms         | Backtesting     |
| Historical read (1M bars)      | ~711ms         | Backtesting     |

*\*Downloads hit real network endpoints. Yahoo Finance applies rate limits, so these numbers are meant as a reference, not as a real benchmark.*

<br>

**Backtest (11k bars)**

| Strategy            | Performance    | Use Case    |
|---------------------|----------------|-------------|
| Buy & Hold          | ~1.1ms         | Backtesting |
| ROC Rotation        | ~1.1ms         | Backtesting |
| RSRS Rotation       | ~1.1ms         | Backtesting |
| Multi-BB Rotation   | ~1.2ms         | Backtesting |
| ROC                 | ~1.3ms         | Backtesting |
| Triple RSI Rotation | ~1.5ms         | Backtesting |
| VCP                 | ~1.5ms         | Backtesting |
| RSRS                | ~1.7ms         | Backtesting |
| Double Top          | ~2.2ms         | Backtesting |
| Momentum            | ~4.1ms         | Backtesting |
| SMA Naive           | ~4.2ms         | Backtesting |
| Alpha RSI Pro       | ~4.5ms         | Backtesting |
| Turtle Trading      | ~4.6ms         | Backtesting |
| Risk Averse         | ~4.9ms         | Backtesting |
| BB Mean Reversion   | ~5.5ms         | Backtesting |
| MACD                | ~6.3ms         | Backtesting |
| Adaptive RSI        | ~7.4ms         | Backtesting |
| SMA Crossover       | ~7.7ms         | Backtesting |
| Hybrid Alpha RSI    | ~7.9ms         | Backtesting |
| RSI                 | ~8.8ms         | Backtesting |

<br>

📘 Documentation
----------------

**Relevant links** | |
--- | ---
⭐ **[About](https://tvdboom.github.io/backtide/latest/about/)** | Learn more about the package.
🚀 **[Getting started](https://tvdboom.github.io/backtide/latest/getting_started/)** | New to backtide? Here's how to get you started!
👨‍💻 **[User guide](https://tvdboom.github.io/backtide/latest/user_guide/)** | How to use backtide and its features.
🎛️ **[API Reference](https://tvdboom.github.io/backtide/latest/API/backtide/api/)** | The detailed reference for backtide's API.
❔ **[FAQ](https://tvdboom.github.io/backtide/latest/faq/)** | Get answers to frequently asked questions.
🔧 **[Contributing](https://tvdboom.github.io/backtide/latest/contributing/)** | Do you wan to contribute to the project? Read this before creating a PR.
🌳 **[Dependencies](https://tvdboom.github.io/backtide/latest/dependencies/)** | Which other packages does backtide depend on?
📃 **[License](https://tvdboom.github.io/backtide/latest/license/)** | Copyright and permissions under the MIT license.
