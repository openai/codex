use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::future::Future;
use std::pin::Pin;

pub type Result<T> = std::result::Result<T, CloudTaskError>;
pub type CloudBackendFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

#[derive(Debug, thiserror::Error)]
pub enum CloudTaskError {
    #[error("unimplemented: {0}")]
    Unimplemented(&'static str),
    #[error("http error: {0}")]
    Http(String),
    #[error("io error: {0}")]
    Io(String),
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
    /// True when the backend reports this task as a code review.
    #[serde(default)]
    pub is_review: bool,
    /// Number of assistant attempts (best-of-N), when reported by the backend.
    #[serde(default)]
    pub attempt_total: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AttemptStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnAttempt {
    pub turn_id: String,
    pub attempt_placement: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub status: AttemptStatus,
    pub diff: Option<String>,
    pub messages: Vec<String>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskListPage {
    pub tasks: Vec<TaskSummary>,
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DiffSummary {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskText {
    pub prompt: Option<String>,
    pub messages: Vec<String>,
    pub turn_id: Option<String>,
    pub sibling_turn_ids: Vec<String>,
    pub attempt_placement: Option<i64>,
    pub attempt_status: AttemptStatus,
}

impl Default for TaskText {
    fn default() -> Self {
        Self {
            prompt: None,
            messages: Vec::new(),
            turn_id: None,
            sibling_turn_ids: Vec::new(),
            attempt_placement: None,
            attempt_status: AttemptStatus::Unknown,
        }
    }
}

pub trait CloudBackend: Send + Sync {
    fn list_tasks<'a>(
        &'a self,
        env: Option<&'a str>,
        limit: Option<i64>,
        cursor: Option<&'a str>,
    ) -> CloudBackendFuture<'a, TaskListPage>;
    fn get_task_summary(&self, id: TaskId) -> CloudBackendFuture<'_, TaskSummary>;
    fn get_task_diff(&self, id: TaskId) -> CloudBackendFuture<'_, Option<String>>;
    /// Return assistant output messages (no diff) when available.
    fn get_task_messages(&self, id: TaskId) -> CloudBackendFuture<'_, Vec<String>>;
    /// Return the creating prompt and assistant messages (when available).
    fn get_task_text(&self, id: TaskId) -> CloudBackendFuture<'_, TaskText>;
    /// Return any sibling attempts (best-of-N) for the given assistant turn.
    fn list_sibling_attempts(
        &self,
        task: TaskId,
        turn_id: String,
    ) -> CloudBackendFuture<'_, Vec<TurnAttempt>>;
    /// Dry-run apply (preflight) that validates whether the patch would apply cleanly.
    /// Never modifies the working tree. When `diff_override` is supplied, the provided diff is
    /// used instead of re-fetching the task details so callers can apply alternate attempts.
    fn apply_task_preflight(
        &self,
        id: TaskId,
        diff_override: Option<String>,
    ) -> CloudBackendFuture<'_, ApplyOutcome>;
    fn apply_task(
        &self,
        id: TaskId,
        diff_override: Option<String>,
    ) -> CloudBackendFuture<'_, ApplyOutcome>;
    fn create_task<'a>(
        &'a self,
        env_id: &'a str,
        prompt: &'a str,
        git_ref: &'a str,
        qa_mode: bool,
        best_of_n: usize,
    ) -> CloudBackendFuture<'a, CreatedTask>;
}
