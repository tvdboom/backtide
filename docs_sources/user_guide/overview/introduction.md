# Introduction

Trading starts with an idea: perhaps momentum persists, prices revert after an extreme move, or
risk can be controlled by changing position size. The difficult part is not describing the idea;
it is turning it into explicit rules and finding out where those rules fail.

Backtide is a local-first research workspace for that process. It helps you collect market data,
express a strategy, test it against history, inspect the trades and risk, and then observe the same
logic with current market data through simulated paper trading. The application and Python API use
the same saved strategies, indicators, metrics, sizers, configuration, and results.

## From an idea to evidence

A useful research loop has five stages:

1. **State the hypothesis.** Define the market behavior you expect and why it might persist.
2. **Make the rules precise.** Choose inputs, entry and exit conditions, position sizing, fees,
   slippage, and risk limits before looking at the result.
3. **Backtest.** Run the rules over historical data and inspect the full result—not only the final
   return, but also drawdown, rejected orders, individual trades, and unstable periods.
4. **Challenge the result.** Try other instruments and time windows, vary assumptions, and keep
   unseen data for confirmation. A good-looking chart is a reason to investigate, not proof.
5. **Paper trade.** Observe the strategy on current market updates with simulated fills before
   deciding whether the idea deserves any real-world use.

Backtide makes this loop reproducible: experiments retain their configuration and outputs, while
custom library objects let you reuse the same definitions in later tests.

## Choose a workflow

- Start with [Backtest introduction](../backtest/introduction.md) to learn what historical testing can
  and cannot tell you.
- Use [Experiments](../backtest/experiment.md) to configure and run a complete test.
- Read [Results](../backtest/results.md) and [Plots](../backtest/plots.md) to inspect returned objects and visual evidence.
- Continue with [Live introduction](../live/introduction.md) and [Paper trading](../live/paper_trading.md)
  when you want to observe a strategy against current exchange data.
- Build reusable logic in the [library][strategies]: strategies decide, indicators transform data,
  sizers choose quantities, and metrics summarize finished runs.

!!! warning
    Backtide is a research and education tool. Backtests and paper trading simplify execution and
    cannot predict future returns or guarantee that a real order would fill at the simulated price.
