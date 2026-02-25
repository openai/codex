CREATE TABLE thread_dynamic_tools (
    thread_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    input_schema TEXT NOT NULL,
    PRIMARY KEY(thread_id, position),
    FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
);

CREATE INDEX idx_thread_dynamic_tools_thread ON thread_dynamic_tools(thread_id);
