CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    position INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE project_roots (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    path TEXT NOT NULL,
    PRIMARY KEY (project_id, position)
);

CREATE TABLE project_idempotency_keys (
    key TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);

ALTER TABLE threads ADD COLUMN project_id TEXT
    REFERENCES projects(id) ON DELETE SET NULL;

CREATE INDEX idx_projects_position
    ON projects(position ASC, id ASC);
CREATE INDEX idx_threads_project_id
    ON threads(project_id, archived, created_at_ms DESC, id DESC)
    WHERE project_id IS NOT NULL;
