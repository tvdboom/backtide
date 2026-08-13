CREATE TABLE IF NOT EXISTS experiment_equity (
    run_id      VARCHAR NOT NULL,
    ts          BIGINT NOT NULL,
    equity      DOUBLE NOT NULL,
    cash        VARCHAR NOT NULL,
    drawdown    DOUBLE NOT NULL,
    UNIQUE (run_id, ts)
);
