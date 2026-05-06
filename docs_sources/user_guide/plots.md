# Plots
-------

Backtide provides many plotting methods to analyze the data or compare the
model performances. Descriptions and examples can be found in the API
section. Backtide uses the [plotly](https://plotly.com/python/) library
for plotting. Plotly makes interactive, publication-quality graphs that
are rendered using HTML.

<br>

## Available plots

The plotting functions are split into two groups by what they consume:
**data plots** operate on raw market bars (and indicators / dividends) and
are typically used **before** an experiment to explore the universe; **result
plots** operate on the [`RunResult`] / [`ExperimentResult`] objects produced
by [`run_experiment`] and are used to evaluate strategy performance **after**
a run.

### Data analysis

Use these to understand the symbols you plan to trade. They take a
DataFrame of OHLCV bars (typically returned by [`query_bars`]) plus a
small set of plot-specific options.

| Plot | Description |
|---|---|
| [`plot_candlestick`](../api/analysis/plot_candlestick.md) | OHLC candlestick chart with optional volume sub-plot. |
| [`plot_correlation`](../api/analysis/plot_correlation.md) | Correlation heatmap across symbols. |
| [`plot_dividends`](../api/analysis/plot_dividends.md) | Dividend cash-flow timeline. |
| [`plot_drawdown`](../api/analysis/plot_drawdown.md) | Percentage drawdown from running peak. |
| [`plot_price`](../api/analysis/plot_price.md) | Price line chart, with optional indicator overlays. |
| [`plot_returns`](../api/analysis/plot_returns.md) | Distribution of period-over-period returns. |
| [`plot_seasonality`](../api/analysis/plot_seasonality.md) | Average return per calendar bucket (month / weekday / hour). |
| [`plot_volatility`](../api/analysis/plot_volatility.md) | Rolling annualised volatility. |
| [`plot_volume`](../api/analysis/plot_volume.md) | Trading volume bar chart. |
| [`plot_vwap`](../api/analysis/plot_vwap.md) | Volume-weighted average price line. |

In addition, [`compute_statistics`](../api/analysis/compute_statistics.md)
returns a tabular summary (mean / volatility / Sharpe / drawdown / ...) over
the same DataFrame inputs — handy as a non-graphical complement to the
plots above.

### Experiment results

Use these to inspect a finished experiment. They take one or more
[`RunResult`] objects (returned by [`run_experiment`] in
`result.strategies` or by [`query_strategy_runs`]).

| Plot | Multi-run? | Description |
|---|---|---|
| [`plot_cash_holdings`](../api/analysis/plot_cash_holdings.md) | multi | Per-currency cash balance over time. |
| [`plot_mae_mfe`](../api/analysis/plot_mae_mfe.md) | single | Maximum adverse / favourable excursion per trade. |
| [`plot_pnl`](../api/analysis/plot_pnl.md) | multi | Cumulative PnL (or relative return) per strategy. |
| [`plot_pnl_histogram`](../api/analysis/plot_pnl_histogram.md) | multi | Distribution of per-trade PnL. |
| [`plot_position_size`](../api/analysis/plot_position_size.md) | single | Per-symbol position size over time. |
| [`plot_rolling_returns`](../api/analysis/plot_rolling_returns.md) | multi | Compounded return over a trailing window. |
| [`plot_rolling_sharpe`](../api/analysis/plot_rolling_sharpe.md) | multi | Rolling annualised Sharpe ratio per run. |
| [`plot_trade_duration`](../api/analysis/plot_trade_duration.md) | multi | Distribution of trade holding times. |
| [`plot_trade_pnl`](../api/analysis/plot_trade_pnl.md) | multi | Per-trade PnL scatter / bar chart. |

<br>

## Parameters

Apart from the plot-specific parameters, all plots have five parameters in common:

* The `title` parameter adds a title to the plot. The default value doesn't
  show any title. Provide a configuration (as dictionary) to customize its
  appearance, e.g., `#!python title=dict(text="Awesome plot", color="red")`.
  Read more in plotly's [documentation](https://plotly.com/python/figure-labels/).
* The `legend` parameter is used to show/hide, position or customize the
  plot's legend. Provide a configuration (as dictionary) to customize its
  appearance (e.g., `#!python legend=dict(title="Title for legend", title_font_color="red")`)
  or choose one of the following locations:

    - upper left
    - upper right
    - lower left
    - lower right
    - upper center
    - lower center
    - center left
    - center right
    - center
    - out: Position the legend outside the axis, on the right hand side. This
      is plotly's default position. Note that this shrinks the size of the axis
      to fit both legend and axes in the specified `figsize`.

* The `figsize` parameter adjust the plot's size.
* The `filename` parameter is used to save the plot.
* The `display` parameter determines whether to show or return the plot.

<br>

## Aesthetics

The plot's aesthetics are controlled through the `plots` section of the
[configuration][configuration]. The default values are:

* **template:** `"plotly"` — Plotly template for figure styling.
* **palette:** Blue-to-teal gradient. Colors cycle when there are more
  traces than entries. `["rgb(13, 71, 161)", "rgb(2, 136, 209)", "rgb(0,
  172, 193)", "rgb(0, 137, 123)", "rgb(56, 142, 60)", "rgb(129, 199, 132)"]`
* **title_fontsize:** `22` — Font size (px) for plot titles.
* **label_fontsize:** `20` — Font size (px) for axis labels and legends.
* **tick_fontsize:** `14` — Font size (px) for axis tick labels.

To change these values, set them in your configuration file or programmatically:

```python
from backtide.config import get_config, set_config

cfg = get_config()
cfg.plots.template = "plotly_dark"
cfg.plots.title_fontsize = 28
set_config(cfg)
```
