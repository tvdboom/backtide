# Data
------

Backtide keeps the user-facing data workflow provider-independent. You request
bars for canonical symbols, Backtide routes each instrument type to its configured
provider, determines which history is actually available for each interval, and
automatically adds any currency-conversion pairs needed to value everything in
the portfolio base currency.

By default, the provider mapping is:

| instrument type | Default provider |
| --- | --- |
| Stocks | `yahoo` |
| ETF | `yahoo` |
| Forex | `yahoo` |
| Crypto | `binance` |

You can override those defaults in the [configuration]. Any instrument type that's not
overridden keeps its default provider.

<br>

## Canonical symbols

Backtide uses a canonical symbol format so the same instrument can be referred to
consistently even when providers use different native tickers.

- For **stocks** and **ETFs**: The canonical symbol is the Yahoo-style ticker, e.g., `AAPL` or `ASML.AS`.
- For **forex** and **crypto**: The canonical symbol is always `base-quote`, e.g., `BTC-USD` or `ETH-USDC`.

This canonical layer matters for two reasons:

1. Your code stays stable when providers differ.
2. Backtide can automatically inject currency-conversion legs using the same
   symbol convention.

In short, you should think and work in canonical symbols. Provider-specific
translations are an internal implementation detail handled by Backtide.

<br>

## Providers

Backtide currently supports four market-data providers. Yahoo can serve all
instrument classes, while Binance, Coinbase, and Kraken are primarily crypto
providers (though some, like Kraken, also list major forex pairs).

### Yahoo Finance

- Supports stocks, ETFs, forex, and crypto.
- Uses Yahoo-style symbols for equities, which is why equity canonical symbols
  also follow Yahoo's naming (for example `AAPL` or `ASML.AS`).

Important caveats:

- Yahoo intraday availability is clamped to the following rolling windows:
  - `1m`: Last 7 days.
  - `5m`, `15m`, `30m`: Last 60 days.
  - `1h`, `4h`: Last 730 days.
- Daily and weekly history usually go back to the instrument's first trade
  date.

### Binance

- Supports crypto only.
- Uses Binance's public spot REST API; no authentication is required.
- Canonical symbols such as `BTC-USDT` are translated to Binance's compact
  symbols such as `BTCUSDT`.
- Instrument discovery is based on Binance spot pairs with status `TRADING`.

Important caveats:

- Binance providers can only be used for crypto instrument types.
- Binance symbol formatting differs from Backtide's canonical format, but the
  translation is handled automatically.

### Kraken

- Supports crypto and forex.
- Kraken-specific ticker aliases are normalized back to canonical names, e.g.,
  `XBT` â†’ `BTC` and `XDG` â†’ `DOGE`.

Important caveats:

- Kraken history is effectively bounded by a 720-bar window per interval, so
  high-frequency intervals have a much shorter accessible history than daily
  or weekly intervals.

!!! tip "Kraken as a forex provider"

    Kraken lists major forex pairs such as `EUR-USD` and `GBP-USD` alongside
    its crypto offerings. You can point the [forex provider][nom-provider] at
    Kraken so that [currency conversion] legs are sourced from the same exchange
    as your crypto trades. This is useful when you want to model a Kraken-only
    portfolio and keep all price data consistent with a single provider.

    ```toml
    [data.providers]
    forex  = "kraken"
    crypto = "kraken"
    ```

### Coinbase

- Supports crypto only.
- Instrument discovery only includes online spot products.

Important caveats:

- Coinbase does not support the `1w` interval.

<br>

## Currency conversion

All values in Backtide are normalized to the project's base currency. If a symbol
is quoted in another currency, Backtide resolves the conversion path automatically
and downloads the required legs together with the requested bars.

Examples:

- A forex pair whose quote currency is already the base currency needs no extra
  conversion.
- A stock priced in `EUR` with `base_currency="USD"` requires an `EUR-USD`
  conversion leg.
- A crypto pair may be routed through a triangulation currency such as `USDT`
  before reaching the base currency.

The conversion path is controlled by `triangulation_strategy`:

- `direct`: Prefer a direct conversion when one exists, such as `JPY-EUR`.
- `earliest`: Compare the direct path with a triangulated path and keep the one
  that reaches furthest back in history.

For triangulation, Backtide uses the following configuration:

- `triangulation_fiat` for fiat-to-fiat routing.
- `triangulation_crypto` for crypto routing.
- `triangulation_crypto_pegged` for the fiat currency that the chosen
  crypto intermediary is assumed to be pegged to.
