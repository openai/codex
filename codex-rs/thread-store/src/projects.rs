use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredProjectRoot {
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredProject {
    pub id: String,
    pub name: String,
    pub roots: Vec<StoredProjectRoot>,
    pub metadata: BTreeMap<String, String>,
    pub position: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredProjectsPage {
    pub projects: Vec<StoredProject>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListProjectsParams {
    pub cursor: Option<String>,
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateProjectParams {
    pub name: String,
    pub roots: Vec<StoredProjectRoot>,
    pub metadata: BTreeMap<String, String>,
    pub thread_ids: Vec<String>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedProject {
    pub project: StoredProject,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateProjectParams {
    pub project_id: String,
    pub name: Option<String>,
    pub roots: Option<Vec<StoredProjectRoot>>,
    pub metadata: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveProjectParams {
    pub project_id: String,
    pub before_project_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectMoveOutcome {
    Unchanged,
    Moved,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatedProject {
    pub project: StoredProject,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletedProject {
    pub affected_active_thread_ids: Vec<String>,
    pub affected_archived_thread_ids: Vec<String>,
}
