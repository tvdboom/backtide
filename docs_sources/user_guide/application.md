# Application
--------------

Backtide includes a modern Vue web application for the complete research workflow:
download market data, manage reusable strategies and indicators, configure backtests,
inspect interactive Plotly results, analyze stored series, and run live paper-trading
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
| **Research** | Experiments | Configure data, portfolios, reusable assets, execution assumptions, risk, and engine behavior. |
| **Research** | Results | Compare completed runs and inspect metrics, equity, trades, orders, configuration, and logs. |
| **Research** | Analysis | Build interactive price, return, correlation, seasonality, volatility, volume, and dividend plots. |
| **Trading** | Paper trading | Configure and run live-data strategies with local simulated execution. See [Paper trading]. |
| **Trading** | Session history | Inspect persisted journals and replay a prior session through a fresh paper engine. |
| **Library** | Strategies | Create and manage strategies used by research and paper trading. |
| **Library** | Indicators | Manage strategy dependencies and optional research/live monitoring indicators. |
| **Library** | Metrics | Browse built-in and custom performance definitions. |
| **Library** | Sizers | Save reusable built-in presets or custom position-sizing policies. |
| **Data** | Download | Resolve instruments and download normalized OHLCV bars. |
| **Data** | Storage | Inspect coverage, open stored series in Analysis, or delete selected data. |

Searchable selectors are keyboard accessible and remain usable on smaller screens.
Long-running downloads and experiments run in background jobs, so the interface can
show progress and accept cancellation without blocking navigation.

<br>

## How the frontend talks to Backtide

The Vue single-page application uses same-origin JSON endpoints exposed by the local
Python service. That service validates browser input and calls the public Python API;
CPU-heavy backtests, paper fills, strategies, indicators, provider clients, and DuckDB
operations continue to run in the Rust core.

```text
Vue application -> local JSON API -> Python facade -> Rust/PyO3 engine
                                              |-> DuckDB storage
                                              `-> provider REST/WebSockets
```

Plotly figures are created by the existing Python analysis functions and serialized
to the browser, so the web app and Python API use the same calculations. The browser
does not read the database directly and does not contain a second implementation of
trading logic.

<br>

## Frontend development

Package users can skip this section. Contributors changing `frontend/` install its
development dependencies, run unit tests, and rebuild the committed production bundle:

```console
just frontend-sync
just frontend-test
just frontend-build
```

The build output is written to `src/backtide/ui/static/` and must be committed with
the source change. `just launch` always serves that production bundle through the
same backend used by an installed wheel.

<br>

## Screenshots

![Home](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/home.png)
![Experiment](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/experiment.png)
![Results](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/results.png)
![Paper trading](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/live.png)
![Storage](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/storage.png)
![Analysis](https://raw.githubusercontent.com/tvdboom/backtide/master/images/scenery/analysis.png)
