use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

pub type Result<T> = std::result::Result<T, CloudTaskError>;

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

#[async_trait::async_trait]
pub trait CloudBackend: Send + Sync {
    async fn list_tasks(&self, env: Option<&str>) -> Result<Vec<TaskSummary>>;
    async fn get_task_summary(&self, id: TaskId) -> Result<TaskSummary>;
    async fn get_task_diff(&self, id: TaskId) -> Result<Option<String>>;
    /// Return assistant output messages (no diff) when available.
    async fn get_task_messages(&self, id: TaskId) -> Result<Vec<String>>;
    /// Return the creating prompt and assistant messages (when available).
    async fn get_task_text(&self, id: TaskId) -> Result<TaskText>;
    /// Return any sibling attempts (best-of-N) for the given assistant turn.
    async fn list_sibling_attempts(
        &self,
        task: TaskId,
        turn_id: String,
    ) -> Result<Vec<TurnAttempt>>;
    /// Dry-run apply (preflight) that validates whether the patch would apply cleanly.
    /// Never modifies the working tree. When `diff_override` is supplied, the provided diff is
    /// used instead of re-fetching the task details so callers can apply alternate attempts.
    async fn apply_task_preflight(
        &self,
        id: TaskId,
        diff_override: Option<String>,
    ) -> Result<ApplyOutcome>;
    async fn apply_task(&self, id: TaskId, diff_override: Option<String>) -> Result<ApplyOutcome>;
    async fn create_task(
        &self,
        env_id: &str,
        prompt: &str,
        git_ref: &str,
        qa_mode: bool,
        best_of_n: usize,
    ) -> Result<CreatedTask>;
}
