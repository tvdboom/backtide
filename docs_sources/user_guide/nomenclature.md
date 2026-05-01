# Nomenclature
--------------

This documentation consistently uses terms to refer to certain concepts
related to this package. The most frequent terms are described hereunder.

<br>

[](){#nom-bar}
<strong id="bar">bar</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A single OHLCV record representing price activity over one [interval][nom-interval]
— consisting of an open, high, low, close, adjusted close, and volume. Bars are the
fundamental unit of market data in Backtide. Also referred to as a *candle* or *candlestick*.
See [`Bar`].
</div>

<br>

[](){#base-currency}
<strong id="base-currency">base currency</strong>
<div markdown style="margin: -1em 0 0 1.2em">
The ISO 4217 currency code that all prices and portfolio values are normalised
to throughout Backtide. Configured globally via [`Config`].
</div>

<br>

[](){#nom-exchange}
<strong id="exchange">exchange</strong>
<div markdown style="margin: -1em 0 0 1.2em">
The marketplace on which an [instrument][nom-instrument] is listed and traded, such as NASDAQ,
NYSE, or Binance. The exchange determines the trading calendar and session hours used
when aligning bars across multiple instruments.
</div>

<br>

[](){#nom-instrument}
<strong>instrument</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A tradeable financial instrument, such as a stock, ETF, currency pair, or cryptocurrency.
Each instrument is uniquely identified by a [symbol][nom-symbol] and belongs to exactly
one [instrument type].
</div>

<br>

[](){#instrument-profile}
<strong id="instrument-profile">instrument profile</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A wrapper around an [instrument][nom-instrument] enriched with download metadata.
It carries the per-[interval][nom-interval] earliest and latest available timestamps
as well as the currency-conversion legs needed to reach the [base currency]. Instrument
profiles are resolved automatically when preparing a download and are the primary input
to the download pipeline. See [`InstrumentProfile`].
</div>

<br>

[](){#instrument-type}
<strong id="instrument-type">instrument type</strong>
<div markdown style="margin: -1em 0 0 1.2em">
The broad category an [instrument][nom-instrument] belongs to. These include stock
(individual equity shares), etf (exchange-traded funds), forex (spot foreign-exchange
pairs) or crypto (cryptocurrency spot pairs). The instrument type determines which
[provider][nom-provider] is used to fetch data for that instrument. See [`InstrumentType`].
</div>

<br>

[](){#nom-interval}
<strong id="interval">interval</strong>
<div markdown style="margin: -1em 0 0 1.2em">
The time resolution of a [bar][nom-bar], such as one minute, one hour, or one day.
All bars within a single dataset share the same interval. Also referred to as
*timeframe* or *granularity*. See [`Interval`].
</div>

<br>

[](){#nom-provider}
<strong id="provider">provider</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A data source from which Backtide fetches historical market data. Each
[instrument type] is mapped to exactly one active provider at a time.
</div>

<br>

[](){#nom-run}
<strong id="run">run</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A specific strategy execution within an experiment. An experiment that evaluates
N strategies produces N runs — each with its own equity curve, executed orders,
closed trades and summary metrics. Runs are persisted independently and can be
queried via [`query_strategy_runs`]. See [`RunResult`].
</div>

<br>

[](){#nom-symbol}
<strong id="symbol">symbol</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A short string that uniquely identifies an [instrument][nom-instrument]. Backtide uses a canonical
symbol convention since the same underlying instrument may carry different symbols across
[providers][nom-provider]. For stocks and ETFs, symbols are of the form expected by the
yahoo data provider (e.g., `AAPL` or `ASML.AS`). For forex and cryptos, symbols are of
the form `base-quote` (e.g., `BTC-USDT`, or `EUR-USD`).
</div>
