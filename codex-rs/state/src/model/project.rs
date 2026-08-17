use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRoot {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub roots: Vec<ProjectRoot>,
    pub metadata: BTreeMap<String, String>,
    pub position: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedProject {
    pub project: Project,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectsPage {
    pub projects: Vec<Project>,
    pub next_cursor: Option<String>,
}
