# Application endpoints

The packaged Vue application talks to a same-origin local JSON API. These endpoints are
application integration points; Python users should normally prefer the public APIs in
[Public API product map].

## Overview endpoints

| Method | Endpoint | Purpose |
| --- | --- | --- |
| `GET` | `/api/health` | Check that the local service is available. |
| `GET` | `/api/bootstrap` | Load catalogs, enums, defaults, and live capabilities. |
| `GET` | `/api/dashboard` | Load the home-page summary. |

## Research endpoints

| Method | Endpoint | Purpose |
| --- | --- | --- |
| `GET`, `POST` | `/api/experiments` | List or start experiments. |
| `GET`, `DELETE` | `/api/experiments/{id}` | Inspect or delete an experiment. |
| `GET` | `/api/experiments/{id}/orders` | Page through recorded orders. |
| `GET` | `/api/experiments/{id}/logs` | Download the complete experiment log. |
| `GET` | `/api/experiments/{id}/paper-config` | Create a compatible paper-session draft. |
| `POST` | `/api/experiments/abort` | Cancel the active experiment. |
| `POST` | `/api/results/plot` | Render a stored-result plot. |
| `POST` | `/api/analysis` | Render a data-analysis plot. |

## Trading endpoints

| Method | Endpoint | Purpose |
| --- | --- | --- |
| `GET`, `POST` | `/api/live` | Read or start the active paper session. |
| `POST` | `/api/live/stop` | Stop and finalize the active session. |
| `POST` | `/api/live/pause`, `/api/live/resume` | Pause or resume strategy evaluation. |
| `POST` | `/api/live/flatten` | Flatten positions on the next market event. |
| `POST` | `/api/live/cancel-all` | Cancel resting orders on the next event. |
| `GET` | `/api/live/instruments` | Search a provider's live instrument catalog. |
| `GET` | `/api/live/sessions` | List persisted paper sessions. |
| `GET` | `/api/live/sessions/{id}` | Read a session and its event journal. |
| `POST` | `/api/live/replay` | Replay a persisted journal at `speed` (`0.1`-`100` or `"max"`) through a fresh paper engine. |

## Library endpoints

| Method | Endpoint | Purpose |
| --- | --- | --- |
| `GET`, `POST` | `/api/strategies` | Browse or save strategies. |
| `GET`, `POST` | `/api/indicators` | Browse or save indicators. |
| `GET`, `POST` | `/api/metrics` | Browse or save metrics. |
| `GET`, `POST` | `/api/sizers` | Browse or save position sizers. |
| `PUT`, `DELETE` | `/api/{asset}/{name}` | Update or remove a saved library asset. |

## Data endpoints

| Method | Endpoint | Purpose |
| --- | --- | --- |
| `GET` | `/api/instruments` | Search stored or provider instrument catalogs. |
| `GET` | `/api/bars` | Query stored normalized bars. |
| `POST` | `/api/downloads`, `/api/downloads/plan` | Plan or start downloads. |
| `GET`, `DELETE` | `/api/storage` | Summarize or remove stored series. |

The API is local-only by design. Responses never intentionally include local filesystem
paths or Python stack traces.

[Public API product map]: product_map.md
