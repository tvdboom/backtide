CREATE TABLE IF NOT EXISTS dividends (
    symbol       VARCHAR NOT NULL,
    provider     VARCHAR NOT NULL,
    ex_date      BIGINT NOT NULL,
    amount       DOUBLE NOT NULL,
    UNIQUE (symbol, provider, ex_date)
);
