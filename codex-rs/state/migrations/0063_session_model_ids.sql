CREATE TABLE session_model_ids (
    session_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    external_key TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    initial_state TEXT,
    PRIMARY KEY (session_id, kind, external_key),
    UNIQUE (session_id, kind, ordinal),
    CHECK ((ordinal = 0) = (initial_state IS NOT NULL))
);
