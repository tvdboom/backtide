# Data APIs

Data covers provider discovery, normalized historical bars, local storage, and live instrument
catalogs.

## Public Python API

| Module | Main public surface |
| --- | --- |
| `backtide.data` | Resolve profiles, list instruments, fetch provider catalogs, and download bars. |
| `backtide.storage` | Query and remove normalized bars, dividends, instruments, experiments, and strategy runs. |
| `backtide.live` | List exchange instruments and consume normalized WebSocket updates for Trading. |

## Local application integration

The Data UI uses `/api/instruments`, `/api/bars`, `/api/downloads`, and `/api/storage`. Trading uses
`/api/live/instruments` because live-provider capabilities and symbols can differ from historical
providers.

See [Application endpoints] for the complete method-level table.

[Application endpoints]: application_endpoints.md
