CREATE TABLE IF NOT EXISTS live_sessions (
    id             VARCHAR PRIMARY KEY,
    status         VARCHAR NOT NULL,
    started_at     VARCHAR NOT NULL,
    finished_at    VARCHAR,
    config         VARCHAR NOT NULL,
    snapshot       VARCHAR NOT NULL,
    health         VARCHAR NOT NULL,
    error          VARCHAR
);
