# About
-------

## What is it?

Backtide is an open-source, local-first trading research platform for Python,
built for retail investors who want to test and observe trading ideas without
drowning in infrastructure. It combines fast historical simulation and live
simulated execution with a friendly Python API, interactive UI, local storage,
market-data integrations, built-in indicators, strategy templates, position
sizers and analysis plots.

The goal is simple: go from market data to reproducible evidence in minutes.
Test a strategy against historical bars, observe the same strategy on public
exchange WebSockets with locally simulated fills, and keep every important
setting configurable when you want more control. Click [here][getting-started]
to get started.

<br>

## What can I do with it?

Backtide covers the complete workflow for researching rule-based trading ideas:
download and store market data, configure historical experiments, run one or
more strategies, benchmark and analyze the results, then use those same
strategies in live sessions. Sessions can consume public exchange
WebSockets, record market events and replay them deterministically for later
inspection. Click on the icons to read more about its main functionalities.

<div class="row">
  <div class="column">
    <div class="icon">
      <a href="../user_guide/overview/application" draggable="false">
        <img src="../img/icons/application.svg" alt="Application" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Application</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/data/market_data" draggable="false">
        <img src="../img/icons/market_data.svg" alt="Market data" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Market data</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/backtest/experiment" draggable="false">
          <img src="../img/icons/experiments.svg" alt="Experiments" draggable="false">
          <figcaption style="margin-top: -8px"><strong>Experiments</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/library/strategies" draggable="false">
        <img src="../img/icons/strategies.svg" alt="Strategies" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Strategies</strong></figcaption>
      </a>
    </div>
  </div>
</div>
<div class="row">
  <div class="column">
    <div class="icon">
      <a href="../user_guide/library/performance" draggable="false">
        <img src="../img/icons/performance.svg" alt="Performance" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Performance</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/backtest/plots" draggable="false">
        <img src="../img/icons/plots.svg" alt="Plots" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Plots</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/data/storage" draggable="false">
        <img src="../img/icons/storage.svg" alt="Storage" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Storage</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/overview/configuration" draggable="false">
        <img src="../img/icons/configuration.svg" alt="Configuration" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Configuration</strong></figcaption>
      </a>
    </div>
  </div>
</div>


## Who is it intended for?

* **Beginner retail investors** who want to learn how a trading idea behaved
  historically and observe it on live market data without risking real money.
* **Python users** who want a clean API for market-data ingestion, technical
  indicators, backtesting, live simulation, storage and plotting without
  stitching together many separate tools.
* **Tinkerers and strategy builders** who want to compare built-in strategies,
  write custom strategies, test position sizing rules, and inspect every
  simulated order and trade across historical and live sessions.
* **Data-minded investors** who care about reproducibility: experiment configs,
  live-session events, results, equity curves, orders and trades are persisted
  locally for later analysis and replay.
* **Educators and learners** who want an approachable sandbox for portfolio
  mechanics, indicators, drawdowns, risk, currency conversion and benchmark
  comparisons.

!!! warning
    Backtide is intended for research and education. Live simulation uses simulated
    execution and never submits broker orders. Backtide does not provide financial
    advice or guarantee future returns.


<br>

## Support

Backtide recognizes the support from [JetBrains](http://www.jetbrains.com) by providing core project
contributors with a set of developer tools free of charge.

<div class="support-logos">
  <a href="https://www.jetbrains.com/community/opensource/#support">
    <img src="img/support/jetbrains.png" alt="JetBrains">
  </a>
  <a href="https://www.jetbrains.com/rustrover/">
    <img src="img/support/rustrover.png" alt="RustRover">
  </a>
  <a href="https://www.jetbrains.com/pycharm/">
    <img src="img/support/pycharm.png" alt="PyCharm">
  </a>
</div>

<br>

## Data integrations

<br>

<div class="row">
  <div class="column">
    <div class="logo">
      <a href="../user_guide/data/market_data/#yahoo-finance" draggable="false">
        <img src="../img/integrations/yahoo.png" alt="yahoo" draggable="false">
      </a>
    </div>
  </div>
  <div class="column">
    <div class="logo">
      <a href="../user_guide/data/market_data/#binance" draggable="false">
        <img src="../img/integrations/binance.png" alt="binance" draggable="false">
      </a>
    </div>
  </div>
  <div class="column">
    <div class="logo">
      <a href="../user_guide/data/market_data/#kraken" draggable="false">
        <img src="../img/integrations/kraken.png" alt="kraken" draggable="false">
      </a>
    </div>
  </div>
  <div class="column">
    <div class="logo">
      <a href="../user_guide/data/market_data/#coinbase" draggable="false">
        <img src="../img/integrations/coinbase.png" alt="coinbase" draggable="false">
      </a>
    </div>
  </div>
</div>
