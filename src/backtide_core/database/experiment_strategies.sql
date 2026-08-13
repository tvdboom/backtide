CREATE TABLE IF NOT EXISTS experiment_strategies (
    id               VARCHAR NOT NULL,
    experiment_id    VARCHAR NOT NULL,
    strategy_id      VARCHAR NOT NULL,
    strategy_name    VARCHAR NOT NULL,
    metrics          VARCHAR NOT NULL,
    base_currency    VARCHAR,
    error            VARCHAR,
    is_benchmark     BOOLEAN NOT NULL DEFAULT FALSE,
    UNIQUE (experiment_id, strategy_id)
);
