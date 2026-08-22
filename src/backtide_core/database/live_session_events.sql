CREATE TABLE IF NOT EXISTS live_session_events (
    session_id     VARCHAR NOT NULL,
    kind           VARCHAR NOT NULL,
    event_index    BIGINT NOT NULL,
    payload        VARCHAR NOT NULL,
    UNIQUE (session_id, kind, event_index)
);
