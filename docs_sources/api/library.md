# Library APIs

Library assets are reusable across Research and Trading; they do not belong to either workflow.

| Module | Reuse boundary |
| --- | --- |
| `backtide.strategies` | Built-in strategies and the `BaseStrategy` Python extension contract. |
| `backtide.indicators` | Built-in and custom indicators used by experiments, strategies, and live monitoring. |
| `backtide.metrics` | Built-in experiment metrics and custom post-run metrics; live sessions expose the compatible built-in subset. |
| `backtide.sizers` | Built-in and custom position-sizing policies attached to strategy orders. |

The local application exposes matching CRUD collections at `/api/strategies`, `/api/indicators`,
`/api/metrics`, and `/api/sizers`. Saved strategies and indicators can be selected directly in the
paper-trading wizard. Strategy-required indicators are injected automatically; optional monitoring
indicators are displayed without changing strategy decisions.

Saved sizer presets are code-facing library assets. A custom strategy can attach one to an
`Order` or call it while calculating a quantity, but the experiment and paper-trading setup
screens do not apply a saved sizer independently of the selected strategy. Built-in strategies
continue to own their internal sizing rules.

See the generated `indicators`, `metrics`, `sizers`, and `strategies` sections below for individual
class contracts.
