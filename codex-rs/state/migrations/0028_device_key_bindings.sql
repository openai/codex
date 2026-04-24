CREATE TABLE device_key_bindings (
    key_id TEXT PRIMARY KEY NOT NULL,
    account_user_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
