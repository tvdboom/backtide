# Application
--------------

Backtide includes a modern Vue web application for the complete research workflow:
download market data, manage reusable strategies and indicators, configure backtests,
inspect interactive Plotly results, analyze stored series, and run live
sessions. The app and API run locally on your machine.

The production frontend is bundled into every Python wheel. Installing Backtide does
not install or require Node.js, npm, or pnpm; those tools are used only by contributors
who rebuild the frontend source.

<br>

## Launching the app

Start the application from the command line:

```console
backtide launch
```

This starts Backtide's local HTTP and JSON API server at `http://localhost:8501`
and opens the application in the default browser. Customize the listener through
the [configuration][configuration] or command-line options:

```console
backtide launch --address 0.0.0.0 --port 9000
```

You can also launch it programmatically:

```python
from backtide.ui import launch

launch(address="localhost", port=8501)
```

Binding to `0.0.0.0` makes the local app reachable from other devices on the same
network. Use a firewall and a trusted network when exposing the listener because
the app can start jobs and modify the configured local Backtide store.

<br>

## Sections

| Group | Section | Purpose |
|---|---|---|
| **Overview** | Home | Review recent experiments, stored datasets, active sessions, and workflow shortcuts. |
| **Backtest** | New experiment | Configure a single run or study across market data, portfolio, strategy, metrics, execution, risk, and engine settings. |
| **Backtest** | Results | Compare completed runs and inspect metrics, equity, trades, orders, configuration, and logs. |
| **Live** | Live session | Configure and run live-data strategies with local simulated execution. See [Live simulation]. |
| **Live** | Session history | Inspect persisted journals and replay a prior session through a fresh simulation engine. |
| **Library** | Strategies | Create and manage strategies used by backtests and live simulation. |
| **Library** | Indicators | Manage strategy dependencies and optional backtest/live monitoring indicators. |
| **Library** | Metrics | Browse built-in and custom performance definitions. |
| **Library** | Sizers | Save reusable built-in presets or custom position-sizing policies. |
| **Data** | Download | Resolve instruments and download normalized OHLCV bars. |
| **Data** | Storage | Inspect coverage, open stored series in Analysis, or delete selected data. |
| **Data** | Analysis | Compare stored series with metrics and interactive price, return, correlation, seasonality, volatility, volume, VWAP, and dividend plots. |

Searchable selectors are keyboard accessible and remain usable on smaller screens.
Long-running downloads and experiments run in background jobs, so the interface can
show progress and accept cancellation without blocking navigation.

<br>

## How the application runs

The web application and Python API use the same calculations, configuration, and local storage.
The browser never reads the database directly, and long-running work can report progress or be
canceled without blocking navigation.

<br>

## Screenshots

![Home](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/home.png)
![Experiment](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/experiment.png)
![Results](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/results.png)
![Live simulation](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/live.png)
![Storage](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/storage.png)
![Analysis](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/analysis.png)
