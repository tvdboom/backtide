CREATE TABLE IF NOT EXISTS experiment_trades (
    run_id         VARCHAR NOT NULL,
    symbol         VARCHAR NOT NULL,
    quantity       DOUBLE NOT NULL,
    entry_ts       BIGINT NOT NULL,
    exit_ts        BIGINT NOT NULL,
    entry_price    DOUBLE NOT NULL,
    exit_price     DOUBLE NOT NULL,
    pnl            DOUBLE NOT NULL,
    UNIQUE (run_id, symbol, entry_ts, exit_ts)
);
