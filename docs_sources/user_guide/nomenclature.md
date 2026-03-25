# Nomenclature
--------------

This documentation consistently uses terms to refer to certain concepts
related to this package. The most frequent terms are described hereunder.

<br>

[](){#asset}
<strong>asset</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A tradeable financial instrument, such as a stock, ETF, currency pair, or
cryptocurrency. Each asset is uniquely identified by a [symbol] and belongs
to exactly one [asset type].
</div>

<br>

[](){#asset-type}
<strong id="asset-type">asset type</strong>
<div markdown style="margin: -1em 0 0 1.2em">
The broad category an [asset] belongs to. Backtide distinguishes between four
asset types: stocks (individual equity shares), ETFs (exchange-traded funds),
forex (spot foreign-exchange pairs), and crypto (cryptocurrency spot pairs).
The asset type determines which [provider] is used to fetch data for that asset.
</div>

<br>

[](){#bar}
<strong id="bar">bar</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A single OHLCV record representing price activity over one [interval] — consisting
of an open, high, low, close, adjusted close, and volume. Bars are the fundamental
unit of market data in Backtide. Also referred to as a *candle* or *candlestick*.
</div>

<br>

[](){#base-currency}
<strong id="base-currency">base currency</strong>
<div markdown style="margin: -1em 0 0 1.2em">
The ISO 4217 currency code that all prices and portfolio values are normalised
to throughout Backtide. Configured globally via [`Config`].
</div>

<br>

[](){#exchange}
<strong id="exchange">exchange</strong>
<div markdown style="margin: -1em 0 0 1.2em">
The marketplace on which an [asset] is listed and traded, such as NASDAQ, NYSE,
or Binance. The exchange determines the trading calendar and session hours used
when aligning bars across multiple assets.
</div>

<br>


[](){#ingestion}
<strong id="ingestion">ingestion</strong>
<div markdown style="margin: -1em 0 0 1.2em">
The process of fetching raw market data from a [provider] and writing it to the
local database. Ingestion is idempotent — re-running it for a period that has
already been stored will not produce duplicate records.
</div>

<br>

[](){#interval}
<strong id="interval">interval</strong>
<div markdown style="margin: -1em 0 0 1.2em">
The time resolution of a [bar], such as one minute, one hour, or one day. All bars
within a single dataset share the same interval. Also referred to as *timeframe* or
*granularity*.
</div>

<br>

[](){#provider}
<strong id="provider">provider</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A data source from which Backtide fetches historical market data. Each
[asset type] is mapped to exactly one active provider at a time, configured
via [`ProviderConfig`].
</div>

<br>

[](){#symbol}
<strong id="symbol">symbol</strong>
<div markdown style="margin: -1em 0 0 1.2em">
A short, provider-specific string that uniquely identifies an [asset] within a
data source — for example `AAPL`, `BTC/USDT`, or `EURUSD=X`. The same underlying
asset may carry different symbols across different [providers][provider].
</div>
