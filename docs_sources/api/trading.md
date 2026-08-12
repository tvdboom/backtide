# Trading APIs

Trading is paper simulation driven by normalized public WebSocket candles. Backtide does not
submit broker orders or store brokerage credentials.

## Public Python API

| Object | Purpose |
| --- | --- |
| `LiveMarketFeed` | Connect, reconnect, normalize, cancel, and collect bounded live updates. |
| `MarketUpdate` | Provider-independent OHLCV event used by live and replay execution. |
| `PaperTradingConfig` | Configure cash, fees, order types, fills, leverage, margin, financing, concentration, drawdown guards, and live metrics. |
| `PaperTradingSession` | Warm history, evaluate a strategy, compute monitoring indicators, and simulate fills deterministically. |
| `PaperTradingSnapshot` | Read portfolio, exposure, leverage, buying power, costs, drawdown, halt state, and selected metrics. |

## Local application integration

`/api/live` controls the active session. Pause, resume, flatten, cancel-all, session-history, and
replay routes expose the operational workflow. Persisted journals make recorded sessions
reproducible through a fresh paper engine.

See [Paper trading] for the wizard and operational semantics, and [Application endpoints] for the
complete endpoint table.

[Paper trading]: ../user_guide/live_trading.md
[Application endpoints]: application_endpoints.md
