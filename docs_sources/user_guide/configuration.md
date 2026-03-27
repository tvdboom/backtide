# Configuration
---------------

Backtide is configured through a single [`Config`] object that acts as a
**process-wide singleton**. It is initialized the first time any part of
Backtide reads from it, and it cannot be changed after that point. Set your
configuration at the very start of the process — before calling anything
else from the library.

---

## Configuration file

Backtide automatically discovers a configuration file named `backtide.config`
in the current working directory or its immediate parent. The following
extensions are recognized, checked in this order: `toml`, `yaml`, `yml`, `json`.
If no file is found, the built-in defaults are used.

### Default configuration

=== "TOML"
    ```toml title="backtide.config.toml"
    base_currency = "USD"

    [ingestion]
    storage_path = ".backtide/database.duckdb"

    [ingestion.providers]
    stocks = "yahoo"
    etf = "yahoo"
    forex = "yahoo"
    crypto = "binance"

    [display]
    date_format = "YYYY-MM-DD"
    ```

=== "YAML"
    ```yaml title="backtide.config.yaml"
    base_currency: USD

    ingestion:
        storage_path: .backtide/database.duckdb
        providers:
            stocks: yahoo
            etf: yahoo
            forex: yahoo
            crypto: binance

    display:
        date_format: "YYYY-MM-DD"
        timezone: null
    ```

=== "JSON"
    ```json title="backtide.config.json"
    {
        "base_currency": "USD",
        "ingestion": {
            "storage_path": ".backtide/database.duckdb",
            "providers": {
                "stocks": "yahoo",
                "etf": "yahoo",
                "forex": "yahoo",
                "crypto": "binance"
            }
        },
        "display": {
            "date_format": "YYYY-MM-DD",
            "timezone": null
        }
    }
    ```
