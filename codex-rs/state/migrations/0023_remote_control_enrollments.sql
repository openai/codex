CREATE TABLE remote_control_enrollments (
    websocket_url TEXT NOT NULL,
    account_id TEXT NOT NULL,
    server_id TEXT NOT NULL,
    server_name TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (websocket_url, account_id)
);
