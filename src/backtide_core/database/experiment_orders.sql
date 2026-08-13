CREATE TABLE IF NOT EXISTS experiment_orders (
    run_id         VARCHAR NOT NULL,
    order_id       VARCHAR NOT NULL,
    ts             BIGINT NOT NULL,
    symbol         VARCHAR NOT NULL,
    order_type     VARCHAR NOT NULL,
    quantity       DOUBLE NOT NULL,
    price          DOUBLE,
    limit_price    DOUBLE,
    status         VARCHAR NOT NULL,
    fill_price     DOUBLE,
    reason         VARCHAR NOT NULL,
    commission     DOUBLE NOT NULL DEFAULT 0,
    pnl            DOUBLE,
    UNIQUE (run_id, order_id)
);
