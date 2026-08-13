CREATE TABLE IF NOT EXISTS instruments (
    symbol            VARCHAR NOT NULL,
    provider          VARCHAR NOT NULL,
    instrument_type   VARCHAR NOT NULL,
    name              VARCHAR,
    base              VARCHAR,
    quote             VARCHAR,
    exchange          VARCHAR,
    UNIQUE (symbol, provider)
);
