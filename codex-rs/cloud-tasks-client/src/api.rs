use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unimplemented: {0}")]
    Unimplemented(&'static str),
    #[error("http error: {0}")]
    Http(String),
    #[error("io error: {0}")]
    Io(String),
    /// Expected condition: the task has no diff available yet (e.g., still in progress).
    #[error("no diff available yet")]
    NoDiffYet,
    #[error("{0}")]
    Msg(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    Pending,
    Ready,
    Applied,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: TaskId,
    pub title: String,
    pub status: TaskStatus,
    pub updated_at: DateTime<Utc>,
    /// Backend environment identifier (when available)
    pub environment_id: Option<String>,
    /// Human-friendly environment label (when available)
    pub environment_label: Option<String>,
    pub summary: DiffSummary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApplyStatus {
    Success,
    Partial,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyOutcome {
    pub applied: bool,
    pub status: ApplyStatus,
    pub message: String,
    #[serde(default)]
    pub skipped_paths: Vec<String>,
    #[serde(default)]
    pub conflict_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedTask {
    pub id: TaskId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DiffSummary {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskText {
    pub prompt: Option<String>,
    pub messages: Vec<String>,
}

#[async_trait::async_trait]
pub trait CloudBackend: Send + Sync {
    async fn list_tasks(&self, env: Option<&str>) -> Result<Vec<TaskSummary>>;
    async fn get_task_diff(&self, id: TaskId) -> Result<String>;
    /// Return assistant output messages (no diff) when available.
    async fn get_task_messages(&self, id: TaskId) -> Result<Vec<String>>;
    /// Return the creating prompt and assistant messages (when available).
    async fn get_task_text(&self, id: TaskId) -> Result<TaskText>;
    async fn apply_task(&self, id: TaskId) -> Result<ApplyOutcome>;
    async fn create_task(
        &self,
        env_id: &str,
        prompt: &str,
        git_ref: &str,
        qa_mode: bool,
    ) -> Result<CreatedTask>;
}
