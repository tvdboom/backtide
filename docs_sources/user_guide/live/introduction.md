# Live Introduction

Backtide's live workflow is **live simulation**: it consumes current WebSocket market updates, runs a
strategy, and simulates orders, fills, equity, and metrics locally. It does not connect to a broker
or submit real orders.

Live simulation is the bridge between a historical backtest and real-world observation. It lets you
check whether symbols, bar timing, indicators, currency conversion, and strategy decisions behave
as expected while the market is moving. It is also useful for discovering operational assumptions
that historical data hides, such as delayed updates, incomplete candles, and reconnects.

## Recommended workflow

1. Backtest the strategy across more than one market period.
2. Select a supported live provider and instruments with the required quote currencies.
3. Reuse the strategy, indicators, sizer, fees, and risk settings from the experiment where
   possible.
4. Start a live session and monitor connection state, orders, fills, positions, and metrics.
5. Stop the session explicitly and review its persisted history before changing the rules.

The simulation only evaluates completed candles for strategy decisions. Incoming partial candles
can still appear in the market feed, but they do not cause repeated decisions for the same bar.

Continue with the [Live simulation guide](sessions.md) for the application, CLI, and Python
interfaces. The [complete session example](../../examples/live/session.md) shows the
full lifecycle in one script.

!!! note
    Live-session results can differ from a backtest because the observed period, update timing, and
    simulated execution path differ. They can also differ from real trading because exchange
    latency, queue position, liquidity, taxes, and broker behavior are not fully modeled.
