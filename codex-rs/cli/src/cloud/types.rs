//! Shared types for the headless Cloud CLI.

use chrono::DateTime;
use chrono::Utc;
use codex_cloud_tasks_client::TaskId;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct TaskRow {
    pub id: TaskId,
    pub title: String,
    pub status: String,
    pub updated_at: DateTime<Utc>,
    pub environment_label: Option<String>,
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub is_review: bool,
    pub attempt_total: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariantOutput {
    pub variant_index: usize,
    pub is_base: bool,
    pub attempt_placement: Option<i64>,
    pub status: String,
    pub diff: Option<String>,
    pub messages: Vec<String>,
    pub prompt: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ShowOutput {
    pub task_id: String,
    pub variants: Vec<VariantOutput>,
}
