# About
-------

## What is it?

Backtide is an open-source backtesting platform for Python, built for retail
investors who want to test trading ideas without drowning in infrastructure.
It combines a fast Rust simulation engine with a friendly Python API, interactive
UI, local storage, market-data integrations, built-in indicators, strategy
templates, position sizers and analysis plots.

The goal is simple: go from market data to a reproducible backtest in minutes,
while still keeping every important setting configurable when you want more
control. Click [here][getting-started] to get started.

<br>

## What can I do with it?

Backtide covers the complete workflow for testing rule-based trading ideas:
download and store market data, configure an experiment, run one or more
strategies, benchmark the results, and inspect what happened with plots and
trade-level analytics. Click on the icons to read more about its main
functionalities.

<div class="row">
  <div class="column">
    <div class="icon">
      <a href="../user_guide/application" draggable="false">
        <img src="../img/icons/application.svg" alt="Application" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Application</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/data" draggable="false">
        <img src="../img/icons/market_data.svg" alt="Market data" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Market data</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/experiment" draggable="false">
          <img src="../img/icons/experiments.svg" alt="Experiments" draggable="false">
          <figcaption style="margin-top: -8px"><strong>Experiments</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/strategies" draggable="false">
        <img src="../img/icons/strategies.svg" alt="Strategies" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Strategies</strong></figcaption>
      </a>
    </div>
  </div>
</div>
<div class="row">
  <div class="column">
    <div class="icon">
      <a href="../user_guide/strategies#performance" draggable="false">
        <img src="../img/icons/performance.svg" alt="Performance" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Performance</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/plots" draggable="false">
        <img src="../img/icons/plots.svg" alt="Plots" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Plots</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/storage" draggable="false">
        <img src="../img/icons/storage.svg" alt="Storage" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Storage</strong></figcaption>
      </a>
    </div>
  </div>
  <div class="column">
    <div class="icon">
      <a href="../user_guide/configuration" draggable="false">
        <img src="../img/icons/configuration.svg" alt="Configuration" draggable="false">
        <figcaption style="margin-top: -8px"><strong>Configuration</strong></figcaption>
      </a>
    </div>
  </div>
</div>


## Who is it intended for?

* **Beginner retail investors** who want to learn whether a trading idea would
  have worked historically before risking real money.
* **Python users** who want a clean API for market-data ingestion, technical
  indicators, strategy execution, storage and plotting without stitching
  together many separate tools.
* **Tinkerers and strategy builders** who want to compare built-in strategies,
  write custom strategies, test position sizing rules and inspect every order
  and trade.
* **Data-minded investors** who care about reproducibility: experiment configs,
  results, equity curves, orders and trades are persisted locally for later
  analysis.
* **Educators and learners** who want an approachable sandbox for portfolio
  mechanics, indicators, drawdowns, risk, currency conversion and benchmark
  comparisons.

!!! warning
    Backtide is intended for research and education. It helps you test assumptions;
    it does not provide financial advice nor guarantee future returns.


<br>

## Support

Backtide recognizes the support from [JetBrains](http://www.jetbrains.com) by providing core project
contributors with a set of developer tools free of charge.

<div align="center" markdown>
  [![JetBrains](img/support/jetbrains.png){ .icon width="200" height="200" }](https://www.jetbrains.com/community/opensource/#support)
  &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
  [![RustRover](img/support/rustrover.png){ .icon width="200" height="200" }](https://www.jetbrains.com/rustrover/)
  &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
  [![PyCharm](img/support/pycharm.png){ .icon width="200" height="200" }](https://www.jetbrains.com/pycharm/)
</div>

<br>

## Data integrations

<br>

<div class="row">
  <div class="column">
    <div class="logo">
      <a href="../user_guide/data/#yahoo-finance" draggable="false">
        <img src="../img/integrations/yahoo.png" alt="yahoo" draggable="false">
      </a>
    </div>
  </div>
  <div class="column">
    <div class="logo">
      <a href="../user_guide/data/#binance" draggable="false">
        <img src="../img/integrations/binance.png" alt="binance" draggable="false">
      </a>
    </div>
  </div>
  <div class="column">
    <div class="logo">
      <a href="../user_guide/data/#kraken" draggable="false">
        <img src="../img/integrations/kraken.png" alt="kraken" draggable="false">
      </a>
    </div>
  </div>
  <div class="column">
    <div class="logo">
      <a href="../user_guide/data/#coinbase" draggable="false">
        <img src="../img/integrations/coinbase.png" alt="coinbase" draggable="false">
      </a>
    </div>
  </div>
</div>
