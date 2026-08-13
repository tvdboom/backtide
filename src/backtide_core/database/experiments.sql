CREATE TABLE IF NOT EXISTS experiments (
    id             VARCHAR PRIMARY KEY,
    name           VARCHAR NOT NULL,
    icon           VARCHAR NOT NULL,
    tags           VARCHAR NOT NULL,
    description    VARCHAR NOT NULL,
    config_toml    VARCHAR NOT NULL,
    started_at     BIGINT NOT NULL,
    finished_at    BIGINT NOT NULL,
    status         VARCHAR NOT NULL
);
