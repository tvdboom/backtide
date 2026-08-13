CREATE TABLE IF NOT EXISTS bars (
    symbol            VARCHAR NOT NULL,
    interval          VARCHAR NOT NULL,
    provider          VARCHAR NOT NULL,
    open_ts           BIGINT NOT NULL,
    close_ts          BIGINT NOT NULL,
    open_ts_exchange  BIGINT NOT NULL,
    open              DOUBLE NOT NULL,
    high              DOUBLE NOT NULL,
    low               DOUBLE NOT NULL,
    close             DOUBLE NOT NULL,
    adj_close         DOUBLE NOT NULL,
    volume            DOUBLE NOT NULL,
    n_trades          INTEGER,
    UNIQUE (symbol, provider, interval, open_ts)
);
