<div align="center">
<p align="center">
	<img src="https://github.com/tvdboom/backtide/blob/master/images/logo transparent.png?raw=true" alt="backtide" title="backtide" height="300" width="300"/>
</p>

## A refreshingly simple trading backtester for beginner retail investors
</div>

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

- **Fast** — Rust-powered engine runs backtests orders of magnitude faster than pure-Python alternatives.
- **Simple** — Sensible defaults mean a working backtest in minutes, not hours of configuration.
- **Flexible** — Every parameter is exposed and customizable when you need full control.
- **Multi-exchange** — Stocks, ETFs, forex and crypto from Yahoo, Binance, Kraken and more.
- **Batteries included** — 20 predefined strategies and 12 technical indicators out of the box.
- **Interactive UI** — Professional UI to configure, run and analyze experiments visually.
- **Open source** — MIT-licensed, community-driven and free forever.

<br>

📈 Performance
--------------

Based on comprehensive [benchmarks](https://tvdboom.github.io/backtide/latest/contributing#benchmarks):

| Operation                         | Performance | Use Case         |
|-----------------------------------|-------------|------------------|
| OHLC download (1 symbol - 1m)     | ~22ms       | Data ingestion   |
| OHLC download (10 symbols - 1d)   | ~40ms       | Data ingestion   |
| Batch bar insert (100)            | ~22ms       | Bulk processing  |
| Batch bar insert (10000)          | ~48ms       | Bulk processing  |
| Historical read (1 symbol)        | ~2.8ms      | Backtesting      |
| Historical read (10 symbols)      | ~14ms       | Backtesting      |


<br>

