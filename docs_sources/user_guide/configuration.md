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
    [general]
    base_currency = "USD"
    triangulation_strategy = "direct"
    triangulation_fiat = "USD"
    triangulation_crypto = "USDT"
    triangulation_crypto_pegged = "USD"
    log_level = "warn"

    [data]
    storage_path = ".backtide"
    dataframe_library = "pandas"

    [data.providers]
    stocks = "yahoo"
    etf = "yahoo"
    forex = "yahoo"
    crypto = "binance"

    [display]
    date_format = "YYYY-MM-DD"
    time_format = "HH:MM"
    currency_prefix = true
    port = 8501

    [plots]
    template = "plotly"
    palette = [
        "rgb(13, 71, 161)",
        "rgb(2, 136, 209)",
        "rgb(0, 172, 193)",
        "rgb(0, 137, 123)",
        "rgb(56, 142, 60)",
        "rgb(129, 199, 132)",
    ]
    title_fontsize = 22
    label_fontsize = 20
    tick_fontsize = 14
    line_width = 2.0
    marker_size = 8.0
    ```

=== "YAML"
    ```yaml title="backtide.config.yaml"
    general:
        base_currency: USD
        triangulation_strategy: direct
        triangulation_fiat: USD
        triangulation_crypto: USDT
        triangulation_crypto_pegged: USD
        log_level: warn

    data:
        storage_path: .backtide
        dataframe_library: pandas
        providers:
            stocks: yahoo
            etf: yahoo
            forex: yahoo
            crypto: binance

    display:
        date_format: "YYYY-MM-DD"
        time_format: "HH:MM"
        timezone: null
        currency_prefix: true
        logokit_api_key: null
        address: null
        port: 8501

    plots:
        template: plotly
        palette:
            - "rgb(13, 71, 161)"
            - "rgb(2, 136, 209)"
            - "rgb(0, 172, 193)"
            - "rgb(0, 137, 123)"
            - "rgb(56, 142, 60)"
            - "rgb(129, 199, 132)"
        title_fontsize: 22
        label_fontsize: 20
        tick_fontsize: 14
        line_width: 2.0
        marker_size: 8.0
    ```

=== "JSON"
    ```json title="backtide.config.json"
    {
        "general": {
            "base_currency": "USD",
            "triangulation_strategy": "direct",
            "triangulation_fiat": "USD",
            "triangulation_crypto": "USDT",
            "triangulation_crypto_pegged": "USD",
            "log_level": "warn"
        },
        "data": {
            "storage_path": ".backtide",
            "dataframe_library": "pandas",
            "providers": {
                "stocks": "yahoo",
                "etf": "yahoo",
                "forex": "yahoo",
                "crypto": "binance"
            }
        },
        "display": {
            "date_format": "YYYY-MM-DD",
            "time_format": "HH:MM",
            "timezone": null,
            "currency_prefix": true,
            "logokit_api_key": null,
            "address": null,
            "port": 8501
        },
        "plots": {
            "template": "plotly",
            "palette": [
                "rgb(13, 71, 161)",
                "rgb(2, 136, 209)",
                "rgb(0, 172, 193)",
                "rgb(0, 137, 123)",
                "rgb(56, 142, 60)",
                "rgb(129, 199, 132)"
            ],
            "title_fontsize": 22,
            "label_fontsize": 20,
            "tick_fontsize": 14,
            "line_width": 2.0,
            "marker_size": 8.0
        }
    }
    ```
