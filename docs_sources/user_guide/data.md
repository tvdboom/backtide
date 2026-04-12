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

## Downloading data

The Streamlit UI handles downloading for you, but you can also drive the
entire workflow from Python. The key concept is the two-step pipeline:
**resolve**, then **download**.

### Instruments vs. profiles

An [`Instrument`] is a lightweight descriptor — it knows a symbol's name, quote
currency, exchange, and instrument type, but nothing about the data that is actually
available.  An [`InstrumentProfile`] wraps an instrument with the metadata needed
to download it: the per-interval date range the provider can serve and the
currency-conversion legs required to reach the portfolio base currency.

You can fetch instruments directly with [`get_instruments`], but to download
bars you need profiles.

### Resolving profiles

The [`resolve_profiles`] function takes a list of symbols, an instrument type, and
one or more intervals. It queries the provider for each symbol's available date range,
works out which FX or crypto legs are needed for currency conversion, and returns
a flat, deduplicated list of [`InstrumentProfile`].

```pycon
from backtide.data import resolve_profiles

profiles = resolve_profiles(
    ["AAPL", "MSFT"],
    instrument_type="stocks",
    interval=["1d", "1h"],
)

for p in profiles:
    print(p.symbol, p.earliest_ts, p.latest_ts, p.legs)
```

### Downloading bars

Pass the resolved profiles to [`download_instruments`].  It checks what is
already in the database, downloads only the missing portions, and writes
everything (including any dividend data for stocks and ETFs) in a single
bulk transaction.

```pycon
from backtide.data import resolve_profiles, download_instruments

profiles = resolve_profiles(["AAPL", "MSFT"], "stocks", "1d")
result = download_instruments(profiles)

print(result.n_succeeded, "succeeded,", result.n_failed, "failed")
```

By default, the full provider history is downloaded. You can restrict the window
with Unix-timestamp boundaries:

```pycon
from datetime import datetime, timezone
from backtide.data import resolve_profiles, download_instruments

profiles = resolve_profiles("BTC-EUR", "crypto", "1d")
result = download_instruments(
    profiles,
    start=int(datetime(2024, 1, 1, tzinfo=timezone.utc).timestamp()),
    end=int(datetime(2024, 4, 1, tzinfo=timezone.utc).timestamp()),
)

print(result.n_succeeded, "succeeded,", result.n_failed, "failed")
```

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

- **direct**: Prefer a direct conversion when one exists, such as `JPY-EUR`.
- **earliest**: Compare the direct path with a triangulated path and keep the one
  that reaches furthest back in history.

For triangulation, Backtide uses the following configuration:

- **triangulation_fiat** for fiat-to-fiat routing.
- **triangulation_crypto** for crypto routing.
- **triangulation_crypto_pegged** for the fiat currency that the chosen
  crypto intermediary is assumed to be pegged to.
